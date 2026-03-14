use serde::Deserialize;
use std::fs::read_to_string;
use std::net::SocketAddr;

#[derive(Deserialize)]
pub struct ProxyConfig {
    pub listen: SocketAddr,
    pub timeouts: TimeoutConfig,
    pub health_check: HealthCheckConfig,
    pub backends: BackendConfig,
}

#[derive(Deserialize, Clone)]
pub struct TimeoutConfig {
    pub connect_ms: u32,
    pub read_ms: u32,
    pub write_ms: u32,
}

#[derive(Deserialize)]
pub struct HealthCheckConfig {
    pub interval_secs: u32,
    pub failure_threshold: u32,
    pub recovery_threshold: u32,
    pub path: String,
}

#[derive(Deserialize)]
pub struct BackendConfig {
    pub servers: Vec<SocketAddr>,
}

impl ProxyConfig {
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let contents = read_to_string(path)?;
        let config: ProxyConfig = toml::from_str(&contents)?;
        Ok(config)
    }
}
