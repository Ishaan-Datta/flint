use crate::errors::CommandError;

use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum WriteError {
    #[error(transparent)]
    CommandError(#[from] CommandError),
    #[error(transparent)]
    IoError(#[from] Arc<std::io::Error>),
    #[error(transparent)]
    InvalidPromptInput(#[from] Arc<inquire::InquireError>),
    #[error("Aborting flake rename operation based on user input")]
    AbortUserInput,
}

impl From<std::io::Error> for WriteError {
    fn from(err: std::io::Error) -> Self {
        WriteError::IoError(Arc::new(err))
    }
}

impl From<inquire::InquireError> for WriteError {
    fn from(err: inquire::InquireError) -> Self {
        WriteError::InvalidPromptInput(Arc::new(err))
    }
}
