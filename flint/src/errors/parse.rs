use std::{
  num::ParseIntError,
  path::PathBuf,
  string::FromUtf8Error,
  sync::Arc,
};

use thiserror::Error;

use crate::errors::CommandError;

#[derive(Error, Debug, Clone)]
pub enum FetchError {
  #[error(
    "Flake path: {0} is invalid (directory does not exist or flake.nix could \
     not be found)"
  )]
  InvalidPath(PathBuf),
  #[error("Failed to parse last modified time from command output: {0}")]
  IntParse(#[from] ParseIntError),
  #[error("Failed to read stdout from command: {0}")]
  StdParse(#[from] FromUtf8Error),
  #[error("Failed to parse input map: {0}")]
  InputMapParse(#[from] Arc<serde_json::Error>),
  #[error("Command failed to run: {0}")]
  CommandExecution(#[from] Arc<anyhow::Error>),
  #[error(transparent)]
  CommandError(#[from] CommandError),
  #[error("Flake inputs list is empty")]
  NoFlakeInputs,
  #[error("The flake root object was not found")]
  MalformedFlake,
}

impl From<serde_json::Error> for FetchError {
  fn from(err: serde_json::Error) -> Self {
    Self::InputMapParse(Arc::new(err))
  }
}
