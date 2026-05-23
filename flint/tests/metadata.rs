use std::{collections::HashMap, path::Path, time::Duration};

use anyhow::{Context, Result};
use tempfile::tempdir;
use tracing_test::traced_test;

mod common;
use common::{
  INVALID_FLAKE_CONTENT,
  SHORT_FLAKE_CONTENT,
  TIMEOUT,
  VALID_FLAKE_CONTENT,
  VALID_FLAKE_LOCK_CONTENT,
  assert_single_input_url,
  make_flake_file,
  make_git_directory,
  make_lock_file,
  path_to_string,
  stage_git_file,
};
use flint::metadata::{
  get_all_local_modified_times,
  get_flake_path,
  get_input_deps,
  get_input_urls,
  get_remote_modified_time,
};

const EMPTY_INPUTS_FLAKE_CONTENT: &str = r#"
{
  description = "empty inputs test flake";

  inputs = { };

  outputs = { self }: { };
}
"#;

const FOLLOWS_ONLY_INPUT_FLAKE_CONTENT: &str = r#"
{
  description = "flake with an input that has no url";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    follows-only = {
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, ... }: { };
}
"#;

const EMPTY_ROOT_LOCK_CONTENT: &str = r#"
{
  "nodes": {
    "root": {
      "inputs": { }
    }
  },
  "root": "root",
  "version": 7
}
"#;

