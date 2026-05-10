use crate::command::run_command_with_timeout;
use crate::errors::CommandError;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

const URL_CMD: &str = r#"nix eval --json --impure --expr '  
builtins.mapAttrs (_: v: v.url or null) ((import ./flake.nix).inputs)  
'
"#;

#[derive(Error, Debug, Clone)]
pub enum ParseError {
    #[error("Failed to parse input map: {0}")]
    InputMapParse(#[from] Arc<serde_json::Error>),
    #[error("Command failed to run: {0}")]
    CommandExecution(#[from] Arc<anyhow::Error>),
    #[error("Command exitted with non-zero status code: {0}\nstderr: {1}\nstdout: {2}\n")]
    NonZeroExitCode(i32, String, String),
    #[error(transparent)]
    CommandError(#[from] CommandError),
}

impl From<serde_json::Error> for ParseError {
    fn from(err: serde_json::Error) -> Self {
        ParseError::InputMapParse(Arc::new(err))
    }
}

/// Get the URL for each flake.nix input
pub fn get_input_urls(timeout: Duration) -> Result<HashMap<String, String>, ParseError> {
    let output = run_command_with_timeout(URL_CMD.to_string(), timeout)?;

    if output.status.success() {
        let url_map: HashMap<String, String> = serde_json::from_slice(&output.stdout)?;
        tracing::trace!("Successfully fetched {} flake input urls", url_map.len());
        return Ok(url_map);
    } else {
        let stdout_str = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();
        let code = output.status.code().unwrap_or(1);
        Err(ParseError::CommandError(CommandError::NonZeroExitCode(
            code, stderr_str, stdout_str,
        )))
    }
}
