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
    cache::{read_cache, write_cache},
    command::run_command_with_timeout,
    errors::{CommandError, FetchError},
    metadata::{get_input_urls, update_inputs::update_stale_flake_inputs},
    modified_time::{
        Input,
        InputStatus,
        RemoteInput,
        format_status_line,
        print_summary_message,
    },
};

// NB: add `sleep 2 &&` for below cmd during demo
const REMOTE_MODIFIED_TIME_CMD: &str = r"nix --refresh flake metadata {URL} --json --no-write-lock-file | jq -er '.lastModified'";
// NB: add `sleep 0.5 &&` for below cmd during demo
const LOCAL_MODIFIED_TIME_CMD: &str = r"nix flake metadata --json --no-write-lock-file {PATH} | jq -er '.locks.nodes | map_values(.locked.lastModified)'";

/// Fetch the remote modified time for a single flake URL.
///
/// Runs `nix --refresh flake metadata` for the given URL and parses
/// `.lastModified` from the JSON output.
///
/// # Arguments
///
/// * `url` - Flake input URL to query.
/// * `timeout` - Maximum time allowed for the metadata command.
///
/// # Returns
///
/// Returns the remote `lastModified` timestamp as seconds since the Unix epoch.
///
/// # Errors
///
/// Returns `FetchError` if the command fails, exits with a non-zero status,
/// emits invalid UTF-8, or returns a timestamp that cannot be parsed.
pub fn get_remote_modified_time(
    url: &str,
    timeout: Duration,
) -> Result<u64, FetchError> {
    let cmd = REMOTE_MODIFIED_TIME_CMD.replace("{URL}", url);
    let output = run_command_with_timeout(&cmd, timeout)?;

    if output.status.success() {
        let s = String::from_utf8(output.stdout)?;
        Ok(s.trim().parse::<u64>()?)
    } else {
        let stdout_str = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();
        let code = output.status.code().unwrap_or(1);
        Err(FetchError::CommandError(CommandError::NonZeroExitCode(
            code, stderr_str, stdout_str,
        )))
    }
}

/// Get the locked local modified times for all flake inputs.
///
/// Runs `nix flake metadata` for the local flake path and parses the
/// `locked.lastModified` value for each lock node.
///
/// # Arguments
///
/// * `timeout` - Maximum time allowed for the metadata command.
/// * `flake_dir_path` - Path to the flake directory.
///
/// # Returns
///
/// Returns a map of input names to optional local `lastModified` timestamps.
/// Inputs without a locked timestamp are represented as `None`.
///
/// # Errors
///
/// Returns `FetchError` if the command fails, exits with a non-zero status, or
/// emits JSON that cannot be parsed into the expected map.
pub fn get_all_local_modified_times(
    timeout: Duration,
    flake_dir_path: &Path,
) -> Result<HashMap<String, Option<u64>>, FetchError> {
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
        let mod_map: HashMap<String, Option<u64>> =
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

/// Check local flake inputs against their latest remote modified times.
///
/// Fetches local lock-file timestamps, resolves declared input URLs, obtains
/// remote timestamps using the cache when possible, prints a grouped status
/// summary, and optionally updates stale inputs.
///
/// # Arguments
///
/// * `threshold` - Maximum allowed age difference between local and remote
///   timestamps before an input is considered stale.
/// * `timeout` - Maximum time allowed for each Nix metadata command.
/// * `quiet` - Suppress status output and use the process exit code to report
///   whether stale inputs were found.
/// * `auto_update` - Update stale inputs after checking.
/// * `override_bool` - Passed to the update path to override normal update
///   behavior.
/// * `flake_dir_path` - Path to the flake directory.
/// * `cache_file_path` - Path to the remote modified-time cache file.
/// * `cache_expiry` - Maximum cache age in seconds.
///
/// # Exits
///
/// Exits with code `1` when local metadata or input URL discovery fails. In
/// quiet mode, exits with code `1` as soon as a stale input is found and exits
/// with code `0` when no stale inputs are found. Also exits with code `1` if
/// auto-update is enabled and updating stale inputs fails.
#[allow(clippy::too_many_arguments)]
pub fn check_flake_inputs(
    threshold: Duration,
    timeout: Duration,
    quiet: bool,
    auto_update: bool,
    override_bool: bool,
    flake_dir_path: &Path,
    cache_file_path: PathBuf,
    cache_expiry: u64,
) {
    let start_time = std::time::Instant::now();

    tracing::info!("Checking flake inputs for updates");
    let current_times = get_all_local_modified_times(timeout, flake_dir_path)
        .unwrap_or_else(|e| {
            tracing::error!("Failed to parse local modified times: {e}");
            exit(1);
        });

    let input_urls =
        get_input_urls(timeout, flake_dir_path).unwrap_or_else(|e| {
            tracing::error!("Failed to get input urls: {e}");
            exit(1);
        });

    let fetched_times = get_all_remote_modified_times(
        &input_urls,
        timeout,
        cache_file_path,
        cache_expiry,
    );

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
        let input_struct = Input::new(
            &input.input_name,
            current_times
                .get(&input.input_name)
                .unwrap_or(&None)
                .as_ref(),
            new_time,
            threshold,
        );

        if quiet {
            if input_struct.clone().status == InputStatus::Stale {
                exit(1);
            }
        } else {
            inputs.push(input_struct);
        }
    }

    if quiet {
        exit(0);
    }

    inputs.sort_by(|a, b| {
        a.status.cmp(&b.status).then_with(|| a.name.cmp(&b.name))
    });

    let name_width = fetched_times
        .keys()
        .map(|input: &RemoteInput| input.input_name.width())
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

    if auto_update
        && let Err(e) = update_stale_flake_inputs(
            &inputs,
            timeout,
            quiet,
            override_bool,
            flake_dir_path,
        )
    {
        tracing::error!("\nFailed to auto-update stale flake inputs: {e}");
        exit(1);
    }
}

