use crate::command::run_command_with_timeout;
use crate::errors::CommandError;
use crate::errors::FetchError;
use crate::metadata::get_input_urls;
use crate::modified_time::Input;
use crate::modified_time::InputStatus;
use crate::modified_time::format_status_line;
use crate::modified_time::print_summary_message;

use indicatif::ProgressStyle;
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;
use std::cmp;
use std::collections::HashMap;
use std::process::Command;
use std::process::exit;
use std::time::Duration;
use tracing::info_span;
use tracing_indicatif::span_ext::IndicatifSpanExt;
use unicode_width::UnicodeWidthStr;
use yansi::Paint;
use yansi::Painted;

// add "--no-write-lock-file after --json"
const INPUT_MODIFIED_CMD: &str =
    r#"nix --refresh flake metadata {URL} --json | jq -r '.lastModified'"#;
const LAST_MODIFIED_CMD: &str = r#"nix flake metadata --json --no-write-lock-file . | jq -r '.locks.nodes | map_values(.locked.lastModified)'"#;

/// Fetches new modified time for a single flake url
pub fn get_new_modified_time(url: &str, timeout: Duration) -> Result<i64, FetchError> {
    let cmd = INPUT_MODIFIED_CMD.replace("{URL}", url);
    let output = run_command_with_timeout(cmd, timeout)?;

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

/// Gets the last updated time for all flake inputs
// TODO: make this use the command thing like the one above
pub fn get_last_modified_times() -> Result<HashMap<String, Option<i64>>, anyhow::Error> {
    let mod_output = Command::new("sh")
        .args(["-c", LAST_MODIFIED_CMD])
        .output()?;
    if mod_output.status.success() {
        let mod_map: HashMap<String, Option<i64>> = serde_json::from_slice(&mod_output.stdout)?;
        tracing::trace!("{mod_map:#?}");
        Ok(mod_map)
    } else {
        anyhow::bail!("Failed to get input modified times: format error here...");
    }
}

// TODO: add proper error handling, dont return errors here, top level
pub fn print_input_summary(threshold: u64, timeout: u64, quiet: bool) {
    let start_time = std::time::Instant::now();
    let timeout = Duration::from_millis(timeout);

    tracing::info!("> Checking flake inputs for updates");
    let current_times = get_last_modified_times()?;

    let input_urls = get_input_urls(timeout)
        .map_err(|e| {
            tracing::debug!("Failed to get input urls: {e}");
            exit(1);
        })
        .unwrap();

    let fetched_times = get_modified_times(input_urls, timeout);

    if current_times.len() != fetched_times.len() {
        tracing::debug!(
            "Number of fetched items ({}) is not equal to the current number of modified times ({})",
            fetched_times.len(),
            current_times.len()
        );
    }

    let mut inputs = Vec::<Input>::new();
    for (input, new_time) in fetched_times.iter() {
        let mut input_struct =
            Input::new(input, current_times.get(input).unwrap_or(&None), new_time);
        input_struct.get_status(threshold);

        if quiet {
            if input_struct.clone().status == InputStatus::Stale {
                exit(1);
            } else {
                continue;
            }
        } else {
            inputs.push(input_struct);
        }
    }

    if quiet {
        return;
    }

    inputs.sort_by(|a, b| a.status.cmp(&b.status).then_with(|| a.name.cmp(&b.name)));

    let name_width = fetched_times
        .keys()
        .map(|input: &String| input.width())
        .max()
        .unwrap_or(0)
        + 1;
    let mut last_status = None::<InputStatus>;

    print_summary_message(start_time);

    for input in inputs {
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
            last_status = Some(input.clone().status)
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
}

// TODO: rename
pub fn get_modified_times(
    url_map: HashMap<String, String>,
    timeout: Duration,
) -> HashMap<String, Result<i64, FetchError>> {
    let header_span = info_span!("modified_times");
    header_span.pb_set_style(
        &ProgressStyle::with_template("{spinner} {pos}/{len} [{wide_bar:.cyan/blue}] {msg}")
            .unwrap()
            .progress_chars("#>-"),
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
                item_span.pb_set_style(&ProgressStyle::with_template("  {spinner} {msg}").unwrap());
                let ts = item_span.in_scope(|| {
                    item_span.pb_set_message(&format!("Fetching {url}"));
                    let ts = get_new_modified_time(url, timeout);
                    match &ts {
                        Ok(_) => {
                            item_span.pb_set_message(&format!("Fetched {url}"));
                        }
                        Err(e) => {
                            tracing::debug!("Failed to fetch {url}: {e}");
                            item_span.pb_set_message(&format!("Failed {url}"));
                        }
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
