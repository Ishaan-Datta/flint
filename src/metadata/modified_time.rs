use crate::metadata::get_input_urls;
use crate::metadata::get_modified_times;

use chrono::Local;
use std::cmp;
use std::collections::HashMap;
use std::process::Command;
use std::time::Instant;
use thiserror::Error;
use tracing::Span;
use tracing_indicatif::span_ext::IndicatifSpanExt;
use unicode_width::UnicodeWidthStr as _;
use yansi::Paint;
use yansi::Painted;

#[derive(Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusError {
    #[error("Time travel detected (The local input source is more recent than the remote)")]
    Less,
    #[error("Could not fetch the flake metadata for the input source")]
    NotFetched,
}

// add "--no-write-lock-file after --json"
const INPUT_MODIFIED_CMD: &str =
    r#"nix --refresh flake metadata {URL} --json | jq -r '.lastModified'"#;
const LAST_MODIFIED_CMD: &str = r#"nix flake metadata --json --no-write-lock-file . | jq -r '.locks.nodes | map_values(.locked.lastModified)'"#;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum InputStatus {
    Fresh,
    Stale,
    Error(StatusError),
}

impl PartialOrd for InputStatus {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl cmp::Ord for InputStatus {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        // need updates
        // then errored
        // then fresh
        match (self, other) {
            // Same variants are equal
            (Self::Fresh, Self::Fresh) => cmp::Ordering::Equal,
            (Self::Stale, Self::Stale) => cmp::Ordering::Equal,
            (Self::Error(_), Self::Error(_)) => cmp::Ordering::Equal,

            // Stale comes before everything else
            (Self::Stale, _) => cmp::Ordering::Less,
            (_, Self::Stale) => cmp::Ordering::Greater,

            // Errors come before fresh
            (Self::Error(_), Self::Fresh) => cmp::Ordering::Less,
            (Self::Fresh, Self::Error(_)) => cmp::Ordering::Greater,
        }
    }
}

impl InputStatus {
    fn char(self) -> Painted<&'static char> {
        match self {
            Self::Fresh => '✔'.green().bold(), // or ↻ or ~
            Self::Stale => '!'.yellow().bold(),
            Self::Error(_) => '✘'.red().bold(),
        }
    }
}

#[derive(Debug, Clone)]
struct Input {
    pub name: String,
    pub status: InputStatus,
    local_time: Option<i64>,
    remote_time: Option<i64>,
}

impl Input {
    fn new(input_string: &str, local_time: &Option<i64>, remote_time: &Option<i64>) -> Self {
        Input {
            name: input_string.to_string(),
            status: InputStatus::Fresh,
            local_time: *local_time,
            remote_time: *remote_time,
        }
    }

    fn get_status(&mut self, threshold: i64) {
        match self.remote_time {
            None => self.status = InputStatus::Error(StatusError::NotFetched),
            Some(remote_time) => match self.local_time {
                None => self.status = InputStatus::Stale,
                Some(local_time) => {
                    if local_time > remote_time {
                        self.status = InputStatus::Error(StatusError::Less)
                    } else if remote_time - local_time > threshold {
                        self.status = InputStatus::Stale
                    } else {
                        self.status = InputStatus::Fresh
                    }
                }
            },
        }
    }

    fn get_additional_info(&self) -> Option<String> {
        match self.status {
            InputStatus::Fresh => None,
            InputStatus::Stale => Some(
                format!(
                    "Time since last update: {}",
                    self.get_human_readable_time_diff()
                )
                .yellow()
                .to_string(),
            ),
            InputStatus::Error(e) => Some(format!("{e}").red().to_string()),
        }
    }

    fn get_human_readable_time_diff(&self) -> String {
        if self.local_time.is_none() {
            return "Unknown".to_string();
        }
        if self.remote_time.is_none() {
            return "Unknown".to_string();
        }
        format_age(self.remote_time.unwrap() - self.local_time.unwrap())
    }
}

