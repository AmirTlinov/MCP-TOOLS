use crate::{
    infra::metrics::{MetricsServerConfig, TlsConfig},
    shared::types::ReleaseTrack,
};
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;

const CONFIG_DIR_ENV: &str = "APP_CONFIG_DIR";
const CONFIG_PROFILE_ENV: &str = "APP_CONFIG_PROFILE";
const DEFAULT_CONFIG_DIR: &str = "config";
const DEFAULT_PROFILE: &str = "default";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub metrics_addr: Option<String>,
    pub allow_insecure_metrics_dev: Option<bool>,
    pub metrics_auth_token: Option<String>,
    pub metrics_tls_cert_path: Option<String>,
    pub metrics_tls_key_path: Option<String>,
    pub outbox_path: Option<String>,
    pub outbox_dlq_path: Option<String>,
    pub outbox_db_path: Option<String>,
    #[serde(default)]
    pub idempotency_conflict_policy: IdempotencyConflictPolicy,
    #[serde(default)]
    pub error_budget: ErrorBudgetSettings,
    #[serde(default)]
    pub release_track: ReleaseTrack,
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let base_dir = env::var(CONFIG_DIR_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(DEFAULT_CONFIG_DIR));
        Self::load_from_dir(&base_dir)
    }

    pub fn load_from_dir<P: AsRef<Path>>(dir: P) -> Result<Self> {
        let dir = dir.as_ref();
        let mut config = AppConfig::default();
        let mut overlays = Vec::new();

        if dir.exists() {
            let mut profiles = Vec::new();
            profiles.push(DEFAULT_PROFILE.to_string());
            if let Ok(active_profile) = env::var(CONFIG_PROFILE_ENV) {
                if !active_profile.trim().is_empty() && active_profile != DEFAULT_PROFILE {
                    profiles.push(active_profile);
                }
            }
            profiles.push("local".to_string());

            for profile in profiles {
                let candidate = dir.join(format!("{profile}.toml"));
                if let Some(overlay) = ConfigOverlay::from_file(&candidate)? {
                    overlays.push(overlay);
                }
            }
        }

        overlays.push(ConfigOverlay::from_env());

        for overlay in overlays {
            config.apply_overlay(overlay);
        }

        Ok(config)
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

    fn apply_overlay(&mut self, overlay: ConfigOverlay) {
        if let Some(value) = overlay.metrics_addr {
            self.metrics_addr = Some(value);
        }
        if let Some(value) = overlay.allow_insecure_metrics_dev {
            self.allow_insecure_metrics_dev = Some(value);
        }
        if let Some(value) = overlay.metrics_auth_token {
            self.metrics_auth_token = Some(value);
        }
        if let Some(value) = overlay.metrics_tls_cert_path {
            self.metrics_tls_cert_path = Some(value);
        }
        if let Some(value) = overlay.metrics_tls_key_path {
            self.metrics_tls_key_path = Some(value);
        }
        if let Some(value) = overlay.outbox_path {
            self.outbox_path = Some(value);
        }
        if let Some(value) = overlay.outbox_dlq_path {
            self.outbox_dlq_path = Some(value);
        }
        if let Some(value) = overlay.outbox_db_path {
            self.outbox_db_path = Some(value);
        }
        if let Some(policy) = overlay.idempotency_conflict_policy {
            self.idempotency_conflict_policy = policy;
        }
        if let Some(budget) = overlay.error_budget {
            self.error_budget.apply_overlay(budget);
        }
        if let Some(track) = overlay.release_track {
            self.release_track = track;
        }
    }

    pub fn metrics_server_config(&self) -> Result<Option<MetricsServerConfig>> {
        let addr = match self.metrics_addr.as_ref() {
            Some(addr) => addr
                .parse::<SocketAddr>()
                .with_context(|| format!("parse METRICS_ADDR '{}'", addr))?,
            None => return Ok(None),
        };

        let allow_insecure = self.allow_insecure_metrics_dev.unwrap_or(false);
        let tls = match (
            self.metrics_tls_cert_path.as_ref(),
            self.metrics_tls_key_path.as_ref(),
        ) {
            (Some(cert), Some(key)) => Some(TlsConfig {
                cert_path: PathBuf::from(cert),
                key_path: PathBuf::from(key),
            }),
            (None, None) => None,
            _ => {
                return Err(anyhow!(
                    "metrics TLS requires both METRICS_TLS_CERT_PATH and METRICS_TLS_KEY_PATH"
                ));
            }
        };

        Ok(Some(MetricsServerConfig {
            addr,
            auth_token: self.metrics_auth_token.clone(),
            allow_insecure,
            tls,
        }))
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            metrics_addr: None,
            allow_insecure_metrics_dev: None,
            metrics_auth_token: None,
            metrics_tls_cert_path: None,
            metrics_tls_key_path: None,
            outbox_path: None,
            outbox_dlq_path: None,
            outbox_db_path: None,
            idempotency_conflict_policy: IdempotencyConflictPolicy::default(),
            error_budget: ErrorBudgetSettings::default(),
            release_track: ReleaseTrack::default(),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct ConfigOverlay {
    metrics_addr: Option<String>,
    allow_insecure_metrics_dev: Option<bool>,
    metrics_auth_token: Option<String>,
    metrics_tls_cert_path: Option<String>,
    metrics_tls_key_path: Option<String>,
    outbox_path: Option<String>,
    outbox_dlq_path: Option<String>,
    outbox_db_path: Option<String>,
    idempotency_conflict_policy: Option<IdempotencyConflictPolicy>,
    #[serde(default)]
    error_budget: Option<ErrorBudgetOverlay>,
    #[serde(default)]
    release_track: Option<ReleaseTrack>,
}

impl ConfigOverlay {
    fn from_file(path: &Path) -> Result<Option<Self>> {
        if !path.is_file() {
            return Ok(None);
        }
        let contents = fs::read_to_string(path)
            .with_context(|| format!("read config file {}", path.display()))?;
        let overlay: Self = toml::from_str(&contents)
            .with_context(|| format!("parse config file {}", path.display()))?;
        Ok(Some(overlay))
    }

    fn from_env() -> Self {
        let metrics_addr = env::var("METRICS_ADDR").ok();
        let allow_insecure_metrics_dev = env::var("ALLOW_INSECURE_METRICS_DEV")
            .ok()
            .and_then(|v| v.parse::<bool>().ok());
        let metrics_auth_token = env::var("METRICS_AUTH_TOKEN").ok();
        let metrics_tls_cert_path = env::var("METRICS_TLS_CERT_PATH").ok();
        let metrics_tls_key_path = env::var("METRICS_TLS_KEY_PATH").ok();
        let outbox_path = env::var("OUTBOX_PATH").ok();
        let outbox_dlq_path = env::var("OUTBOX_DLQ_PATH").ok();
        let outbox_db_path = env::var("OUTBOX_DB_PATH").ok();
        let idempotency_conflict_policy = env::var("IDEMPOTENCY_CONFLICT_POLICY")
            .ok()
            .and_then(|raw| IdempotencyConflictPolicy::from_str(&raw).ok());
        let error_budget = ErrorBudgetOverlay::from_env();
        let release_track = env::var("RELEASE_TRACK")
            .ok()
            .and_then(|raw| ReleaseTrack::from_str(&raw).ok());
        Self {
            metrics_addr,
            allow_insecure_metrics_dev,
            metrics_auth_token,
            metrics_tls_cert_path,
            metrics_tls_key_path,
            outbox_path,
            outbox_dlq_path,
            outbox_db_path,
            idempotency_conflict_policy,
            error_budget,
            release_track,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IdempotencyConflictPolicy {
    ReturnExisting,
    #[serde(alias = "conflict_409", alias = "conflict")]
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::tempdir;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    fn with_env<F: FnOnce()>(vars: &[(&str, Option<&str>)], f: F) {
        let _guard = ENV_MUTEX.lock().expect("env mutex");
        let snapshot: Vec<(String, Option<String>)> = vars
            .iter()
            .map(|(k, _)| (k.to_string(), env::var(k).ok()))
            .collect();
        for (key, value) in vars {
            match value {
                Some(val) => unsafe {
                    // SAFETY: tests run serially within helper and restore prior state.
                    env::set_var(key, val);
                },
                None => unsafe {
                    env::remove_var(key);
                },
            }
        }
        f();
        for (key, value) in snapshot {
            match value {
                Some(val) => unsafe {
                    env::set_var(&key, val);
                },
                None => unsafe {
                    env::remove_var(&key);
                },
            }
        }
    }

    #[test]
    fn load_from_dir_without_files_uses_defaults() -> Result<()> {
        let dir = tempdir()?;
        with_env(
            &[
                ("METRICS_ADDR", None),
                ("ALLOW_INSECURE_METRICS_DEV", None),
                ("METRICS_AUTH_TOKEN", None),
                ("METRICS_TLS_CERT_PATH", None),
                ("METRICS_TLS_KEY_PATH", None),
                ("OUTBOX_PATH", None),
                ("OUTBOX_DLQ_PATH", None),
                ("OUTBOX_DB_PATH", None),
                ("ERROR_BUDGET_ENABLED", None),
                ("ERROR_BUDGET_SUCCESS_THRESHOLD", None),
                ("ERROR_BUDGET_SAMPLE_WINDOW_SECS", None),
                ("ERROR_BUDGET_MIN_REQUESTS", None),
                ("ERROR_BUDGET_FREEZE_SECS", None),
                ("RELEASE_TRACK", None),
            ],
            || {
                let cfg = AppConfig::load_from_dir(dir.path()).expect("config load");
                assert!(cfg.metrics_addr.is_none());
                assert_eq!(
                    cfg.idempotency_conflict_policy,
                    IdempotencyConflictPolicy::Conflict409
                );
                let (main, dlq) = cfg.outbox_paths();
                assert_eq!(main, PathBuf::from("data/outbox/events.jsonl"));
                assert_eq!(dlq, PathBuf::from("data/outbox/dlq.jsonl"));
                assert!(cfg.error_budget.enabled);
                assert_eq!(cfg.error_budget.success_threshold, 0.99);
                assert_eq!(cfg.release_track, ReleaseTrack::Stable);
            },
        );
        Ok(())
    }

    #[test]
    fn load_merges_profile_local_and_env() -> Result<()> {
        let dir = tempdir()?;
        std::fs::write(
            dir.path().join("default.toml"),
            "metrics_addr = \"127.0.0.1:9999\"\n",
        )?;
        std::fs::write(
            dir.path().join("beta.toml"),
            "allow_insecure_metrics_dev = true\n",
        )?;
        std::fs::write(
            dir.path().join("local.toml"),
            "outbox_path = \"/tmp/outbox.jsonl\"\n",
        )?;

        with_env(
            &[
                (CONFIG_PROFILE_ENV, Some("beta")),
                ("METRICS_ADDR", Some("127.0.0.1:5555")),
                ("METRICS_AUTH_TOKEN", Some("secret")),
                ("METRICS_TLS_CERT_PATH", Some("/tmp/cert.pem")),
                ("METRICS_TLS_KEY_PATH", Some("/tmp/key.pem")),
                ("IDEMPOTENCY_CONFLICT_POLICY", Some("return_existing")),
                ("ERROR_BUDGET_ENABLED", Some("false")),
                ("ERROR_BUDGET_SUCCESS_THRESHOLD", Some("0.75")),
                ("RELEASE_TRACK", Some("canary")),
            ],
            || {
                let cfg = AppConfig::load_from_dir(dir.path()).expect("config load");
                assert_eq!(cfg.metrics_addr.as_deref(), Some("127.0.0.1:5555"));
                assert_eq!(cfg.metrics_auth_token.as_deref(), Some("secret"));
                assert_eq!(cfg.metrics_tls_cert_path.as_deref(), Some("/tmp/cert.pem"));
                assert_eq!(cfg.metrics_tls_key_path.as_deref(), Some("/tmp/key.pem"));
                assert_eq!(cfg.allow_insecure_metrics_dev, Some(true));
                assert_eq!(cfg.outbox_path.as_deref(), Some("/tmp/outbox.jsonl"));
                assert_eq!(
                    cfg.idempotency_conflict_policy,
                    IdempotencyConflictPolicy::ReturnExisting
                );
                assert!(!cfg.error_budget.enabled);
                assert_eq!(cfg.error_budget.success_threshold, 0.75);
                assert_eq!(cfg.release_track, ReleaseTrack::Canary);
            },
        );
        Ok(())
    }

    #[test]
    fn default_config_parses() -> Result<()> {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let config_dir = manifest_dir.join("config");
        let cfg = AppConfig::load_from_dir(&config_dir)?;
        assert_eq!(
            cfg.idempotency_conflict_policy,
            IdempotencyConflictPolicy::Conflict409
        );
        Ok(())
    }

    #[test]
    fn error_budget_overlay_merges_partials() {
        let mut settings = ErrorBudgetSettings::default();
        settings.apply_overlay(ErrorBudgetOverlay {
            enabled: Some(false),
            success_threshold: Some(0.9),
            sample_window_secs: None,
            minimum_requests: Some(10),
            freeze_window_secs: Some(60),
        });
        assert!(!settings.enabled);
        assert_eq!(settings.success_threshold, 0.9);
        assert_eq!(settings.minimum_requests, 10);
        assert_eq!(settings.freeze_window_secs, 60);
        assert_eq!(settings.sample_window_secs, 120);
    }

    #[test]
    fn metrics_server_config_requires_tls_pair() -> Result<()> {
        let dir = tempdir()?;
        std::fs::write(
            dir.path().join("default.toml"),
            "metrics_addr = \"0.0.0.0:9100\"\nallow_insecure_metrics_dev = false\n",
        )?;

        with_env(
            &[
                ("METRICS_TLS_CERT_PATH", Some("cert.pem")),
                ("METRICS_TLS_KEY_PATH", Some("key.pem")),
            ],
            || {
                let cfg = AppConfig::load_from_dir(dir.path()).expect("config load");
                let server_cfg = cfg.metrics_server_config().expect("metrics cfg");
                let server_cfg = server_cfg.expect("metrics enabled");
                assert_eq!(server_cfg.addr, "0.0.0.0:9100".parse().unwrap());
                assert!(server_cfg.tls.is_some());
            },
        );

        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorBudgetSettings {
    #[serde(default = "ErrorBudgetSettings::default_enabled")]
    pub enabled: bool,
    #[serde(default = "ErrorBudgetSettings::default_success_threshold")]
    pub success_threshold: f64,
    #[serde(default = "ErrorBudgetSettings::default_sample_window_secs")]
    pub sample_window_secs: u64,
    #[serde(default = "ErrorBudgetSettings::default_minimum_requests")]
    pub minimum_requests: u64,
    #[serde(default = "ErrorBudgetSettings::default_freeze_window_secs")]
    pub freeze_window_secs: u64,
}

impl Default for ErrorBudgetSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            success_threshold: 0.99,
            sample_window_secs: 120,
            minimum_requests: 20,
            freeze_window_secs: 300,
        }
    }
}

impl ErrorBudgetSettings {
    const fn default_enabled() -> bool {
        true
    }
    const fn default_success_threshold() -> f64 {
        0.99
    }
    const fn default_sample_window_secs() -> u64 {
        120
    }
    const fn default_minimum_requests() -> u64 {
        20
    }
    const fn default_freeze_window_secs() -> u64 {
        300
    }

    fn apply_overlay(&mut self, overlay: ErrorBudgetOverlay) {
        if let Some(value) = overlay.enabled {
            self.enabled = value;
        }
        if let Some(value) = overlay.success_threshold {
            self.success_threshold = value;
        }
        if let Some(value) = overlay.sample_window_secs {
            self.sample_window_secs = value;
        }
        if let Some(value) = overlay.minimum_requests {
            self.minimum_requests = value;
        }
        if let Some(value) = overlay.freeze_window_secs {
            self.freeze_window_secs = value;
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ErrorBudgetOverlay {
    enabled: Option<bool>,
    success_threshold: Option<f64>,
    sample_window_secs: Option<u64>,
    minimum_requests: Option<u64>,
    freeze_window_secs: Option<u64>,
}

impl ErrorBudgetOverlay {
    fn from_env() -> Option<Self> {
        let mut overlay = Self::default();
        let mut seen = false;

        if let Some(value) = env::var("ERROR_BUDGET_ENABLED")
            .ok()
            .and_then(|v| v.parse::<bool>().ok())
        {
            overlay.enabled = Some(value);
            seen = true;
        }
        if let Some(value) = env::var("ERROR_BUDGET_SUCCESS_THRESHOLD")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
        {
            overlay.success_threshold = Some(value);
            seen = true;
        }
        if let Some(value) = env::var("ERROR_BUDGET_SAMPLE_WINDOW_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
        {
            overlay.sample_window_secs = Some(value);
            seen = true;
        }
        if let Some(value) = env::var("ERROR_BUDGET_MIN_REQUESTS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
        {
            overlay.minimum_requests = Some(value);
            seen = true;
        }
        if let Some(value) = env::var("ERROR_BUDGET_FREEZE_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
        {
            overlay.freeze_window_secs = Some(value);
            seen = true;
        }

        if seen { Some(overlay) } else { None }
    }
}
