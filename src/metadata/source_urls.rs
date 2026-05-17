use crate::command::run_command_with_timeout;
use crate::errors::CommandError;
use crate::errors::FetchError;

use std::collections::HashMap;
use std::time::Duration;

// TODO: make this path change...
const URL_CMD: &str = r#"nix eval --json --impure --expr '  
builtins.mapAttrs (_: v: v.url or null) ((import ./flake.nix).inputs)  
'
"#;

/// Get the URL for each flake.nix input
pub(crate) fn get_input_urls(timeout: Duration) -> Result<HashMap<String, String>, FetchError> {
    let output = run_command_with_timeout(URL_CMD.to_string(), timeout)?;

    if output.status.success() {
        let url_map: HashMap<String, String> = serde_json::from_slice(&output.stdout)?;
        tracing::trace!("Successfully fetched {} flake input urls", url_map.len());
        return Ok(url_map);
    } else {
        let stdout_str = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();
        let code = output.status.code().unwrap_or(1);
        Err(FetchError::CommandError(CommandError::NonZeroExitCode(
            code, stderr_str, stdout_str,
        )))
    }
}
