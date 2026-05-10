use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum CommandError {
    #[error("Command execution exceeded {0}ms")]
    CommandTimeout(u128),
    #[error("Command exitted with non-zero status code: {0}\nstderr: {1}\nstdout: {2}\n")]
    NonZeroExitCode(i32, String, String),
    #[error(transparent)]
    Other(#[from] Arc<anyhow::Error>),
}

impl From<anyhow::Error> for CommandError {
    fn from(err: anyhow::Error) -> Self {
        CommandError::Other(Arc::new(err))
    }
}
