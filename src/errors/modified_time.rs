use crate::errors::command::CommandError;
use std::num::ParseIntError;
use std::string::FromUtf8Error;
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum FetchError {
    #[error("Fetch operation timed out")]
    FetchTimeout,
    #[error("Failed to parse last modified time from command output: {0}")]
    IntParse(#[from] ParseIntError),
    #[error("Failed to read stdout from command: {0}")]
    StdParse(#[from] FromUtf8Error),
    #[error("Command failed to run: {0}")]
    CommandExecution(#[from] Arc<anyhow::Error>),
    #[error(transparent)]
    CommandError(#[from] CommandError),
}

impl From<anyhow::Error> for FetchError {
    fn from(err: anyhow::Error) -> Self {
        FetchError::CommandExecution(Arc::new(err))
    }
}
