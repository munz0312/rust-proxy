use std::{pin::Pin, sync::Arc};

use http_body_util::{BodyExt, combinators::BoxBody};
use hyper::{
    Request, Response,
    body::{Bytes, Incoming},
    service::Service,
};
use hyper_util::client::legacy::{Client, connect::HttpConnector};

use crate::load_balancer::{LoadBalancer, RoundRobin};

pub struct Proxy {
    client: Client<HttpConnector, Incoming>,
    load_balancer: Arc<RoundRobin>,
}

impl Clone for Proxy {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            load_balancer: self.load_balancer.clone(),
        }
    }
}

impl Proxy {
    pub fn new(load_balancer: Arc<RoundRobin>) -> Self {
        let client = Client::builder(hyper_util::rt::TokioExecutor::new()).build_http();
        Self {
            client,
            load_balancer,
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
        let client = self.client.clone();
        Box::pin(async move {
            let backend = lb.next_backend().ok_or("no healthy backends")?;
            let request: Request<Incoming> = Request::builder()
                .method(req.method())
                .uri(format!("http://{}{}", backend.addr, req.uri().path()))
                .body(req.into_body())
                .unwrap();
            let response = client.request(request).await?;
            Ok(response.map(|body| body.boxed()))
        })
    }
}
