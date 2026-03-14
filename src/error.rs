use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProxyError {
    #[error("no healthy backends")]
    NoBackends,

    #[error("{0}")]
    Backend(#[from] hyper::Error),

    #[error("{0}")]
    Http(#[from] hyper::http::Error),

    #[error("{0}")]
    InvalidHeaderValue(#[from] hyper::header::InvalidHeaderValue),

    #[error("{0}")]
    InvalidHeaderStr(#[from] hyper::header::ToStrError),

    #[error("{0}")]
    InvalidUri(#[from] hyper::http::uri::InvalidUri),

    #[error("{0}")]
    Pool(Box<dyn std::error::Error + Send + Sync>),
}
