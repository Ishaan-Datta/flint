use std::sync::Arc;

use thiserror::Error;

use crate::errors::CommandError;

#[derive(Error, Debug, Clone)]
pub enum WriteError {
    #[error(transparent)]
    CommandError(#[from] CommandError),
    #[error(transparent)]
    IoError(#[from] Arc<std::io::Error>),
    #[error(transparent)]
    InvalidPromptInput(#[from] Arc<inquire::InquireError>),
    #[error("Aborting flake update operation based on user input")]
    AbortUserInput,
}

impl From<std::io::Error> for WriteError {
    fn from(err: std::io::Error) -> Self {
        Self::IoError(Arc::new(err))
    }
}

impl From<inquire::InquireError> for WriteError {
    fn from(err: inquire::InquireError) -> Self {
        Self::InvalidPromptInput(Arc::new(err))
    }
}