const MALFORMED_LOCK_CONTENT: &str = r#"
{
  "nodes": {
    "root": {
      "inputs": {
"#;

const NO_DUPLICATE_DEPS_FLAKE_CONTENT: &str = r#"
{
  description = "no duplicate dependency replacement fixture";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    systems.url = "github:nix-systems/default";
  };

  outputs = { self, ... }: { };
}
"#;

#[test]
#[traced_test]
fn get_input_urls_returns_github_url() -> Result<()> {
  assert_single_input_url("github", "github:nix-systems/default")
}

#[test]
#[traced_test]
fn get_input_urls_returns_github_ref_url() -> Result<()> {
  assert_single_input_url("github-ref", "github:NixOS/nixpkgs/nixos-unstable")
}

#[test]
#[traced_test]
fn get_input_urls_returns_gitlab_url() -> Result<()> {
  assert_single_input_url(
    "gitlab",
    "gitlab:simple-nixos-mailserver/nixos-mailserver",
  )
}

#[test]
#[traced_test]
fn get_input_urls_returns_codeberg_url() -> Result<()> {
  assert_single_input_url(
    "codeberg",
    "git+https://codeberg.org/BANanaD3V/niri-nix",
  )
}

#[test]
#[traced_test]
fn get_input_urls_returns_sourcehut_url() -> Result<()> {
  assert_single_input_url(
    "sourcehut",
    "git+https://git.sr.ht/~andreafeletto/buongiorno",
  )
}

#[test]
#[traced_test]
fn get_input_urls_returns_generic_git_url() -> Result<()> {
  assert_single_input_url(
    "generic-git",
    "git+https://github.com/numtide/flake-utils",
  )
}

#[test]
#[traced_test]
fn get_input_urls_returns_tarball_url() -> Result<()> {
  assert_single_input_url(
    "tarball",
    "https://channels.nixos.org/nixos-unstable/nixexprs.tar.xz",
  )
}

#[test]
#[traced_test]
fn get_input_urls_returns_empty_map_for_empty_inputs_attrset() -> Result<()> {
  let dir = tempdir()?;
  make_flake_file(EMPTY_INPUTS_FLAKE_CONTENT, &dir)?;

  let urls = get_input_urls(TIMEOUT, dir.path())?;

  assert!(urls.is_empty(), "expected no inputs, got {urls:#?}");
  Ok(())
}

#[test]
#[traced_test]
fn get_input_urls_errors_for_invalid_flake_file() -> Result<()> {
  let dir = tempdir()?;
  make_flake_file(INVALID_FLAKE_CONTENT, &dir)?;

  let err = get_input_urls(TIMEOUT, dir.path())
    .expect_err("expected error for invalid flake file");

  assert!(
    format!("{err:?}").contains("NonZeroExitCode")
      || format!("{err:?}").contains("CommandError"),
    "expected command failure for invalid flake, got {err:?}",
  );

  Ok(())
}

#[test]
#[traced_test]
fn get_input_urls_errors_when_an_input_has_no_url() -> Result<()> {
  let dir = tempdir()?;
  make_flake_file(FOLLOWS_ONLY_INPUT_FLAKE_CONTENT, &dir)?;

  let err = get_input_urls(TIMEOUT, dir.path())
    .expect_err("expected error when input has no url");

  // URL_CMD emits null for inputs without `url`.
  // The production function currently deserializes into HashMap<String,
  // String>, so a null value should fail JSON deserialization.
  assert!(
    format!("{err:?}").contains("invalid type")
      || format!("{err:?}").contains("Json")
      || format!("{err:?}").contains("Serde"),
    "expected JSON deserialization failure for null URL, got {err:?}",
  );

  Ok(())
}

#[test]
#[traced_test]
fn get_input_urls_errors_when_flake_nix_is_missing() -> Result<()> {
  let dir = tempdir()?;

  let err = get_input_urls(TIMEOUT, dir.path())
    .expect_err("expected error for missing flake.nix");

  assert!(
    format!("{err:?}").contains("NonZeroExitCode")
      || format!("{err:?}").contains("CommandError"),
    "expected command failure for missing flake.nix, got {err:?}",
  );

  Ok(())
}

#[test]
#[traced_test]
fn get_all_local_modified_times_returns_values_for_locked_inputs() -> Result<()>
{
  let dir = tempdir()?;
  make_flake_file(VALID_FLAKE_CONTENT, &dir)?;
  make_lock_file(VALID_FLAKE_LOCK_CONTENT, &dir)?;

  let times = get_all_local_modified_times(TIMEOUT, dir.path())?;

  assert_eq!(times.get("nixpkgs_2"), Some(&Some(1772542754)));
  assert_eq!(times.get("nixpkgs-stable"), Some(&Some(1776221942)));
  assert_eq!(times.get("systems"), Some(&Some(1681028828)));
  assert_eq!(times.get("flake-utils"), Some(&Some(1731533236)));

  // Root has no `locked.lastModified`.
  assert_eq!(times.get("root"), Some(&None));

  assert!(
    times.len() > 10,
    "expected real lock metadata, got {times:#?}",
  );

  Ok(())
}

#[test]
#[traced_test]
fn get_all_local_modified_times_returns_empty_map_for_empty_root_inputs()
-> Result<()> {
  let dir = tempdir()?;
  make_flake_file(EMPTY_INPUTS_FLAKE_CONTENT, &dir)?;
  make_lock_file(EMPTY_ROOT_LOCK_CONTENT, &dir)?;

  let times = get_all_local_modified_times(TIMEOUT, dir.path())?;

  assert_eq!(
    times,
    HashMap::from([("root".to_string(), None)]),
    "expected only root with no modified time, got {times:#?}",
  );

  Ok(())
}

#[test]
#[traced_test]
fn get_all_local_modified_times_errors_for_malformed_lock_file() -> Result<()> {
  let dir = tempdir()?;
  make_flake_file(EMPTY_INPUTS_FLAKE_CONTENT, &dir)?;
  make_lock_file(MALFORMED_LOCK_CONTENT, &dir)?;

  let err = get_all_local_modified_times(TIMEOUT, dir.path())
    .expect_err("expected error for malformed flake.lock");

  assert!(
    format!("{err:?}").contains("NonZeroExitCode")
      || format!("{err:?}").contains("CommandError"),
    "expected command failure for malformed flake.lock, got {err:?}",
  );

  Ok(())
}

#[test]
#[traced_test]
fn get_remote_modified_time_errors_for_invalid_url() {
  let err =
    get_remote_modified_time("not-a-valid-flake-url", Duration::from_secs(10))
      .expect_err("expected error for invalid flake url");

  assert!(
    format!("{err:?}").contains("NonZeroExitCode")
      || format!("{err:?}").contains("CommandError"),
    "expected command failure for invalid URL, got {err:?}",
  );
}

fn assert_remote_modified_time(name: &str, url: &str) -> Result<()> {
  let modified_time =
    get_remote_modified_time(url, TIMEOUT).with_context(|| {
      format!("failed to get remote modified time for {name}: {url}")
    })?;

  assert!(
    modified_time > 1_500_000_000,
    "{name} returned an implausibly old timestamp: {modified_time}",
  );

  assert!(
    modified_time < 4_102_444_800,
    "{name} returned an implausibly future timestamp: {modified_time}",
  );

  Ok(())
}

#[test]
fn get_remote_modified_time_supports_github() -> Result<()> {
  assert_remote_modified_time("github", "github:nix-systems/default")
}

#[test]
fn get_remote_modified_time_supports_github_ref() -> Result<()> {
  assert_remote_modified_time(
    "github-ref",
    "github:NixOS/nixpkgs/nixos-unstable",
  )
}

#[test]
fn get_remote_modified_time_supports_gitlab() -> Result<()> {
  assert_remote_modified_time(
    "gitlab",
    "gitlab:simple-nixos-mailserver/nixos-mailserver",
  )
}

#[test]
fn get_remote_modified_time_supports_codeberg() -> Result<()> {
  assert_remote_modified_time(
    "codeberg",
    "git+https://codeberg.org/BANanaD3V/niri-nix",
  )
}

#[test]
fn get_remote_modified_time_supports_sourcehut() -> Result<()> {
  assert_remote_modified_time(
    "sourcehut",
    "git+https://git.sr.ht/~andreafeletto/buongiorno",
  )
}

#[test]
fn get_remote_modified_time_supports_generic_git() -> Result<()> {
  assert_remote_modified_time(
    "generic-git",
    "git+https://github.com/numtide/flake-utils",
  )
}

#[test]
fn get_remote_modified_time_supports_tarball() -> Result<()> {
  assert_remote_modified_time(
    "tarball",
    "https://channels.nixos.org/nixos-unstable/nixexprs.tar.xz",
  )
}

#[test]
#[traced_test]
fn get_flake_path_resolves_valid_git_tracked_local_flake_directory()
-> Result<()> {
  let dir = make_git_directory()?;
  make_flake_file(SHORT_FLAKE_CONTENT, &dir)?;
  stage_git_file(dir.path(), "flake.nix")?;

  let flake_path = get_flake_path(&path_to_string(dir.path()), TIMEOUT)?;

  assert!(
    flake_path.exists(),
    "resolved flake path should exist: {}",
    flake_path.display(),
  );
  assert!(
    flake_path.is_dir(),
    "resolved flake path should be a directory: {}",
    flake_path.display(),
  );
  assert!(
    flake_path.join("flake.nix").is_file(),
    "resolved flake path should contain flake.nix: {}",
    flake_path.display(),
  );

  Ok(())
}

#[test]
#[traced_test]
fn get_flake_path_errors_when_flake_file_is_untracked_in_git_repo() -> Result<()>
{
  let dir = make_git_directory()?;
  make_flake_file(SHORT_FLAKE_CONTENT, &dir)?;

  let err = get_flake_path(&path_to_string(dir.path()), TIMEOUT).expect_err(
    "expected error when flake.nix exists but is not tracked by git",
  );

  assert!(
    format!("{err:?}").contains("NonZeroExitCode")
      || format!("{err:?}").contains("CommandError"),
    "expected command failure for untracked flake.nix, got {err:?}",
  );

  Ok(())
}

#[test]
#[traced_test]
fn get_flake_path_errors_for_untracked_local_flake_directory() -> Result<()> {
  let dir = tempdir()?;
  make_flake_file(SHORT_FLAKE_CONTENT, &dir)?;

  let err = get_flake_path(&path_to_string(dir.path()), TIMEOUT)
    .expect_err("expected error for local flake directory not tracked by git");

  assert!(
    format!("{err:?}").contains("NonZeroExitCode")
      || format!("{err:?}").contains("CommandError"),
    "expected command failure for untracked local flake directory, got {err:?}",
  );

  Ok(())
}

#[test]
#[traced_test]
fn get_flake_path_accepts_trailing_slash_for_git_tracked_local_flake_directory()
-> Result<()> {
  let dir = make_git_directory()?;
  make_flake_file(SHORT_FLAKE_CONTENT, &dir)?;
  stage_git_file(dir.path(), "flake.nix")?;

  let input_path = format!("{}/", dir.path().display());
  let flake_path = get_flake_path(&input_path, TIMEOUT)?;

  assert!(
    flake_path.is_dir(),
    "resolved flake path should be a directory: {}",
    flake_path.display(),
  );
  assert!(
    flake_path.join("flake.nix").is_file(),
    "resolved flake path should contain flake.nix: {}",
    flake_path.display(),
  );

  Ok(())
}

#[test]
#[traced_test]
fn get_flake_path_errors_for_directory_without_flake_nix() -> Result<()> {
  let dir = tempdir()?;

  let err = get_flake_path(&path_to_string(dir.path()), TIMEOUT)
    .expect_err("expected error for directory without flake.nix");

  assert!(
    format!("{err:?}").contains("NonZeroExitCode")
      || format!("{err:?}").contains("CommandError")
      || format!("{err:?}").contains("InvalidPath"),
    "expected command or invalid path error for directory without flake.nix, \
     got {err:?}",
  );

  Ok(())
}

#[test]
#[traced_test]
fn get_flake_path_errors_for_nonexistent_path() {
  let missing_path = "/definitely/not/a/real/flake/path";

  let err = get_flake_path(missing_path, Duration::from_secs(10))
    .expect_err("expected error for nonexistent flake path");

  assert!(
    format!("{err:?}").contains("NonZeroExitCode")
      || format!("{err:?}").contains("CommandError")
      || format!("{err:?}").contains("InvalidPath"),
    "expected command or invalid path error for nonexistent path, got {err:?}",
  );
}

#[test]
#[traced_test]
fn get_flake_path_errors_for_invalid_flake_syntax() -> Result<()> {
  let dir = tempdir()?;
  make_flake_file(INVALID_FLAKE_CONTENT, &dir)?;

  let err = get_flake_path(&path_to_string(dir.path()), TIMEOUT)
    .expect_err("expected error for invalid flake syntax");

  assert!(
    format!("{err:?}").contains("NonZeroExitCode")
      || format!("{err:?}").contains("CommandError"),
    "expected command failure for invalid flake syntax, got {err:?}",
  );

  Ok(())
}

#[test]
#[traced_test]
fn get_flake_path_errors_when_input_path_points_to_file() -> Result<()> {
  let dir = tempdir()?;
  let flake_file = make_flake_file(SHORT_FLAKE_CONTENT, &dir)?;

  let err = get_flake_path(&path_to_string(&flake_file), TIMEOUT)
    .expect_err("expected error when input path is a file");

  assert!(
    format!("{err:?}").contains("NonZeroExitCode")
      || format!("{err:?}").contains("CommandError")
      || format!("{err:?}").contains("InvalidPath"),
    "expected command or invalid path error when input path is a file, got \
     {err:?}",
  );

  Ok(())
}

#[test]
#[traced_test]
fn get_input_deps_returns_empty_map_when_there_are_no_duplicate_dependencies()
-> Result<()> {
  let dir = tempdir()?;
  make_flake_file(NO_DUPLICATE_DEPS_FLAKE_CONTENT, &dir)?;
  make_lock_file(EMPTY_ROOT_LOCK_CONTENT, &dir)?;

  let deps = get_input_deps(dir.path(), TIMEOUT)?;

  assert!(
    deps.is_empty(),
    "expected no duplicate dependency replacements, got {deps:#?}",
  );

  Ok(())
}

#[test]
#[traced_test]
fn get_input_deps_errors_for_malformed_lock_file() -> Result<()> {
  let dir = tempdir()?;
  make_flake_file(VALID_FLAKE_CONTENT, &dir)?;
  make_lock_file(MALFORMED_LOCK_CONTENT, &dir)?;

  let err = get_input_deps(dir.path(), TIMEOUT)
    .expect_err("expected error for malformed flake.lock");

  assert!(
    format!("{err:?}").contains("NonZeroExitCode")
      || format!("{err:?}").contains("CommandError"),
    "expected command failure for malformed flake.lock, got {err:?}",
  );

  Ok(())
}

#[test]
#[traced_test]
fn get_input_deps_errors_for_nonexistent_flake_directory() {
  let err = get_input_deps(
    Path::new("/definitely/not/a/real/flake/path"),
    Duration::from_secs(10),
  )
  .expect_err("expected error for nonexistent flake directory");

  assert!(
    format!("{err:?}").contains("NonZeroExitCode")
      || format!("{err:?}").contains("CommandError"),
    "expected command failure for nonexistent flake directory, got {err:?}",
  );
}
