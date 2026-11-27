use std::{
    error::Error,
    future::IntoFuture,
    net::{IpAddr, SocketAddr},
    time::Duration,
};

use axum::{
    Router,
    body::Body,
    extract::State,
    http::{Response, StatusCode, header},
    routing::get,
};
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use tokio::{net::TcpListener, task::JoinSet};

use crate::{
    metrics::{Metrics, start_polling},
    power_reading::PowerReader,
};

async fn metrics_handler(State(metrics): State<Metrics>) -> Response<Body> {
    match metrics.render() {
        Some(body) => Response::builder()
            .status(StatusCode::OK)
            .header(
                header::CONTENT_TYPE,
                "text/plain; version=0.0.4; charset=utf-8",
            )
            .body(Body::from(body))
            .expect("static metrics response is valid"),
        None => unavailable_response(),
    }
}

fn unavailable_response() -> Response<Body> {
    Response::builder()
        .status(StatusCode::SERVICE_UNAVAILABLE)
        .body(Body::from("Unable to read power sensors\n"))
        .expect("static error response is valid")
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
    smc: PowerReader,
) -> Result<(), Box<dyn Error>> {
    let metrics = Metrics::new()?;
    let app = Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(metrics.clone());

    let listeners = hosts
        .into_iter()
        .map(|host| bind_listener(host, port))
        .collect::<Result<Vec<_>, _>>()?;
    let addresses = listeners
        .iter()
        .map(|listener| listener.local_addr())
        .collect::<Result<Vec<_>, _>>()?;

    start_polling(smc, Duration::from_millis(poll_interval), metrics);

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
