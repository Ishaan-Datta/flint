use std::{cmp, time::Duration};

use serde::{Deserialize, Serialize};
use yansi::{Paint, Painted};

use crate::{
    errors::{FetchError, StatusError},
    modified_time::format_age,
};

/// Remote flake input identified by name and URL.
///
/// This type is used as the cache key for remote modified-time results.
#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct RemoteInput {
    /// Name of the flake input.
    pub input_name: String,

    /// URL used to fetch remote metadata for the input.
    pub input_url: String,
}

/// Status of a flake input after comparing local and remote modified times.
#[derive(Debug, PartialEq, Eq, Clone)]
pub(crate) enum InputStatus {
    /// The input is within the configured staleness threshold.
    Fresh,

    /// The remote input is newer than the local lock-file entry by more than
    /// the configured threshold, or no local timestamp was available.
    Stale,

    /// The input could not be compared because fetching or timestamp comparison
    /// failed.
    Error(StatusError),
}

impl PartialOrd for InputStatus {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl cmp::Ord for InputStatus {
    /// Sort statuses in display priority order: stale, error, then fresh.
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
    /// Return the plain, uncolored status marker.
    ///
    /// # Returns
    ///
    /// Returns `"✔"` for fresh, `"!"` for stale, and `"✘"` for errored inputs.
    pub(crate) const fn plain_char(&self) -> &'static str {
        match self {
            Self::Fresh => "✔",
            Self::Stale => "!",
            Self::Error(_) => "✘",
        }
    }

    /// Return the colored, bold status marker.
    ///
    /// # Returns
    ///
    /// Returns the painted marker for the current status.
    pub(crate) fn painted_char(&self) -> Painted<&'static char> {
        match &self {
            Self::Fresh => '✔'.green().bold(), // or ↻ or ~
            Self::Stale => '!'.yellow().bold(),
            Self::Error(_) => '✘'.red().bold(),
        }
    }
}

/// Comparison result for a single flake input.
#[derive(Debug, Clone)]
pub struct Input {
    /// Input name used for display and lookup.
    pub name: String,

    /// Freshness status after comparing local and remote timestamps.
    pub(crate) status: InputStatus,

    /// Difference between remote and local timestamps in seconds.
    ///
    /// Present only when both timestamps were available and comparison
    /// succeeded.
    difference: Option<i64>,
}

impl Input {
    /// Create an input comparison result from local and remote timestamps.
    ///
    /// The resulting status is:
    ///
    /// * `Error` when the remote fetch failed.
    /// * `Stale` when no local timestamp is available.
    /// * `Error` when the remote timestamp is older than the local timestamp.
    /// * `Stale` when the remote timestamp is newer than the local timestamp by
    ///   more than `threshold`.
    /// * `Fresh` otherwise.
    ///
    /// # Arguments
    ///
    /// * `input_name` - Name of the flake input.
    /// * `local_time` - Local locked `lastModified` timestamp, if available.
    /// * `remote_time` - Remote fetch result containing either a timestamp or a
    ///   fetch error.
    /// * `threshold` - Maximum allowed timestamp difference before the input is
    ///   considered stale.
    ///
    /// # Returns
    ///
    /// Returns the computed `Input` status record.
    ///
    /// # Panics
    ///
    /// Panics if `threshold.as_secs()` cannot be converted into `i64`.
    pub(crate) fn new(
        input_name: &str,
        local_time: Option<&u64>,
        remote_time: &Result<u64, FetchError>,
        threshold: Duration,
    ) -> Self {
        let (status, difference) = match remote_time {
            Err(e) => {
                tracing::debug!("Failed to fetch input {}: {e}", input_name);
                (
                    InputStatus::Error(StatusError::NotFetched(e.to_string())),
                    None,
                )
            },
            Ok(remote_time) => {
                match local_time {
                    None => (InputStatus::Stale, None),
                    Some(local_time) => {
                        let difference =
                            *remote_time as i64 - *local_time as i64;

                        if difference < 0 {
                            (InputStatus::Error(StatusError::Less), None)
                        } else if difference
                            > threshold.as_secs().try_into().expect(
                                "Should be able to convert threshold into i64",
                            )
                        {
                            (InputStatus::Stale, Some(difference))
                        } else {
                            (InputStatus::Fresh, Some(difference))
                        }
                    },
                }
            },
        };

        Self {
            name: input_name.to_string(),
            status,
            difference,
        }
    }

    /// Build optional human-readable information for the current status.
    ///
    /// # Returns
    ///
    /// Returns `None` for fresh inputs, the elapsed time since last update for
    /// stale inputs, or the error message for errored inputs.
    pub(crate) fn get_additional_info(&self) -> Option<String> {
        match &self.status {
            InputStatus::Fresh => None,
            InputStatus::Stale => {
                Some(format!(
                    "Time since last update: {}",
                    self.get_human_readable_time_diff()
                ))
            },
            InputStatus::Error(e) => Some(format!("{e}")),
        }
    }

    /// Format the timestamp difference between remote and local updates.
    ///
    /// # Returns
    ///
    /// Returns the formatted difference, or `"Unknown"` when the difference is
    /// unavailable.
    pub(crate) fn get_human_readable_time_diff(&self) -> String {
        if let Some(difference) = self.difference {
            format_age(difference)
        } else {
            "Unknown".to_string()
        }
    }
}