/// Fetch remote modified times for all flake inputs.
///
/// If a valid, unexpired cache file exists, returns the cached entries without
/// running remote metadata commands. Otherwise fetches each remote input in
/// parallel, writes the result map to the cache, and returns the freshly
/// fetched results.
///
/// # Arguments
///
/// * `remote_inputs` - Remote flake inputs to fetch.
/// * `timeout` - Maximum time allowed for each remote metadata command.
/// * `cache_file_path` - Path to the remote modified-time cache file.
/// * `cache_expiry` - Maximum cache age in seconds.
///
/// # Returns
///
/// Returns a map from each `RemoteInput` to either its remote `lastModified`
/// timestamp or the `FetchError` produced while fetching it.
///
/// # Panics
///
/// Panics if the progress style templates are invalid.
pub fn get_all_remote_modified_times(
    remote_inputs: &[RemoteInput],
    timeout: Duration,
    cache_file_path: PathBuf,
    cache_expiry: u64,
) -> HashMap<RemoteInput, Result<u64, FetchError>> {
    let header_span = info_span!("modified_times");
    header_span.pb_set_style(
        &ProgressStyle::with_template(
            "{spinner} {pos}/{len} [{wide_bar:.cyan/blue}] {msg}",
        )
        .expect("Progress style should be created")
        .progress_chars("#>-")
        .tick_chars("⠋⠙⠹⠸⢰⣠⣄⡆⡇⡏⠏⠛ "),
    );
    header_span.pb_set_length(remote_inputs.len() as u64);
    header_span.pb_set_message("Fetching remote modified times");
    let header_enter = header_span.enter();

    if let Some(entries) = read_cache(&cache_file_path, cache_expiry) {
        tracing::debug!("Using remote cache entries: {entries:#?}");
        entries
    } else {
        tracing::debug!("Fetching fresh entries from remotes instead of cache");
        let modified_map: HashMap<RemoteInput, Result<u64, FetchError>> =
            remote_inputs
                .par_iter()
                .map({
                    let header_span = header_span.clone();

                    move |remote_input| {
                        let input = remote_input.input_name.clone();
                        let url = remote_input.input_url.clone();

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
                            item_span
                                .pb_set_message(&format!("Fetching {url}"));
                            let ts = get_remote_modified_time(&url, timeout);
                            match &ts {
                                Ok(_) => {
                                    item_span.pb_set_message(&format!(
                                        "Fetched {url}"
                                    ));
                                },
                                Err(e) => {
                                    tracing::debug!(
                                        "Failed to fetch {url}: {e}"
                                    );
                                    item_span.pb_set_message(&format!(
                                        "Failed {url}"
                                    ));
                                },
                            }
                            ts
                        });

                        header_span.pb_inc(1);
                        (remote_input.clone(), ts)
                    }
                })
                .collect();

        drop(header_enter);
        drop(header_span);
        tracing::trace!("{modified_map:#?}");

        if let Err(e) = write_cache(&modified_map, cache_file_path) {
            tracing::warn!("Failed to save entries to cache: {e}");
        } else {
            tracing::info!("Successfully saved entries to cache");
        }

        modified_map
    }
}
