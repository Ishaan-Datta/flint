use std::{
    num::ParseIntError,
    path::PathBuf,
    string::FromUtf8Error,
    sync::Arc,
};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use crate::errors::CommandError;

/// Errors that can occur while fetching or parsing flake metadata.
#[derive(Error, Debug, Clone)]
pub enum FetchError {
    /// The provided flake path does not exist or does not contain `flake.nix`.
    #[error(
        "Flake path: {0} is invalid (directory does not exist or flake.nix \
         could not be found)"
    )]
    InvalidPath(PathBuf),

    /// A timestamp could not be parsed as an integer.
    #[error("Failed to parse last modified time from command output: {0}")]
    IntParse(#[from] ParseIntError),

    /// Command stdout could not be decoded as UTF-8.
    #[error("Failed to read stdout from command: {0}")]
    StdParse(#[from] FromUtf8Error),

    /// Input metadata JSON could not be parsed.
    #[error("Failed to parse input map: {0}")]
    InputMapParse(#[from] Arc<serde_json::Error>),

    /// The metadata command could not be executed.
    #[error("Command failed to run: {0}")]
    CommandExecution(#[from] Arc<anyhow::Error>),

    /// The metadata command ran but returned a command-level error.
    #[error(transparent)]
    CommandError(#[from] CommandError),

    /// No flake inputs were found.
    #[error("Flake inputs list is empty")]
    NoFlakeInputs,

    /// The expected root object was missing from flake metadata.
    #[error("The flake root object was not found")]
    MalformedFlake,

    /// Error loaded from the cache.
    ///
    /// Cached fetch errors are stored as display strings. When deserialized,
    /// they are represented with this variant instead of the original concrete
    /// error variant.
    #[error("Cached input is an error: {0}")]
    CachedFetchError(String),
}

/// Serialize a fetch error as its display string.
///
/// This keeps cache serialization simple and stable, but it does not preserve
/// the original enum variant for cached errors.
impl Serialize for FetchError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// Deserialize a cached fetch error from its display string.
///
/// Since the serialized form only stores the rendered error message, all cached
/// errors deserialize as `FetchError::CachedFetchError`.
impl<'de> Deserialize<'de> for FetchError {
    fn deserialize<D>(deserializer: D) -> Result<FetchError, D::Error>
    where
        D: Deserializer<'de>,
    {
        let message = String::deserialize(deserializer)?;
        Ok(FetchError::CachedFetchError(message))
    }
}

/// Convert a cached error message into `FetchError::CachedFetchError`.
impl From<String> for FetchError {
    fn from(value: String) -> Self {
        Self::CachedFetchError(value)
    }
}

/// Convert a serde JSON parse error into `FetchError::InputMapParse`.
impl From<serde_json::Error> for FetchError {
    fn from(err: serde_json::Error) -> Self {
        Self::InputMapParse(Arc::new(err))
    }
}
