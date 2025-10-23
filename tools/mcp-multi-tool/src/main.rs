use anyhow::Result;
use mcp_multi_tool::{
    adapters::server::InspectorServer,
    app::{inspector_service::InspectorService, registry::ToolRegistry},
    infra::{config::AppConfig, metrics, outbox::Outbox},
    shared::idempotency::IdempotencyStore,
};
use rmcp::{ServiceExt, transport::stdio};
use std::{sync::Arc, time::Duration};
use time::OffsetDateTime;
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
    if let Some(metrics_cfg) = config.metrics_server_config()? {
        if metrics_cfg.allow_insecure && metrics_cfg.tls.is_none() {
            tracing::warn!(
                addr = %metrics_cfg.addr,
                "metrics server running without TLS (dev override)"
            );
        } else if metrics_cfg.auth_token.is_none() {
            tracing::warn!(
                addr = %metrics_cfg.addr,
                "metrics auth token missing; set METRICS_AUTH_TOKEN for production"
            );
        }
        metrics::spawn_metrics_server(metrics_cfg).await;
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
        let outbox = outbox.clone();
        tokio::spawn(async move {
            let ttl = Duration::from_secs(60);
            let cadence = Duration::from_secs(30);
            loop {
                sleep(cadence).await;
                let reaped = store.reap_expired(ttl, OffsetDateTime::now_utc());
                if reaped.is_empty() {
                    continue;
                }
                metrics::record_reaper_timeout(reaped.len());
                for item in &reaped {
                    if let Err(err) = outbox.append(&item.event) {
                        tracing::error!(
                            key = %item.idempotency_key,
                            %err,
                            "failed to append reaper event to outbox"
                        );
                    }
                }
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
