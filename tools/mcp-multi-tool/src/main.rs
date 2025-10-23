use anyhow::Result;
use mcp_multi_tool::{
    adapters::server::InspectorServer,
    app::{inspector_service::InspectorService, registry::ToolRegistry},
    infra::{config::AppConfig, metrics, outbox::Outbox},
    shared::idempotency::IdempotencyStore,
};
use rmcp::{ServiceExt, transport::stdio};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::time::sleep;
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

    let config = AppConfig::load()?;
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

    let (outbox_main, outbox_dlq) = config.outbox_paths();
    let outbox = if let Some(db_path) = config.outbox_db_path() {
        Outbox::sqlite(db_path, outbox_dlq.clone())?
    } else {
        Outbox::file(outbox_main, outbox_dlq.clone())?
    };
    let outbox = Arc::new(outbox);
    let idempotency = Arc::new(IdempotencyStore::new());
    {
        let store = idempotency.clone();
        tokio::spawn(async move {
            let ttl = Duration::from_secs(60);
            let cadence = Duration::from_secs(30);
            loop {
                sleep(cadence).await;
                store.reap_expired(ttl);
            }
        });
    }

    let handler = InspectorServer::new(
        InspectorService::new(),
        ToolRegistry::default(),
        outbox,
        idempotency,
        config.idempotency_conflict_policy,
    );
    // Start the server. Emit tools/list_changed inside on_initialized so
    // the notification is not lost before the handshake completes.
    let server = handler.serve(stdio()).await?;
    server.waiting().await?;
    Ok(())
}