fn format_age(secs: i64) -> String {
    let mut s = Vec::new();
    let mut remaining = secs.max(0);

    let days = remaining / 86_400;
    remaining %= 86_400;
    let hours = remaining / 3_600;
    remaining %= 3_600;
    let mins = remaining / 60;

    s.push(format!("{days:>3} d"));
    s.push(format!("{hours:>2} h"));
    s.push(format!("{mins:>2} m"));
    s.join(", ")
}

/// Fetches new modified time for a single flake url
///
// TODO: make this a result so you can send errors up
pub fn get_new_modified_time(url: &str) -> Option<i64> {
    let cmd = INPUT_MODIFIED_CMD.replace("{URL}", url);

    // todo: add timeout
    let output = Command::new("sh").args(["-c", &cmd]).output().ok()?;
    if !output.status.success() {
        // tracing error here
        return None;
    }

    let s = String::from_utf8(output.stdout).ok()?;

    match s.trim().parse::<i64>() {
        Ok(val) => {
            // tracing::info!("Fetched remote modified time for flake input: {url}");
            Some(val)
        }
        Err(e) => {
            // tracing::warn!("Failed to fetch remote modified time for flake input: {url}: {e}");
            // bar.set_message("Failed to fetch remote modified time for flake input: {url}: {e}");
            // bar.inc(1);
            // Span::current().pb_inc(1);
            // Span::current().pb_tick();
            None
        }
    }
}

/// Gets the last updated time for all flake inputs
pub fn get_last_modified_times() -> Result<HashMap<String, Option<i64>>, anyhow::Error> {
    let mod_output = Command::new("sh")
        .args(["-c", LAST_MODIFIED_CMD])
        .output()?;
    if mod_output.status.success() {
        let mod_map: HashMap<String, Option<i64>> = serde_json::from_slice(&mod_output.stdout)?;
        // println!("{mod_map:#?}");
        Ok(mod_map)
    } else {
        anyhow::bail!("Failed to get input modified times: format error here...");
    }
}

fn print_summary_message(start: Instant) {
    let now = Local::now();
    let t24 = now.format("%H:%M:%S").to_string();
    let duration = start.elapsed().as_secs();
    let summary = format!("Finished at {t24} after {duration}s")
        .bold()
        .to_string();
    println!("{summary}\n");
}

pub fn print_input_summary() -> Result<(), anyhow::Error> {
    let start_time = std::time::Instant::now();
    println!("> Checking flake inputs for updates");

    let current_times = get_last_modified_times()?;
    let fetched_times = get_modified_times(get_input_urls()?);

    // load from god object
    // should read environment variable for the threshold at which you put out of date, default is like 2 weeks
    let threshold = 1000;

    // make a debug log ehre if the number of fetched items is not equal to the number of fetched inputs,
    // include debug trace

    let mut inputs = Vec::<Input>::new();

    for (input, new_time) in fetched_times.iter() {
        let mut input_struct =
            Input::new(input, current_times.get(input).unwrap_or(&None), new_time);
        input_struct.get_status(threshold);
        inputs.push(input_struct);
    }

    inputs.sort_by(|a, b| a.status.cmp(&b.status).then_with(|| a.name.cmp(&b.name)));

    let name_width = fetched_times
        .keys()
        .map(|input| input.width())
        .max()
        .unwrap_or(0)
        + 1;
    let mut last_status = None::<InputStatus>;

    print_summary_message(start_time);

    for input in inputs {
        if last_status.is_none_or(|ls| ls.cmp(&input.status) != cmp::Ordering::Equal) {
            if last_status.is_some() {
                println!("");
            }

            let header: Painted<&str> = match input.status {
                InputStatus::Error(_) => "ERRORED",
                InputStatus::Fresh => "UP TO DATE",
                InputStatus::Stale => "OUT OF DATE",
            }
            .bold();

            println!("{header}");
            last_status = Some(input.status)
        }

        let status_char = input.status.char();
        let input_name = input.clone().name;

        let line = match input.get_additional_info() {
            Some(info) => {
                format!("[{status_char}] {input_name:<name_width$} {info}")
            }
            None => {
                format!("[{status_char}] {input_name:<name_width$}")
            }
        };
        println!("{line}");
    }
    Ok(())
}
