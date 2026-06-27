use std::{collections::HashMap, path::Path, time::Duration};

use crate::{
    command::run_command_with_timeout,
    errors::{CommandError, FetchError},
    modified_time::RemoteInput,
};

const URL_CMD: &str = r"nix eval --json --impure --expr '  
builtins.mapAttrs (_: v: v.url or null) ((import {PATH}).inputs)  
'
";

/// Get URL-backed inputs declared in `flake.nix`.
///
/// Evaluates the flake's `inputs` attribute and extracts each input's `url`
/// field.
///
/// # Arguments
///
/// * `timeout` - Maximum time allowed for the Nix evaluation.
/// * `flake_dir_path` - Path to the flake directory.
///
/// # Returns
///
/// Returns a vector of `RemoteInput` values containing each input name and URL.
///
/// # Errors
///
/// Returns `FetchError` if the Nix command fails, exits with a non-zero status,
/// or returns JSON that cannot be parsed into the expected input URL map.
pub fn get_input_urls(
    timeout: Duration,
    flake_dir_path: &Path,
) -> Result<Vec<RemoteInput>, FetchError> {
    let flake_path = flake_dir_path.join("flake.nix");
    let cmd = URL_CMD.replace("{PATH}", &flake_path.display().to_string());

    let output = run_command_with_timeout(&cmd, timeout)?;

    if output.status.success() {
        let mut inputs = Vec::<RemoteInput>::new();
        let url_map: HashMap<String, String> =
            serde_json::from_slice(&output.stdout)?;

        for (input_name, input_url) in url_map {
            inputs.push(RemoteInput {
                input_name,
                input_url,
            });
        }

        tracing::trace!(
            "Successfully fetched {} flake input urls",
            inputs.len()
        );

        Ok(inputs)
    } else {
        let stdout_str = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();
        let code = output.status.code().unwrap_or(1);
        Err(FetchError::CommandError(CommandError::NonZeroExitCode(
            code, stderr_str, stdout_str,
        )))
    }
}
