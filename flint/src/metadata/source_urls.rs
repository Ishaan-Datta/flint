use std::{collections::HashMap, path::Path, time::Duration};

use crate::{
    command::run_command_with_timeout,
    errors::{CommandError, FetchError},
};

const URL_CMD: &str = r"nix eval --json --impure --expr '  
builtins.mapAttrs (_: v: v.url or null) ((import {PATH}).inputs)  
'
";

/// Get the URL for each `flake.nix` input.
///
/// # Arguments
///
/// * `timeout` - Maximum time allowed for the Nix evaluation.
/// * `flake_dir_path` - Path to the flake directory containing `flake.nix`.
///
/// # Returns
///
/// Returns a map of input names to their resolved URL strings.
///
/// # Errors
///
/// Returns an error if the Nix command fails or the JSON output cannot be
/// parsed.
pub fn get_input_urls(
    timeout: Duration,
    flake_dir_path: &Path,
) -> Result<HashMap<String, String>, FetchError> {
    let flake_path = flake_dir_path.join("flake.nix");
    let cmd = URL_CMD.replace("{PATH}", &flake_path.display().to_string());

    let output = run_command_with_timeout(&cmd, timeout)?;

    if output.status.success() {
        let url_map: HashMap<String, String> =
            serde_json::from_slice(&output.stdout)?;
        tracing::trace!(
            "Successfully fetched {} flake input urls",
            url_map.len()
        );
        Ok(url_map)
    } else {
        let stdout_str = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();
        let code = output.status.code().unwrap_or(1);
        Err(FetchError::CommandError(CommandError::NonZeroExitCode(
            code, stderr_str, stdout_str,
        )))
    }
}
