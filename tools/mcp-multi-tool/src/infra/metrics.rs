use axum::{
    Router,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::get,
};
use axum_server::tls_rustls::RustlsConfig;
use once_cell::sync::Lazy;
use prometheus::{
    Encoder, Histogram, HistogramVec, IntCounter, IntGauge, TextEncoder, register_histogram,
    register_histogram_vec, register_int_counter, register_int_gauge,
};
use std::{collections::HashMap, net::SocketAddr, path::PathBuf, time::Duration};
use tokio::net::TcpListener;
use tracing::{error, info, warn};

pub static LATENCY_HISTO: Lazy<Histogram> = Lazy::new(|| {
    register_histogram!(
        "inspector_latency_ms",
        "Latency of inspector operations in ms"
    )
    .unwrap()
});

pub static INSPECTOR_INFLIGHT: Lazy<IntGauge> =
    Lazy::new(|| register_int_gauge!("inspector_inflight", "In-flight inspector calls").unwrap());

pub static OUTBOX_BACKLOG: Lazy<IntGauge> = Lazy::new(|| {
    register_int_gauge!(
        "outbox_backlog",
        "Total events appended to the transactional outbox"
    )
    .unwrap()
});

pub static REAPER_TIMEOUTS: Lazy<IntCounter> = Lazy::new(|| {
    register_int_counter!(
        "idempotency_timeouts_total",
        "Number of inspection runs failed by the reaper"
    )
    .unwrap()
});

pub static ERROR_BUDGET_FROZEN: Lazy<IntGauge> = Lazy::new(|| {
    register_int_gauge!(
        "error_budget_frozen",
        "1 when the inspector is in an error budget freeze"
    )
    .unwrap()
});

pub static LOCK_WAIT_HISTO: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec!(
        "inspector_lock_wait_ms",
        "Mutex lock wait duration in milliseconds",
        &["component"]
    )
    .unwrap()
});

#[derive(Clone)]
pub struct PendingGaugeGuard;

impl PendingGaugeGuard {
    pub fn new() -> Self {
        INSPECTOR_INFLIGHT.inc();
        PendingGaugeGuard
    }
}

impl Drop for PendingGaugeGuard {
    fn drop(&mut self) {
        INSPECTOR_INFLIGHT.dec();
    }
}

#[derive(Clone, Debug)]
pub struct TlsConfig {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct MetricsServerConfig {
    pub addr: SocketAddr,
    pub auth_token: Option<String>,
    pub allow_insecure: bool,
    pub tls: Option<TlsConfig>,
}

#[derive(Clone)]
struct MetricsState {
    auth_token: Option<String>,
}

pub async fn spawn_metrics_server(config: MetricsServerConfig) {
    let MetricsServerConfig {
        addr,
        auth_token,
        allow_insecure,
        tls,
    } = config.clone();
    if !allow_insecure && tls.is_none() {
        warn!(%addr, "metrics server skipped: TLS required but not configured");
        return;
    }

    let state = MetricsState { auth_token };
    let app = Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(state.clone());

    tokio::spawn(async move {
        if let Some(tls_cfg) = tls {
            match RustlsConfig::from_pem_file(&tls_cfg.cert_path, &tls_cfg.key_path).await {
                Ok(rustls_config) => {
                    info!(%addr, "metrics server (TLS) starting");
                    if let Err(err) = axum_server::bind_rustls(addr, rustls_config)
                        .serve(app.into_make_service())
                        .await
                    {
                        error!(%addr, %err, "metrics server terminated");
                    }
                }
                Err(err) => {
                    error!(%addr, %err, "failed to load TLS config");
                }
            }
        } else {
            info!(%addr, "metrics server (HTTP) starting");
            match TcpListener::bind(addr).await {
                Ok(listener) => {
                    if let Err(err) = axum::serve(listener, app.into_make_service()).await {
                        error!(%addr, %err, "metrics server terminated");
                    }
                }
                Err(err) => {
                    error!(%addr, %err, "failed to bind metrics listener");
                }
            }
        }
    });
}

async fn metrics_handler(
    State(state): State<MetricsState>,
    headers: HeaderMap,
) -> axum::response::Response {
    if let Some(token) = &state.auth_token {
        if !is_authorized(headers.get(http::header::AUTHORIZATION), token) {
            return StatusCode::UNAUTHORIZED.into_response();
        }
    }

    let encoder = TextEncoder::new();
    let metrics = prometheus::gather();
    let mut buf = Vec::new();
    if let Err(err) = encoder.encode(&metrics, &mut buf) {
        error!(%err, "failed to encode metrics");
        return (StatusCode::INTERNAL_SERVER_ERROR, "metrics encoding failed").into_response();
    }

    let body = axum::body::Bytes::from(buf);
    let mut resp: http::Response<axum::body::Body> =
        http::Response::new(axum::body::Body::from(body));
    let ct = encoder.format_type().to_string();
    resp.headers_mut().insert(
        http::header::CONTENT_TYPE,
        HeaderValue::from_str(&ct).unwrap_or(HeaderValue::from_static("text/plain")),
    );
    resp.into_response()
}

fn is_authorized(header: Option<&HeaderValue>, token: &str) -> bool {
    match header.and_then(|value| value.to_str().ok()) {
        Some(value) if value.starts_with("Bearer ") => value[7..].trim() == token,
        _ => false,
    }
}

pub fn increment_outbox_backlog() {
    OUTBOX_BACKLOG.inc();
}

pub fn record_reaper_timeout(count: usize) {
    if count > 0 {
        REAPER_TIMEOUTS.inc_by(count as u64);
    }
}

pub fn set_error_budget_frozen(frozen: bool) {
    ERROR_BUDGET_FROZEN.set(if frozen { 1 } else { 0 });
}

pub fn observe_lock_wait(component: &'static str, duration: Duration) {
    let ms = duration.as_secs_f64() * 1000.0;
    LOCK_WAIT_HISTO.with_label_values(&[component]).observe(ms);
    #[cfg(test)]
    test_support::record_lock_wait(component, ms);
}

#[cfg(test)]
mod test_support {
    use super::*;
    use once_cell::sync::Lazy;
    use parking_lot::Mutex;

    static LOCK_WAITS: Lazy<Mutex<HashMap<&'static str, Vec<f64>>>> =
        Lazy::new(|| Mutex::new(HashMap::new()));

    pub fn record_lock_wait(component: &'static str, ms: f64) {
        LOCK_WAITS.lock().entry(component).or_default().push(ms);
    }

    pub fn take_lock_wait_records() -> HashMap<String, Vec<f64>> {
        let mut map = LOCK_WAITS.lock();
        let result = map.drain().map(|(k, v)| (k.to_string(), v)).collect();
        result
    }
}

pub fn take_lock_wait_records() -> HashMap<String, Vec<f64>> {
    #[cfg(test)]
    {
        return test_support::take_lock_wait_records();
    }
    #[cfg(not(test))]
    {
        HashMap::new()
    }
}
