mod adapters;
mod app;
mod domain;
mod infra;
mod shared;

use crate::{
    adapters::server::InspectorServer,
    infra::{config::AppConfig, metrics},
};
use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};
use std::net::SocketAddr;
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("off"));
    // IMPORTANT: write logs to stderr; stdout must remain clear for MCP JSON-RPC
    fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .compact()
        .init();

    let config = AppConfig::from_env();
    if let Some(addr_str) = &config.metrics_addr {
        match addr_str.parse::<SocketAddr>() {
            Ok(addr) => {
                if config.allow_insecure_metrics_dev.unwrap_or(false) {
                    tracing::warn!(%addr, "metrics server running without TLS (dev override)");
                }
                metrics::spawn_metrics_server(addr).await;
            }
            Err(error) => tracing::warn!(%addr_str, %error, "invalid METRICS_ADDR"),
        }
    }

    let handler = InspectorServer::new();
    // Start the server. Emit tools/list_changed inside on_initialized so
    // the notification is not lost before the handshake completes.
    let server = handler.serve(stdio()).await?;
    server.waiting().await?;
    Ok(())
}
