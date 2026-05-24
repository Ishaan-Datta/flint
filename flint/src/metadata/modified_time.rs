use std::{
    cmp,
    collections::HashMap,
    path::{Path, PathBuf},
    process::exit,
    time::Duration,
};

use indicatif::ProgressStyle;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use tracing::info_span;
use tracing_indicatif::span_ext::IndicatifSpanExt;
use unicode_width::UnicodeWidthStr;
use yansi::{Paint, Painted};

use crate::{
    command::run_command_with_timeout,
    errors::{CommandError, FetchError},
    metadata::{get_input_urls, update_inputs::update_stale_flake_inputs},
    modified_time::{
        Input,
        InputStatus,
        format_status_line,
        print_summary_message,
    },
};

const REMOTE_MODIFIED_TIME_CMD: &str = r"nix --refresh flake metadata {URL} --json --no-write-lock-file | jq -er '.lastModified'";
const LOCAL_MODIFIED_TIME_CMD: &str = r"nix flake metadata --json --no-write-lock-file {PATH} | jq -er '.locks.nodes | map_values(.locked.lastModified)'";

/// Fetch the remote modified time for a single flake URL.
///
/// # Arguments
///
/// * `url` - The flake input URL to query.
/// * `timeout` - Maximum time allowed for the metadata command.
///
/// # Returns
///
/// Returns the remote `lastModified` timestamp as seconds since epoch.
///
/// # Errors
///
/// Returns an error if the metadata command fails or the output cannot be
/// parsed.
pub fn get_remote_modified_time(
    url: &str,
    timeout: Duration,
) -> Result<i64, FetchError> {
    let cmd = REMOTE_MODIFIED_TIME_CMD.replace("{URL}", url);
    let output = run_command_with_timeout(&cmd, timeout)?;

    if output.status.success() {
        let s = String::from_utf8(output.stdout)?;
        Ok(s.trim().parse::<i64>()?)
    } else {
        let stdout_str = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();
        let code = output.status.code().unwrap_or(1);
        Err(FetchError::CommandError(CommandError::NonZeroExitCode(
            code, stderr_str, stdout_str,
        )))
    }
}

/// Get the last updated time for all local flake inputs.
///
/// # Arguments
///
/// * `timeout` - Maximum time allowed for the metadata command.
/// * `flake_dir_path` - Path to the flake directory containing `flake.nix`.
///
/// # Returns
///
/// Returns a map of input names to optional `lastModified` timestamps.
///
/// # Errors
///
/// Returns an error if the metadata command fails or the JSON output cannot be
/// parsed.
pub fn get_all_local_modified_times(
    timeout: Duration,
    flake_dir_path: &Path,
) -> Result<HashMap<String, Option<i64>>, FetchError> {
    if !PathBuf::from(flake_dir_path).join("flake.lock").exists() {
        tracing::warn!(
            "Flake.lock file does not exist in: {}, taking longer to rebuild \
             flake graph",
            flake_dir_path.display()
        );
    }

    let cmd = LOCAL_MODIFIED_TIME_CMD
        .replace("{PATH}", &flake_dir_path.display().to_string());
    let output = run_command_with_timeout(&cmd, timeout)?;

    if output.status.success() {
        let mod_map: HashMap<String, Option<i64>> =
            serde_json::from_slice(&output.stdout)?;
        tracing::trace!("{mod_map:#?}");
        Ok(mod_map)
    } else {
        let stdout_str = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();
        let code = output.status.code().unwrap_or(1);
        Err(FetchError::CommandError(CommandError::NonZeroExitCode(
            code, stderr_str, stdout_str,
        )))
    }
}

