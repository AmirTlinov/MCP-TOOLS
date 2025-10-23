use axum::{Router, response::IntoResponse, routing::get};
use once_cell::sync::Lazy;
use prometheus::{
    Encoder, Histogram, IntGauge, TextEncoder, register_histogram, register_int_gauge,
};
use std::net::SocketAddr;

pub static LATENCY_HISTO: Lazy<Histogram> = Lazy::new(|| {
    register_histogram!(
        "inspector_latency_ms",
        "Latency of inspector operations in ms"
    )
    .unwrap()
});
pub static OUTBOX_BACKLOG: Lazy<IntGauge> =
    Lazy::new(|| register_int_gauge!("outbox_backlog", "Size of outbox backlog").unwrap());

pub struct PendingGaugeGuard;

impl PendingGaugeGuard {
    pub fn new() -> Self {
        OUTBOX_BACKLOG.inc();
        PendingGaugeGuard
    }
}

impl Drop for PendingGaugeGuard {
    fn drop(&mut self) {
        OUTBOX_BACKLOG.dec();
    }
}

async fn metrics_handler() -> impl IntoResponse {
    let encoder = TextEncoder::new();
    let metrics = prometheus::gather();
    let mut buf = Vec::new();
    let _ = encoder.encode(&metrics, &mut buf);
    let body = axum::body::Bytes::from(buf);
    let mut resp: http::Response<axum::body::Body> =
        http::Response::new(axum::body::Body::from(body));
    let ct = encoder.format_type().to_string();
    resp.headers_mut().insert(
        http::header::CONTENT_TYPE,
        http::HeaderValue::from_str(&ct).unwrap_or(http::HeaderValue::from_static("text/plain")),
    );
    resp
}

pub async fn spawn_metrics_server(addr: SocketAddr) {
    let app = Router::new().route("/metrics", get(metrics_handler));
    tokio::spawn(async move {
        if let Ok(listener) = tokio::net::TcpListener::bind(addr).await {
            let _ = axum::serve(listener, app).await;
        }
    });
}
