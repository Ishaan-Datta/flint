use crate::errors::FetchError;
use crate::errors::StatusError;
use crate::modified_time::format_age;

use std::cmp;
use std::time::Duration;
use yansi::Paint;
use yansi::Painted;

#[derive(Debug, PartialEq, Eq, Clone)]
pub(crate) enum InputStatus {
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
    /// Ordered as: Updates -> Errored -> Fresh
    fn cmp(&self, other: &Self) -> cmp::Ordering {
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
    pub(crate) fn plain_char(&self) -> &'static str {
        match self {
            Self::Fresh => "✔",
            Self::Stale => "!",
            Self::Error(_) => "✘",
        }
    }

    pub(crate) fn painted_char(&self) -> Painted<&'static char> {
        match &self {
            Self::Fresh => '✔'.green().bold(), // or ↻ or ~
            Self::Stale => '!'.yellow().bold(),
            Self::Error(_) => '✘'.red().bold(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Input {
    pub name: String,
    pub status: InputStatus,
    local_time: Option<i64>,
    remote_time: Result<i64, FetchError>,
}

impl Input {
    pub(crate) fn new(
        input_string: &str,
        local_time: &Option<i64>,
        remote_time: &Result<i64, FetchError>,
    ) -> Self {
        Input {
            name: input_string.to_string(),
            status: InputStatus::Fresh,
            local_time: *local_time,
            remote_time: remote_time.clone(),
        }
    }

    pub(crate) fn get_status(&mut self, threshold: Duration) {
        match &self.remote_time {
            Err(e) => {
                tracing::debug!("Failed to fetch input {}: {e}", self.name);
                self.status = InputStatus::Error(StatusError::NotFetched(e.to_string()))
            }
            Ok(remote_time) => match self.local_time {
                None => self.status = InputStatus::Stale,
                Some(local_time) => {
                    if local_time > *remote_time {
                        self.status = InputStatus::Error(StatusError::Less)
                    } else if remote_time - local_time > threshold.as_secs() as i64 {
                        self.status = InputStatus::Stale
                    } else {
                        self.status = InputStatus::Fresh
                    }
                }
            },
        }
    }

    pub(crate) fn get_additional_info(&self) -> Option<String> {
        match &self.status {
            InputStatus::Fresh => None,
            InputStatus::Stale => Some(format!(
                "Time since last update: {}",
                self.get_human_readable_time_diff()
            )),
            InputStatus::Error(e) => Some(format!("{e}")),
        }
    }

    pub(crate) fn get_human_readable_time_diff(&self) -> String {
        if self.local_time.is_none() {
            return "Unknown".to_string();
        }
        if self.remote_time.is_err() {
            return "Unknown".to_string();
        }
        format_age(self.remote_time.clone().unwrap() - self.local_time.unwrap())
    }
}
