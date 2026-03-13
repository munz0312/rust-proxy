use hyper::server::conn::http1;
use hyper_util::rt::TokioIo;
use std::sync::{Arc, atomic::AtomicUsize};
use tokio::net::TcpListener;

mod backend;
mod config;
mod connection_pool;
mod health;
mod load_balancer;
mod proxy;

use backend::Backend;
use config::ProxyConfig;
use load_balancer::RoundRobin;
use proxy::Proxy;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = ProxyConfig::from_file("config.toml")?;

    let backends: Vec<Backend> = config
        .backends
        .servers
        .iter()
        .map(|&addr| {
            Backend::new(
                addr,
                config.health_check.failure_threshold,
                config.health_check.recovery_threshold,
            )
        })
        .collect();

    let load_balancer = Arc::new(RoundRobin {
        backends: Arc::new(backends),
        index: AtomicUsize::new(0),
    });

    let backends = load_balancer.backends.clone();
    let proxy = Proxy::new(load_balancer);

    let listener = TcpListener::bind(config.listen).await?;
    println!("Proxy listening on {}", config.listen);

    tokio::spawn(health::run_health_checks(
        backends,
        config.health_check.interval_secs as u64,
        config.health_check.path.clone(),
    ));

    loop {
        let (stream, addr) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let mut proxy = proxy.clone();

        proxy.client_addr = Some(addr);
        tokio::spawn(async move {
            if let Err(err) = http1::Builder::new().serve_connection(io, proxy).await {
                eprintln!("connection error: {}", err);
            }
        });
    }
}
