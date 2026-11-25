use std::{
    error::Error,
    future::IntoFuture,
    net::{IpAddr, SocketAddr},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
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
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use tokio::{net::TcpListener, task::JoinSet, time::MissedTickBehavior};

use crate::power_reading::{Readings, SmcConnection};

#[derive(Clone)]
struct MetricsState {
    readings_available: Arc<AtomicBool>,
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
    if !state.readings_available.load(Ordering::Relaxed) {
        return unavailable_response();
    }

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

fn bind_listener(host: IpAddr, port: u16) -> Result<TcpListener, Box<dyn Error>> {
    let domain = if host.is_ipv4() {
        Domain::IPV4
    } else {
        Domain::IPV6
    };
    let socket = Socket::new(domain, Type::STREAM, Some(Protocol::TCP))?;
    socket.set_reuse_address(true)?;
    if host.is_ipv6() {
        socket.set_only_v6(true)?;
    }
    socket.bind(&SockAddr::from(SocketAddr::new(host, port)))?;
    socket.listen(1024)?;
    socket.set_nonblocking(true)?;
    Ok(TcpListener::from_std(socket.into())?)
}

pub async fn serve(
    port: u16,
    poll_interval: u64,
    hosts: Vec<IpAddr>,
    smc: SmcConnection,
) -> Result<(), Box<dyn Error>> {
    let prometheus = PrometheusBuilder::new().install_recorder()?;
    describe_metrics();
    let readings_available = Arc::new(AtomicBool::new(false));
    let state = MetricsState {
        readings_available: Arc::clone(&readings_available),
        prometheus,
    };
    let app = Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(state);

    let listeners = hosts
        .into_iter()
        .map(|host| bind_listener(host, port))
        .collect::<Result<Vec<_>, _>>()?;
    let addresses = listeners
        .iter()
        .map(|listener| listener.local_addr())
        .collect::<Result<Vec<_>, _>>()?;

    let smc = Arc::new(Mutex::new(smc));
    let polling_smc = Arc::clone(&smc);
    let poll_interval = Duration::from_millis(poll_interval);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(poll_interval);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            interval.tick().await;
            let smc = Arc::clone(&polling_smc);
            match tokio::task::spawn_blocking(move || {
                let smc = smc
                    .lock()
                    .map_err(|_| std::io::Error::other("AppleSMC lock was poisoned"))?;
                smc.read_power()
            })
            .await
            {
                Ok(Ok(readings)) => {
                    record_metrics(readings);
                    readings_available.store(true, Ordering::Relaxed);
                }
                Ok(Err(error)) => {
                    eprintln!("Unable to read AppleSMC: {error}");
                    readings_available.store(false, Ordering::Relaxed);
                }
                Err(error) => {
                    eprintln!("AppleSMC polling task failed: {error}");
                    readings_available.store(false, Ordering::Relaxed);
                }
            }
        }
    });

    eprintln!(
        "Serving metrics on {}",
        addresses
            .iter()
            .map(|address| format!("http://{address}/metrics"))
            .collect::<Vec<_>>()
            .join(", ")
    );

    let mut servers = JoinSet::new();
    for listener in listeners {
        let app = app.clone();
        servers.spawn(async move { axum::serve(listener, app).into_future().await });
    }
    while let Some(result) = servers.join_next().await {
        result??;
    }
    Ok(())
}
