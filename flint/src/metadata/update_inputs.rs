use crate::ast::check_existing_file_modifications;
use crate::command::with_command_spinner;
use crate::errors::CommandError;
use crate::errors::WriteError;
use crate::modified_time::Input;
use crate::modified_time::InputStatus;
use crate::modified_time::print_summary_message;

use std::path::Path;
use std::process::exit;
use std::time::Duration;

const UPDATE_INPUTS_CMD: &str = r"nix flake update {INPUTS}";

/// Auto updates all stale inputs with `nix flake update`
pub(crate) fn update_stale_flake_inputs(
    inputs: &[Input],
    timeout: Duration,
    quiet: bool,
    override_bool: bool,
    flake_dir_path: &Path,
) -> Result<(), WriteError> {
    let stale_inputs = inputs
        .iter()
        .filter_map(|input| match input.status {
            InputStatus::Stale => Some(input.name.clone()),
            _ => None,
        })
        .collect::<Vec<String>>();

    if stale_inputs.is_empty() {
        tracing::info!("> All inputs are up-to-date.");
        exit(0);
    }

    let start_time = std::time::Instant::now();
    tracing::info!("> Auto-updating stale flake inputs");

    let flake_lock_file = flake_dir_path.join("flake.lock");
    if flake_lock_file.exists() && flake_lock_file.is_file() && !override_bool {
        check_existing_file_modifications(&flake_lock_file, "flake.lock", quiet, timeout)?;
    }

    let cmd = UPDATE_INPUTS_CMD.replace("{INPUTS}", &stale_inputs.join(" "));
    let output = with_command_spinner!("Updating stale flake inputs", &cmd, timeout)?;

    print_summary_message(start_time);

    if output.status.success() {
        Ok(())
    } else {
        let stdout_str = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();
        let code = output.status.code().unwrap_or(1);
        Err(WriteError::CommandError(CommandError::NonZeroExitCode(
            code, stderr_str, stdout_str,
        )))
    }
}
