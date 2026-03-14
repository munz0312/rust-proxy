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
};

#[derive(Clone)]
pub struct Proxy {
    load_balancer: Arc<RoundRobin>,
    pool: Arc<Pool>,
    pub client_addr: Option<SocketAddr>,
    timeouts: TimeoutConfig,
}

impl Proxy {
    pub fn new(load_balancer: Arc<RoundRobin>, timeouts: TimeoutConfig) -> Self {
        Self {
            load_balancer,
            pool: Arc::new(Pool::new()),
            client_addr: None,
            timeouts,
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
        Box::pin(async move {
            let result: Result<Self::Response, Self::Error> = async {
                let backend = lb.next_backend().ok_or(ProxyError::NoBackends)?;
                let mut sender = pool
                    .acquire(backend.addr, timeout_config.connect_ms.into())
                    .await?;

                *req.uri_mut() = req.uri().path().parse()?;

                let headers = req.headers_mut();
                headers.insert("Host", HeaderValue::from_str(&backend.addr.to_string())?);
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

            match result {
                Ok(res) => Ok(res),
                Err(ProxyError::NoBackends) => Ok(Response::builder()
                    .status(503)
                    .body(full("Service Unavailable"))
                    .unwrap()),

                Err(ProxyError::Timeout) => Ok(Response::builder()
                    .status(504)
                    .body(full("Timeout"))
                    .unwrap()),

                Err(e) => Ok(Response::builder()
                    .status(502)
                    .body(full(format!("Bad Gateway: {}", e)))
                    .unwrap()),
            }
        })
    }
}
