use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub metrics_addr: Option<String>,
    pub allow_insecure_metrics_dev: Option<bool>,
    pub outbox_path: Option<String>,
    pub outbox_dlq_path: Option<String>,
    pub outbox_db_path: Option<String>,
    #[serde(default)]
    pub idempotency_conflict_policy: IdempotencyConflictPolicy,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let metrics_addr = std::env::var("METRICS_ADDR").ok();
        let allow_insecure_metrics_dev = std::env::var("ALLOW_INSECURE_METRICS_DEV")
            .ok()
            .and_then(|v| v.parse::<bool>().ok());
        let outbox_path = std::env::var("OUTBOX_PATH").ok();
        let outbox_dlq_path = std::env::var("OUTBOX_DLQ_PATH").ok();
        let outbox_db_path = std::env::var("OUTBOX_DB_PATH").ok();
        let idempotency_conflict_policy = std::env::var("IDEMPOTENCY_CONFLICT_POLICY")
            .ok()
            .and_then(|raw| IdempotencyConflictPolicy::from_str(&raw).ok())
            .unwrap_or_default();
        Self {
            metrics_addr,
            allow_insecure_metrics_dev,
            outbox_path,
            outbox_dlq_path,
            outbox_db_path,
            idempotency_conflict_policy,
        }
    }

    pub fn outbox_paths(&self) -> (PathBuf, PathBuf) {
        let main = self
            .outbox_path
            .as_deref()
            .unwrap_or("data/outbox/events.jsonl");
        let dlq = self
            .outbox_dlq_path
            .as_deref()
            .unwrap_or("data/outbox/dlq.jsonl");
        (PathBuf::from(main), PathBuf::from(dlq))
    }

    pub fn outbox_db_path(&self) -> Option<PathBuf> {
        self.outbox_db_path
            .as_deref()
            .map(|path| PathBuf::from(path))
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdempotencyConflictPolicy {
    ReturnExisting,
    Conflict409,
}

impl Default for IdempotencyConflictPolicy {
    fn default() -> Self {
        Self::Conflict409
    }
}

impl FromStr for IdempotencyConflictPolicy {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "return_existing" => Ok(Self::ReturnExisting),
            "409" | "conflict_409" | "conflict" => Ok(Self::Conflict409),
            other => Err(anyhow!("unknown idempotency conflict policy '{}'", other)),
        }
    }
}
