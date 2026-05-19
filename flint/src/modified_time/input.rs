use std::{cmp, time::Duration};

use yansi::{Paint, Painted};

use crate::{
  errors::{FetchError, StatusError},
  modified_time::format_age,
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum InputStatus {
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
  /// Return the plain, uncolored status marker.
  ///
  /// # Returns
  ///
  /// Returns the marker as a static string.
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

#[derive(Debug, Clone)]
pub struct Input {
  pub name:    String,
  pub status:  InputStatus,
  local_time:  Option<i64>,
  remote_time: Result<i64, FetchError>,
}

impl Input {
  /// Create a new input record with a default `Fresh` status.
  ///
  /// # Arguments
  ///
  /// * `input_string` - The display name for the input.
  /// * `local_time` - The last local update time, if available.
  /// * `remote_time` - The fetched remote update time or a fetch error.
  ///
  /// # Returns
  ///
  /// Returns the initialized `Input`.
  pub(crate) fn new(
    input_string: &str,
    local_time: Option<&i64>,
    remote_time: &Result<i64, FetchError>,
  ) -> Self {
    Self {
      name:        input_string.to_string(),
      status:      InputStatus::Fresh,
      local_time:  local_time.copied(),
      remote_time: remote_time.clone(),
    }
  }

  /// Compute and update the input status using the time threshold.
  ///
  /// # Arguments
  ///
  /// * `threshold` - The maximum allowed age before the input is considered
  ///   stale.
  pub(crate) fn get_status(&mut self, threshold: Duration) {
    match &self.remote_time {
      Err(e) => {
        tracing::debug!("Failed to fetch input {}: {e}", self.name);
        self.status =
          InputStatus::Error(StatusError::NotFetched(e.to_string()));
      },
      Ok(remote_time) => {
        match self.local_time {
          None => self.status = InputStatus::Stale,
          Some(local_time) => {
            if local_time > *remote_time {
              self.status = InputStatus::Error(StatusError::Less);
            } else if remote_time - local_time
              > threshold.as_secs().cast_signed()
            {
              self.status = InputStatus::Stale;
            } else {
              self.status = InputStatus::Fresh;
            }
          },
        }
      },
    }
  }

  /// Build optional human-readable info based on the current status.
  ///
  /// # Returns
  ///
  /// Returns `None` for fresh inputs, or a status-specific message otherwise.
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

  /// Format the time difference between remote and local updates.
  ///
  /// # Returns
  ///
  /// Returns the formatted time difference, or "Unknown" when times are
  /// missing.
  ///
  /// # Panics
  ///
  /// Panics if the internal time values are missing despite earlier checks.
  pub(crate) fn get_human_readable_time_diff(&self) -> String {
    if self.local_time.is_none() {
      return "Unknown".to_string();
    }
    if self.remote_time.is_err() {
      return "Unknown".to_string();
    }
    format_age(
      self
        .remote_time
        .clone()
        .expect("Validated remote time earlier")
        - self.local_time.expect("Validated local time earlier"),
    )
  }
}
