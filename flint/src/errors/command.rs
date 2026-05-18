use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum CommandError {
    #[error("Command execution exceeded {0}ms")]
    CommandTimeout(u128),
    #[error(
        "Command exited with non-zero status code: {0}{stderr}{stdout}",
        stderr = format_output("stderr", .1),
        stdout = format_output("stdout", .2),
    )]
    NonZeroExitCode(i32, String, String),
    #[error(transparent)]
    Other(#[from] Arc<anyhow::Error>),
}

fn format_output(label: &str, value: &str) -> String {
    if value.is_empty() {
        String::new()
    } else {
        format!("\n{label}: {value}")
    }
}

impl From<anyhow::Error> for CommandError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(Arc::new(err))
    }
}
