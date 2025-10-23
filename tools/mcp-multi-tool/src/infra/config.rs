use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub metrics_addr: Option<String>,
    pub allow_insecure_metrics_dev: Option<bool>,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let metrics_addr = std::env::var("METRICS_ADDR").ok();
        let allow_insecure_metrics_dev = std::env::var("ALLOW_INSECURE_METRICS_DEV")
            .ok()
            .and_then(|v| v.parse::<bool>().ok());
        Self {
            metrics_addr,
            allow_insecure_metrics_dev,
        }
    }
}