/// Print a formatted summary of remote input modified times.
///
/// # Arguments
///
/// * `threshold` - Duration used to decide whether an input is stale.
/// * `timeout` - Maximum time allowed for metadata commands.
/// * `quiet` - Whether to suppress output and exit on stale inputs.
/// * `auto_update` - Whether to auto-update stale inputs.
/// * `override_bool` - Skip modification checks when true.
/// * `flake_dir_path` - Path to the flake directory containing `flake.nix`.
pub fn check_flake_inputs(
    threshold: Duration,
    timeout: Duration,
    quiet: bool,
    auto_update: bool,
    override_bool: bool,
    flake_dir_path: &Path,
) {
    let start_time = std::time::Instant::now();

    tracing::info!("Checking flake inputs for updates");
    let current_times = get_all_local_modified_times(timeout, flake_dir_path)
        .unwrap_or_else(|e| {
            tracing::error!("Failed to get input urls: {e}");
            exit(1);
        });

    let input_urls =
        get_input_urls(timeout, flake_dir_path).unwrap_or_else(|e| {
            tracing::error!("Failed to get input urls: {e}");
            exit(1);
        });

    let fetched_times = get_all_remote_modified_times(&input_urls, timeout);

    if current_times.len() != fetched_times.len() {
        tracing::debug!(
            "Number of fetched items ({}) is not equal to the current number \
             of modified times ({})",
            fetched_times.len(),
            current_times.len()
        );
    }

    let mut inputs = Vec::<Input>::new();
    for (input, new_time) in &fetched_times {
        let mut input_struct = Input::new(
            input,
            current_times.get(input).unwrap_or(&None).as_ref(),
            new_time,
        );
        input_struct.get_status(threshold);

        if quiet {
            if input_struct.clone().status == InputStatus::Stale {
                exit(1);
            }
        } else {
            inputs.push(input_struct);
        }
    }

    if quiet {
        return;
    }

    inputs.sort_by(|a, b| {
        a.status.cmp(&b.status).then_with(|| a.name.cmp(&b.name))
    });

    let name_width = fetched_times
        .keys()
        .map(|input: &String| input.width())
        .max()
        .unwrap_or(0)
        + 1;
    let mut last_status = None::<InputStatus>;

    print_summary_message(start_time);

    for input in inputs.clone() {
        if last_status
            .clone()
            .is_none_or(|ls| ls.cmp(&input.status) != cmp::Ordering::Equal)
        {
            if last_status.is_some() {
                tracing::info!("");
            }

            let header: Painted<&str> = match input.status {
                InputStatus::Error(_) => "ERRORED",
                InputStatus::Fresh => "UP TO DATE",
                InputStatus::Stale => "OUT OF DATE",
            }
            .bold();

            tracing::info!("{header}");
            last_status = Some(input.clone().status);
        }

        let input_name = input.clone().name;

        let line = format_status_line(
            &input.status,
            &input_name,
            name_width,
            input.get_additional_info(),
        );

        tracing::info!("{line}");
    }

    tracing::info!("");

    if auto_update
        && let Err(e) = update_stale_flake_inputs(
            &inputs,
            timeout,
            quiet,
            override_bool,
            flake_dir_path,
        )
    {
        tracing::error!("Failed to auto-update stale flake inputs: {e}");
        exit(1);
    }
}

/// Fetch the remote modified times for all flake input URLs.
///
/// # Arguments
///
/// * `url_map` - Map of input names to their URLs.
/// * `timeout` - Maximum time allowed for each metadata command.
///
/// # Returns
///
/// Returns a map of input names to per-input results containing timestamps or
/// errors.
///
/// # Panics
///
/// Panics if the progress style template is invalid.
pub(crate) fn get_all_remote_modified_times(
    url_map: &HashMap<String, String>,
    timeout: Duration,
) -> HashMap<String, Result<i64, FetchError>> {
    let header_span = info_span!("modified_times");
    header_span.pb_set_style(
        &ProgressStyle::with_template(
            "{spinner} {pos}/{len} [{wide_bar:.cyan/blue}] {msg}",
        )
        .expect("Progress style should be created")
        .progress_chars("#>-")
        .tick_chars("⠋⠙⠹⠸⢰⣠⣄⡆⡇⡏⠏⠛ "),
    );
    header_span.pb_set_length(url_map.len() as u64);
    header_span.pb_set_message("Fetching remote modified times");
    let header_enter = header_span.enter();

    let map: Vec<(String, String)> = url_map.clone().into_iter().collect();
    let modified_map: HashMap<String, Result<i64, FetchError>> = map
        .par_iter()
        .map({
            let header_span = header_span.clone();

            move |(input, url)| {
                let item_span = info_span!(
                    parent: &header_span,
                    "input",
                    input = %input,
                    url = %url,
                );
                item_span.pb_set_style(
                    &ProgressStyle::with_template("  {spinner} {msg}")
                        .expect("Progress style should be created")
                        .tick_chars("⠋⠙⠹⠸⢰⣠⣄⡆⡇⡏⠏⠛ "),
                );
                let ts = item_span.in_scope(|| {
                    item_span.pb_set_message(&format!("Fetching {url}"));
                    let ts = get_remote_modified_time(url, timeout);
                    match &ts {
                        Ok(_) => {
                            item_span.pb_set_message(&format!("Fetched {url}"));
                        },
                        Err(e) => {
                            tracing::debug!("Failed to fetch {url}: {e}");
                            item_span.pb_set_message(&format!("Failed {url}"));
                        },
                    }
                    ts
                });

                header_span.pb_inc(1);
                (input.clone(), ts)
            }
        })
        .collect();

    drop(header_enter);
    drop(header_span);
    tracing::trace!("{modified_map:#?}");
    modified_map
}
