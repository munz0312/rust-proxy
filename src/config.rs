use serde::Deserialize;
use std::fs::read_to_string;
use std::net::SocketAddr;

#[derive(Deserialize)]
pub struct ProxyConfig {
    listen: SocketAddr,
    timeouts: TimeoutConfig,
    health_check: HealthCheckConfig,
    backends: BackendConfig,
}

#[derive(Deserialize)]
pub struct TimeoutConfig {
    connect_ms: u32,
    read_ms: u32,
    write_ms: u32,
}

#[derive(Deserialize)]
pub struct HealthCheckConfig {
    interval_secs: u32,
    failure_threshold: u32,
    recovery_threshold: u32,
    path: String,
}

#[derive(Deserialize)]
pub struct BackendConfig {
    servers: Vec<SocketAddr>,
}

impl ProxyConfig {
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = read_to_string(path)?;
        let config: ProxyConfig = toml::from_str(&contents)?;
        Ok(config)
    }
}
