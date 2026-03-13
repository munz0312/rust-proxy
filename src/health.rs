use std::{sync::Arc, time::Duration};

use http_body_util::Empty;
use hyper::{
    Request, Response,
    body::{Bytes, Incoming},
    client::conn::http1,
};
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;

use crate::backend::Backend;

pub async fn run_health_checks(
    backends: Arc<Vec<Backend>>,
    interval_secs: u64,
    path: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));
    loop {
        ticker.tick().await;
        for backend in backends.iter() {
            let response = check_backend(backend, &path).await;
            if response.is_ok() {
                backend.record_success();
                eprintln!("[health] {} ok", backend.addr);
            } else {
                backend.record_failure();
                eprintln!(
                    "[health] {} FAILED (healthy={})",
                    backend.addr,
                    backend.is_healthy()
                );
            }
        }
    }
}

async fn check_backend(
    backend: &Backend,
    path: &str,
) -> Result<Response<Incoming>, Box<dyn std::error::Error + Send + Sync>> {
    let stream = TcpStream::connect(backend.addr).await?;
    let io = TokioIo::new(stream);
    let (mut sender, conn) = http1::handshake(io).await?;
    tokio::spawn(conn);
    let outbound = Request::builder()
        .method("GET")
        .uri(path)
        .header("Host", backend.addr.to_string())
        .body(Empty::<Bytes>::new())?;

    let response = sender.send_request(outbound).await?;
    if response.status().is_success() {
        return Ok(response);
    }
    Err("not 2XX status".into())
}
