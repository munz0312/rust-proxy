use std::{future::Future, pin::Pin, sync::Arc};

use http_body_util::{BodyExt, combinators::BoxBody};
use hyper::{
    Request, Response,
    body::{Bytes, Incoming},
    client::conn::http1,
    service::Service,
};
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;

use crate::{
    connection_pool::Pool,
    load_balancer::{LoadBalancer, RoundRobin},
};

#[derive(Clone)]
pub struct Proxy {
    load_balancer: Arc<RoundRobin>,
    pool: Arc<Pool>,
}

impl Proxy {
    pub fn new(load_balancer: Arc<RoundRobin>) -> Self {
        Self {
            load_balancer,
            pool: Arc::new(Pool::new()),
        }
    }
}

type Req = Request<Incoming>;

impl Service<Req> for Proxy {
    type Response = Response<BoxBody<Bytes, hyper::Error>>;
    type Error = Box<dyn std::error::Error + Send + Sync>;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, req: Req) -> Self::Future {
        let lb = self.load_balancer.clone();
        let pool = self.pool.clone();
        Box::pin(async move {
            let backend = lb.next_backend().ok_or("no healthy backends")?;
            let mut sender = pool.acquire(backend.addr).await?;

            let outbound = Request::builder()
                .method(req.method())
                .uri(req.uri().path())
                .header("Host", backend.addr.to_string())
                .body(req.into_body())?;

            let response = sender.send_request(outbound).await?;
            pool.release(backend.addr, sender);
            Ok(response.map(|body| body.boxed()))
        })
    }
}
