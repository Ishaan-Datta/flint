use std::{path::PathBuf, time::Duration};

use crate::{
  command::with_command_spinner,
  errors::{CommandError, FetchError},
};

const PATH_CMD: &str = r#"nix flake metadata --json --no-write-lock-file {PATH} \
  | jq -er '
      (.resolved.url // error("nix flake metadata returned null for .resolved.url")) as $url
      | ($url | sub("^file://"; "")) as $root
      | (.resolved.dir // "") as $dir
      | if $dir == "" then $root else $root + "/" + $dir end
    '
"#;

/// Get the flake path resolved from `nix flake metadata`.
///
/// # Arguments
///
/// * `input_path` - Path or URL used to resolve the flake.
/// * `timeout` - Maximum time allowed for the metadata command.
///
/// # Returns
///
/// Returns the resolved flake root path.
///
/// # Errors
///
/// Returns an error if the metadata command fails or the resolved path is not
/// a directory containing `flake.nix`.
pub fn get_flake_path(
  input_path: &str,
  timeout: Duration,
) -> Result<PathBuf, FetchError> {
  let quoted_path = shlex::try_quote(input_path)
    .map_err(|_| FetchError::InvalidPath(PathBuf::from(input_path)))?;

  let cmd = PATH_CMD.replace("{PATH}", quoted_path.as_ref());
  let output = with_command_spinner!(
    "Resolving the flake path with `nix flake metadata`",
    &cmd,
    timeout
  )?;

  let stdout_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
  let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();

  if output.status.success() {
    tracing::debug!("Stdout: {stdout_str}");

    if !stderr_str.is_empty() {
      tracing::debug!("Stderr: {stderr_str}");
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
