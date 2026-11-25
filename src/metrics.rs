use std::{
    error::Error,
    future::IntoFuture,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV6},
    sync::{Arc, Mutex},
};

use axum::{
    Router,
    body::Body,
    extract::State,
    http::{Response, StatusCode, header},
    routing::get,
};
use metrics::{describe_gauge, gauge};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::TcpListener;

use crate::power_reading::{Readings, SmcConnection};

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

pub async fn serve(port: u16, smc: SmcConnection) -> Result<(), Box<dyn Error>> {
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

    eprintln!(
        "Serving metrics on http://{}/metrics and http://{}/metrics",
        ipv4.local_addr()?,
        ipv6.local_addr()?
    );
    let ipv4_app = app.clone();
    let ipv6_app = app;
    tokio::try_join!(
        async move { axum::serve(ipv4, ipv4_app).into_future().await },
        async move { axum::serve(ipv6, ipv6_app).into_future().await }
    )?;
    Ok(())
}
