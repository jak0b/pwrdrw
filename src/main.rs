use std::{
    error::Error,
    ffi::c_void,
    future::IntoFuture,
    io,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV6},
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{
    Router,
    body::Body,
    extract::State,
    http::{Response, StatusCode, header},
    routing::get,
};
use clap::Parser;
use metrics::{describe_gauge, gauge};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use socket2::{Domain, Protocol, Socket, Type};
use tokio::{net::TcpListener, time::sleep};

#[cfg(not(target_os = "macos"))]
compile_error!("power-usage-macos can only be built for macOS");

#[derive(Debug, Parser)]
#[command(about = "Read power usage from the macOS AppleSMC service")]
struct Args {
    /// Continuously print readings
    #[arg(short = 'l', long = "loop")]
    looping: bool,

    /// Clear the terminal before each reading
    #[arg(short = 'c', long = "clear")]
    clear_screen: bool,

    /// Delay between readings in milliseconds
    #[arg(short = 'r', long, default_value_t = 1000, value_parser = clap::value_parser!(u64).range(1..=2147483647))]
    interval: u64,

    /// Serve Prometheus metrics on this port
    #[arg(
        short = 'p',
        long,
        value_parser = clap::value_parser!(u16).range(1..),
        conflicts_with_all = ["looping", "clear_screen", "interval"]
    )]
    port: Option<u16>,
}

#[derive(Clone, Copy)]
struct Readings {
    voltage: f32,
    current: f32,
    system_total: f32,
}

struct SmcConnection {
    connection: u32,
}

#[repr(C)]
struct SmcKeyData {
    key: u32,
    ignored: [u8; 24],
    size: u32,
    ignored2: [u8; 10],
    command: i8,
    ignored3: [u8; 5],
    value: f32,
    ignored4: [u8; 28],
}

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOServiceMatching(name: *const i8) -> *mut c_void;
    fn IOServiceGetMatchingServices(
        master_port: u32,
        matching: *mut c_void,
        iterator: *mut u32,
    ) -> i32;
    fn IOIteratorNext(iterator: u32) -> u32;
    fn IOObjectRelease(object: u32) -> i32;
    fn IOServiceOpen(service: u32, owning_task: u32, options: u32, connection: *mut u32) -> i32;
    fn IOServiceClose(connection: u32) -> i32;
    fn IOConnectCallStructMethod(
        connection: u32,
        selector: u32,
        input: *const c_void,
        input_size: usize,
        output: *mut c_void,
        output_size: *mut usize,
    ) -> i32;
}

#[link(name = "System")]
unsafe extern "C" {
    static mach_task_self_: u32;
}

impl SmcConnection {
    fn open() -> Result<Self, Box<dyn Error>> {
        let service_name = b"AppleSMC\0";
        let mut iterator = 0;
        let mut connection = 0;

        // SAFETY: These calls use the IOKit API with valid pointers and release
        // each returned IOKit object on every path.
        unsafe {
            let matching = IOServiceMatching(service_name.as_ptr().cast());
            if matching.is_null() || IOServiceGetMatchingServices(0, matching, &mut iterator) != 0 {
                return Err(io::Error::other("Unable to find AppleSMC").into());
            }

            let service = IOIteratorNext(iterator);
            IOObjectRelease(iterator);
            if service == 0 {
                return Err(io::Error::other("AppleSMC service is unavailable").into());
            }

            let result = IOServiceOpen(service, mach_task_self_, 0, &mut connection);
            IOObjectRelease(service);
            if result != 0 {
                return Err(io::Error::other("Unable to open AppleSMC").into());
            }
        }

        Ok(Self { connection })
    }

    fn read_power(&self) -> Result<Readings, io::Error> {
        Ok(Readings {
            voltage: self.read_key(u32::from_be_bytes(*b"VD0R"))?,
            current: self.read_key(u32::from_be_bytes(*b"ID0R"))?,
            system_total: self.read_key(u32::from_be_bytes(*b"PSTR"))?,
        })
    }

