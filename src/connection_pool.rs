use std::{collections::HashMap, net::SocketAddr, sync::Mutex};

use hyper::{
    body::Incoming,
    client::conn::http1::{self, SendRequest},
};
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;

pub struct Pool {
    conns: Mutex<HashMap<SocketAddr, Vec<SendRequest<Incoming>>>>,
}

impl Pool {
    pub fn new() -> Pool {
        Self {
            conns: Mutex::new(HashMap::new()),
        }
    }

    pub async fn acquire(
        &self,
        addr: SocketAddr,
    ) -> Result<SendRequest<Incoming>, Box<dyn std::error::Error + Send + Sync>> {
        loop {
            let sender = self
                .conns
                .lock()
                .unwrap()
                .get_mut(&addr)
                .and_then(|vec| vec.pop());

            match sender {
                Some(mut s) => {
                    if s.ready().await.is_ok() {
                        return Ok(s);
                    }
                }
                None => break,
            }
        }

        let stream = TcpStream::connect(addr).await?;
        let io = TokioIo::new(stream);
        let (sender, conn) = http1::handshake(io).await?;
        tokio::spawn(conn);
        Ok(sender)
    }

    pub fn release(&self, addr: SocketAddr, sender: SendRequest<Incoming>) {
        self.conns
            .lock()
            .unwrap()
            .entry(addr)
            .or_default()
            .push(sender);
    }
}
