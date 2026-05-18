use crate::command::with_command_spinner;
use crate::errors::CommandError;
use crate::errors::FetchError;

use std::path::PathBuf;
use std::time::Duration;

const PATH_CMD: &str = r#"nix flake metadata --json --no-write-lock-file {PATH} \
    | jq -er '
    (.resolved.url | sub("^file://"; "")) as $root
    | (.resolved.dir // "") as $dir
    | if $dir == "" then $root else $root + "/" + $dir end
    '
"#;

/// Get the flake path resolved from `nix flake metadata` command
pub fn get_flake_path(input_path: &str, timeout: Duration) -> Result<PathBuf, FetchError> {
    let cmd = PATH_CMD.replace("{PATH}", input_path);
    let output = with_command_spinner!(
        "Resolving the flake path with `nix flake metadata`",
        &cmd,
        timeout
    )?;

    let stdout_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        if !stderr_str.trim().is_empty() {
            tracing::warn!("{stderr_str}");
        }

        let flake_path = PathBuf::from(stdout_str);

        if !flake_path.exists() || !flake_path.is_dir() {
            return Err(FetchError::InvalidPath(flake_path));
        }

        let flake_file_path = flake_path.join("flake.nix");
        if !flake_file_path.exists() || !flake_file_path.is_file() {
            return Err(FetchError::InvalidPath(flake_path));
        }

        Ok(flake_path)
    } else {
        let code = output.status.code().unwrap_or(1);
        Err(FetchError::CommandError(CommandError::NonZeroExitCode(
            code, stderr_str, stdout_str,
        )))
    }
}