    fn read_key(&self, key: u32) -> Result<f32, io::Error> {
        let mut request: SmcKeyData = unsafe { std::mem::zeroed() };
        let mut response: SmcKeyData = unsafe { std::mem::zeroed() };
        let mut response_size = std::mem::size_of::<SmcKeyData>();
        request.key = key;
        request.size = 4;
        request.command = 5;

        // SAFETY: The request and response match the SMC structure expected by
        // selector 2. Both buffers remain valid for the duration of the call.
        let result = unsafe {
            IOConnectCallStructMethod(
                self.connection,
                2,
                (&request as *const SmcKeyData).cast(),
                std::mem::size_of::<SmcKeyData>(),
                (&mut response as *mut SmcKeyData).cast(),
                &mut response_size,
            )
        };
        if result != 0 {
            return Err(io::Error::other(format!(
                "Unable to read AppleSMC key {key:#010x}"
            )));
        }

        Ok(response.value)
    }
}

impl Drop for SmcConnection {
    fn drop(&mut self) {
        // SAFETY: The connection was successfully opened by this instance.
        unsafe {
            IOServiceClose(self.connection);
        }
    }
}

#[derive(Clone)]
struct MetricsState {
    smc: Arc<Mutex<SmcConnection>>,
    prometheus: PrometheusHandle,
}

fn record_metrics(readings: Readings) {
    gauge!("macos_power_input_voltage_volts").set(f64::from(readings.voltage));
    gauge!("macos_power_input_current_amperes").set(f64::from(readings.current));
    gauge!("macos_power_input_watts")
        .set(f64::from(readings.voltage) * f64::from(readings.current));
    gauge!("macos_system_power_watts").set(f64::from(readings.system_total));
}

async fn metrics_handler(State(state): State<MetricsState>) -> Response<Body> {
    let readings = match state.smc.lock() {
        Ok(smc) => match smc.read_power() {
            Ok(readings) => readings,
            Err(_) => return unavailable_response(),
        },
        Err(_) => return unavailable_response(),
    };
    record_metrics(readings);

    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )
        .body(Body::from(state.prometheus.render()))
        .expect("static metrics response is valid")
}

fn unavailable_response() -> Response<Body> {
    Response::builder()
        .status(StatusCode::SERVICE_UNAVAILABLE)
        .body(Body::from("Unable to read AppleSMC\n"))
        .expect("static error response is valid")
}

fn describe_metrics() {
    describe_gauge!(
        "macos_power_input_voltage_volts",
        "DC input voltage in volts."
    );
    describe_gauge!(
        "macos_power_input_current_amperes",
        "DC input current in amperes."
    );
    describe_gauge!("macos_power_input_watts", "DC input power in watts.");
    describe_gauge!("macos_system_power_watts", "Total system power in watts.");
}

async fn serve_metrics(port: u16, smc: SmcConnection) -> Result<(), Box<dyn Error>> {
    let prometheus = PrometheusBuilder::new().install_recorder()?;
    describe_metrics();
    let state = MetricsState {
        smc: Arc::new(Mutex::new(smc)),
        prometheus,
    };
    let app = Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(state);
    let ipv4 = TcpListener::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, port))).await?;

    let ipv6_socket = Socket::new(Domain::IPV6, Type::STREAM, Some(Protocol::TCP))?;
    ipv6_socket.set_only_v6(true)?;
    ipv6_socket.set_reuse_address(true)?;
    ipv6_socket.bind(&SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, port, 0, 0).into())?;
    ipv6_socket.listen(1024)?;
    ipv6_socket.set_nonblocking(true)?;
    let ipv6 = TcpListener::from_std(ipv6_socket.into())?;

    eprintln!("Serving metrics on http://localhost:{port}/metrics");
    let ipv4_app = app.clone();
    let ipv6_app = app;
    tokio::try_join!(
        async move { axum::serve(ipv4, ipv4_app).into_future().await },
        async move { axum::serve(ipv6, ipv6_app).into_future().await }
    )?;
    Ok(())
}

async fn print_readings(smc: &Mutex<SmcConnection>) -> Result<(), Box<dyn Error>> {
    let readings = smc
        .lock()
        .map_err(|_| io::Error::other("AppleSMC lock was poisoned"))?
        .read_power()?;
    println!(
        "DC In: {:.6}V {:.6}A {:.6}W",
        readings.voltage,
        readings.current,
        readings.voltage * readings.current
    );
    println!("System Total: {:.6}W", readings.system_total);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let smc = SmcConnection::open()?;

    if let Some(port) = args.port {
        return serve_metrics(port, smc).await;
    }

    let smc = Mutex::new(smc);
    loop {
        if args.looping && args.clear_screen {
            print!("\x1b[H\x1b[2J");
        }
        print_readings(&smc).await?;
        if !args.looping {
            return Ok(());
        }
        sleep(Duration::from_millis(args.interval)).await;
    }
}
