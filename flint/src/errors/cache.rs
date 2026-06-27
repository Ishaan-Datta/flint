use thiserror::Error;

#[derive(Error, Debug)]
pub enum CacheError {
    #[error("Cache does not exist")]
    CacheDNE,
    #[error("Cache expired")]
    CacheExpired,
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    JsonParseError(#[from] serde_json::Error),
}
