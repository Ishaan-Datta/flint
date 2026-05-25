use std::{path::Path, process::exit, time::Duration};

use crate::{
    ast::write::{handle_dirty_file_status, run_git_file_status},
    command::with_command_spinner,
    errors::{CommandError, WriteError},
    modified_time::{Input, InputStatus, print_summary_message},
};

const UPDATE_INPUTS_CMD: &str = r"nix flake update {INPUTS} --flake {PATH}";

/// Auto updates all stale inputs with `nix flake update`.
///
/// # Arguments
///
/// * `inputs` - The list of inputs and their current status.
/// * `timeout` - Maximum time allowed for the update command.
/// * `quiet` - Whether to suppress prompts when checking for modifications.
/// * `override_bool` - Skip modification checks when true.
/// * `flake_dir_path` - Path to the flake directory containing `flake.lock`.
///
/// # Returns
///
/// Returns `Ok(())` when the update completes successfully.
/// Exits the process with status 0 when there are no stale inputs.
/// Returns a `WriteError` if the update command fails or file modification
/// checks fail.
///
/// # Errors
///
/// Returns an error if the existing `flake.lock` changes are rejected or if
/// the `nix flake update` command exits non-zero.
pub(crate) fn update_stale_flake_inputs(
    inputs: &[Input],
    timeout: Duration,
    quiet: bool,
    override_bool: bool,
    flake_dir_path: &Path,
) -> Result<(), WriteError> {
    tracing::info!("");

    let stale_inputs = inputs
        .iter()
        .filter_map(|input| {
            match input.status {
                InputStatus::Stale => Some(input.name.clone()),
                _ => None,
            }
        })
        .collect::<Vec<String>>();

    if stale_inputs.is_empty() {
        tracing::info!("All inputs are up-to-date.");
        exit(0);
    }

    let start_time = std::time::Instant::now();
    tracing::info!("Auto-updating stale flake inputs");

    let flake_lock_file = flake_dir_path.join("flake.lock");
    if flake_lock_file.exists() && flake_lock_file.is_file() && !override_bool {
        let status =
            run_git_file_status(&flake_lock_file, "flake.lock", timeout)?;
        handle_dirty_file_status(status, &flake_lock_file, quiet)?;
    }

    let cmd = UPDATE_INPUTS_CMD
        .replace("{INPUTS}", &stale_inputs.join(" "))
        .replace("{PATH}", &flake_dir_path.display().to_string());
    let output =
        with_command_spinner!("Updating stale flake inputs", &cmd, timeout)?;

    print_summary_message(start_time);

    if output.status.success() {
        tracing::info!("\nSuccessfully updated stale flake inputs\n");
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
