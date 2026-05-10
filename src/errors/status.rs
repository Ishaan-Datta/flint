use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum StatusError {
    #[error("Time travel detected (The local input source is more recent than the remote)")]
    Less,
    #[error("Could not fetch the flake metadata for the input source: {0}")]
    NotFetched(String),
}
