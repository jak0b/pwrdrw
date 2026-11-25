use std::{
    error::Error,
    future::IntoFuture,
    net::{IpAddr, SocketAddr},
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
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use tokio::{net::TcpListener, task::JoinSet};

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
    hosts: Vec<IpAddr>,
    smc: SmcConnection,
) -> Result<(), Box<dyn Error>> {
    let prometheus = PrometheusBuilder::new().install_recorder()?;
    describe_metrics();
    let state = MetricsState {
        smc: Arc::new(Mutex::new(smc)),
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
