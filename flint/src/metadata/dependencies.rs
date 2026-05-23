use std::{
  cmp,
  collections::HashMap,
  path::{Path, PathBuf},
  time::Duration,
};

use crate::{
  command::with_command_spinner,
  errors::{CommandError, FetchError},
};

const DEPENDENCIES_CMD: &str = r#"nix flake metadata --json --no-write-lock-file {PATH} \
  | jq -e '.locks.nodes
        | map_values(
            (.inputs // {})
            | map_values(
                if type == "array" then .
                else [.]
                end
              )
          )'
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputReplacement {
  pub input_dependency:      String,
  pub old_dependency_target: String,
  pub new_dependency_target: String,
}

impl InputReplacement {
  /// Create a new input replacement record.
  ///
  /// # Arguments
  ///
  /// * `input_dependency` - Name of the input that depends on another input.
  /// * `old_dependency_target` - Original dependency target string.
  /// * `new_dependency_target` - Replacement dependency target string.
  ///
  /// # Returns
  ///
  /// Returns a populated `InputReplacement` instance.
  fn new(
    input_dependency: &str,
    old_dependency_target: &str,
    new_dependency_target: &str,
  ) -> Self {
    Self {
      input_dependency:      input_dependency.to_string(),
      old_dependency_target: old_dependency_target.to_string(),
      new_dependency_target: new_dependency_target.to_string(),
    }
  }
}

impl PartialOrd for InputReplacement {
  fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
    Some(self.cmp(other))
  }
}

impl cmp::Ord for InputReplacement {
  fn cmp(&self, other: &Self) -> cmp::Ordering {
    self.input_dependency.cmp(&other.input_dependency)
  }
}

impl From<InputReplacement> for String {
  fn from(r: InputReplacement) -> Self {
    r.new_dependency_target
  }
}

/// Get the current input dependencies for the existing `flake.nix`.
///
/// # Arguments
///
/// * `flake_dir_path` - Path to the flake directory containing `flake.nix`.
/// * `timeout` - Maximum time allowed for the metadata command.
///
/// # Returns
///
/// Returns a map of input names to dependency replacement entries.
///
/// # Errors
///
/// Returns an error if the metadata command fails, the JSON output cannot be
/// parsed, or the flake inputs are missing or malformed.
pub fn get_input_deps(
  flake_dir_path: &Path,
  timeout: Duration,
) -> Result<HashMap<String, Vec<InputReplacement>>, FetchError> {
  if !PathBuf::from(flake_dir_path).join("flake.lock").exists() {
    tracing::warn!(
      "Flake.lock file does not exist in: {}, taking longer to rebuild flake \
       graph",
      flake_dir_path.display()
    );
  }

  let cmd =
    DEPENDENCIES_CMD.replace("{PATH}", &flake_dir_path.display().to_string());
  let output = with_command_spinner!(
    "Parsing flake input dependency tree",
    &cmd,
    timeout
  )?;

  if output.status.success() {
    let mut deps_map: HashMap<String, HashMap<String, Vec<String>>> =
      serde_json::from_slice(&output.stdout)?;

    if deps_map.is_empty() {
      tracing::warn!("Found empty input dependencies map, is the flake blank?");
      return Err(FetchError::NoFlakeInputs);
    }

    tracing::trace!("Dependencies map before filtering: {deps_map:#?}");

    let root_entry = if let Some(val) = deps_map.get("root") {
      val.clone()
    } else {
      tracing::warn!("Missing flake root entry, can't validate user_inputs");
      return Err(FetchError::MalformedFlake);
    };

    deps_map.retain(|k, deps| {
      k != "root" && !deps.is_empty() && root_entry.contains_key(k)
    });
    tracing::trace!("Dependencies map after filtering: {deps_map:#?}");

    let mut dupe_map = HashMap::<String, Vec<InputReplacement>>::new();
    for (input, deps) in deps_map {
      tracing::trace!("Dependencies of {input}: {deps:#?}");

      let mut new_targets = Vec::<InputReplacement>::new();
      for (dependency, target) in deps {
        if target.len() > 1 || target.is_empty() {
          tracing::trace!(
            "[{input}]: Found irregular target: {target:?} for {dependency}"
          );
          continue;
        }

        if ends_with_2_to_99(&target[0]) {
          let new_target =
            format!("inputs.{dependency}.follows = \"{dependency}\";");
          tracing::trace!(
            "[{input}]: Setting target for {dependency}: {new_target}"
          );
          let replacement =
            InputReplacement::new(&dependency, &target[0], &new_target);
          new_targets.push(replacement);
        }
      }
      if !new_targets.is_empty() {
        dupe_map.insert(input.clone(), new_targets);
      }
    }

    tracing::debug!("Final duplicate dependency map: {dupe_map:#?}");
    Ok(dupe_map)
  } else {
    let stdout_str = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(1);
    Err(FetchError::CommandError(CommandError::NonZeroExitCode(
      code, stderr_str, stdout_str,
    )))
  }
}

fn ends_with_2_to_99(s: &str) -> bool {
  if let Some((_, n)) = s.rsplit_once('_')
    && let Ok(v) = n.parse::<u8>()
  {
    return (2..=99).contains(&v);
  }
  false
}
