use http_body_util::{BodyExt, combinators::BoxBody};
use hyper::{
    Request, Response,
    body::{Bytes, Incoming},
    header::HeaderValue,
    service::Service,
};
use rust_proxy::full;
use std::{future::Future, net::SocketAddr, pin::Pin, sync::Arc};

use crate::{
    config::TimeoutConfig,
    connection_pool::Pool,
    error::ProxyError,
    load_balancer::{LoadBalancer, RoundRobin},
    metrics::Metrics,
};

#[derive(Clone)]
pub struct Proxy {
    load_balancer: Arc<RoundRobin>,
    pool: Arc<Pool>,
    pub client_addr: Option<SocketAddr>,
    timeouts: TimeoutConfig,
    metrics: Metrics,
}

impl Proxy {
    pub fn new(load_balancer: Arc<RoundRobin>, timeouts: TimeoutConfig, metrics: Metrics) -> Self {
        Self {
            load_balancer,
            pool: Arc::new(Pool::new()),
            client_addr: None,
            timeouts,
            metrics,
        }
    }
}

type Req = Request<Incoming>;

impl Service<Req> for Proxy {
    type Response = Response<BoxBody<Bytes, hyper::Error>>;
    type Error = ProxyError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, mut req: Req) -> Self::Future {
        let lb = self.load_balancer.clone();
        let pool = self.pool.clone();
        let client_ip = self.client_addr.unwrap().ip().to_string();
        let timeout_config = self.timeouts.clone();
        let metrics = self.metrics.clone();
        Box::pin(async move {
            if req.uri().path() == "/metrics" {
                let body = metrics.encode();
                return Ok(Response::builder()
                    .status(200)
                    .header("Content-Type", "text/plain; charset=utf-8")
                    .body(full(body))
                    .unwrap());
            }

            metrics.requests_total.inc();
            metrics.active_connections.inc();
            let timer = metrics.request_duration_seconds.start_timer();

            let backend = lb.next_backend();
            let backend_label = backend.map(|b| b.addr.to_string());

            let result: Result<Self::Response, Self::Error> = async {
                let backend = backend.ok_or(ProxyError::NoBackends)?;
                let mut sender = pool
                    .acquire(backend.addr, timeout_config.connect_ms.into())
                    .await?;

                *req.uri_mut() = req
                    .uri()
                    .path_and_query()
                    .map(|path| path.as_str())
                    .unwrap_or("/")
                    .parse()?;

                let backend_addr = backend.addr.to_string();
                let headers = req.headers_mut();
                headers.insert("Host", HeaderValue::from_str(&backend_addr)?);
                headers.insert("X-Real-IP", HeaderValue::from_str(&client_ip)?);
                headers.insert("X-Forwarded-Proto", HeaderValue::from_static("http"));

                let xff = match headers.get("X-Forwarded-For") {
                    Some(existing) => format!("{}, {}", existing.to_str()?, client_ip),
                    None => client_ip,
                };
                headers.insert("X-Forwarded-For", HeaderValue::from_str(&xff)?);

                let response = tokio::time::timeout(
                    std::time::Duration::from_millis(timeout_config.read_ms.into()),
                    sender.send_request(req),
                )
                .await
                .map_err(|_| ProxyError::Timeout)??;
                pool.release(backend.addr, sender);
                Ok(response.map(|body| body.boxed()))
            }
            .await;

            timer.observe_duration();
            metrics.active_connections.dec();

            match result {
                Ok(res) => Ok(res),
                Err(ProxyError::NoBackends) => {
                    metrics.requests_failed.inc();
                    Ok(Response::builder()
                        .status(503)
                        .body(full("Service Unavailable"))
                        .unwrap())
                }

                Err(ProxyError::Timeout) => {
                    metrics.requests_failed.inc();
                    if let Some(label) = &backend_label {
                        metrics.backend_errors.with_label_values(&[label]).inc();
                    }
                    Ok(Response::builder()
                        .status(504)
                        .body(full("Timeout"))
                        .unwrap())
                }

                Err(e) => {
                    metrics.requests_failed.inc();
                    if let Some(label) = &backend_label {
                        metrics.backend_errors.with_label_values(&[label]).inc();
                    }
                    Ok(Response::builder()
                        .status(502)
                        .body(full(format!("Bad Gateway: {}", e)))
                        .unwrap())
                }
            }
        })
    }
}
