mod common;
use std::fs;

use common::{
    INVALID_FLAKE_CONTENT,
    TIMEOUT,
    VALID_FLAKE_CONTENT,
    VALID_FLAKE_LOCK_CONTENT,
    assert_file_matches,
    make_flake_file,
    make_git_directory,
    make_lock_file,
    stage_git_file,
};
use flint::ast::write::{
    GitFileStatus,
    run_git_file_status,
    write_new_flake_file,
};
use tracing_test::traced_test;

const OVERRIDE: bool = true;
const QUIET: bool = true;

// Git tracked tests
#[test]
#[traced_test]
fn run_git_file_status_returns_not_repo_when_file_is_not_inside_git_repository()
{
    let dir = tempfile::tempdir().expect("should create temporary directory");

    let flake_path = make_flake_file("{ outputs = _: {}; }", &dir)
        .expect("should write flake");

    let status = run_git_file_status(&flake_path, "flake.nix", TIMEOUT)
        .expect("git status check should succeed");

    assert_eq!(status, GitFileStatus::NotRepo);
}

#[test]
#[traced_test]
fn run_git_file_status_returns_clean_when_file_has_no_unstaged_changes() {
    let dir = make_git_directory().expect("should create git repository");

    let flake_path = make_flake_file(
        r#"
{
  outputs = _: {};
}
"#,
        &dir,
    )
    .expect("should write flake");

    stage_git_file(dir.path(), "flake.nix").expect("should stage flake");

    let status = run_git_file_status(&flake_path, "flake.nix", TIMEOUT)
        .expect("git status check should succeed");

    assert_eq!(status, GitFileStatus::Clean);
}

#[test]
#[traced_test]
fn run_git_file_status_returns_dirty_when_file_has_unstaged_changes() {
    let dir = make_git_directory().expect("should create git repository");

    let flake_path = make_flake_file(
        r#"
{
  outputs = _: {};
}
"#,
        &dir,
    )
    .expect("should write flake");

    stage_git_file(dir.path(), "flake.nix").expect("should stage flake");

    std::fs::write(
        &flake_path,
        r#"
{
  description = "modified";
  outputs = _: {};
}
"#,
    )
    .expect("should modify flake");

    let status = run_git_file_status(&flake_path, "flake.nix", TIMEOUT)
        .expect("git status check should succeed");

    assert_eq!(status, GitFileStatus::Dirty);
}

// File system operation tests
#[test_group::group(nix_sandbox_incompatible)]
#[test]
#[traced_test]
fn writes_valid_flake_without_backup() -> Result<(), anyhow::Error> {
    let dir = tempfile::tempdir()?;

    let flake_path = make_flake_file(VALID_FLAKE_CONTENT, &dir)?;
    make_lock_file(VALID_FLAKE_LOCK_CONTENT, &dir)?;

    write_new_flake_file(
        dir.path(),
        VALID_FLAKE_CONTENT,
        OVERRIDE,
        QUIET,
        false,
        TIMEOUT,
    )?;

    assert_file_matches(&flake_path, VALID_FLAKE_CONTENT)?;

    assert!(
        !dir.path().join("flake.nix.bak").exists(),
        "backup file should not exist when backup=false",
    );

    Ok(())
}

#[test_group::group(nix_sandbox_incompatible)]
#[test]
#[traced_test]
fn writes_valid_flake_and_creates_backup_when_requested()
-> Result<(), anyhow::Error> {
    let dir = tempfile::tempdir()?;

    let flake_path = make_flake_file(VALID_FLAKE_CONTENT, &dir)?;
    make_lock_file(VALID_FLAKE_LOCK_CONTENT, &dir)?;

    write_new_flake_file(
        dir.path(),
        VALID_FLAKE_CONTENT,
        OVERRIDE,
        QUIET,
        true,
        TIMEOUT,
    )?;

    let backup_path = dir.path().join("flake.nix.bak");

    assert!(
        flake_path.exists(),
        "new flake.nix should exist after write",
    );

    assert!(
        backup_path.exists(),
        "flake.nix.bak should exist when backup=true",
    );

    assert_file_matches(&flake_path, VALID_FLAKE_CONTENT)?;
    assert_file_matches(&backup_path, VALID_FLAKE_CONTENT)?;

    Ok(())
}

#[test]
#[traced_test]
fn rejects_invalid_flake_and_preserves_original_without_backup()
-> Result<(), anyhow::Error> {
    let dir = tempfile::tempdir()?;

    let flake_path = make_flake_file(VALID_FLAKE_CONTENT, &dir)?;
    make_lock_file(VALID_FLAKE_LOCK_CONTENT, &dir)?;

    let result = write_new_flake_file(
        dir.path(),
        INVALID_FLAKE_CONTENT,
        OVERRIDE,
        QUIET,
        false,
        TIMEOUT,
    );

    assert!(
        result.is_err(),
        "invalid flake content should fail validation",
    );

    assert_file_matches(&flake_path, VALID_FLAKE_CONTENT)?;

    assert!(
        !dir.path().join("flake.nix.bak").exists(),
        "backup file should not be created when backup=false",
    );

    Ok(())
}

#[test]
#[traced_test]
fn rejects_invalid_flake_before_creating_backup() -> Result<(), anyhow::Error> {
    let dir = tempfile::tempdir()?;

    let flake_path = make_flake_file(VALID_FLAKE_CONTENT, &dir)?;
    make_lock_file(VALID_FLAKE_LOCK_CONTENT, &dir)?;

    let result = write_new_flake_file(
        dir.path(),
        INVALID_FLAKE_CONTENT,
        OVERRIDE,
        QUIET,
        true,
        TIMEOUT,
    );

    assert!(
        result.is_err(),
        "invalid flake content should fail validation",
    );

    assert_file_matches(&flake_path, VALID_FLAKE_CONTENT)?;

    assert!(
        !dir.path().join("flake.nix.bak").exists(),
        "backup should not be created if validation fails before file \
         operations",
    );

    Ok(())
}

#[test_group::group(nix_sandbox_incompatible)]
#[test]
#[traced_test]
fn overwrites_dirty_git_tracked_flake_when_override_is_true()
-> Result<(), anyhow::Error> {
    let dir = make_git_directory()?;

    let flake_path = make_flake_file(VALID_FLAKE_CONTENT, &dir)?;
    make_lock_file(VALID_FLAKE_LOCK_CONTENT, &dir)?;

    stage_git_file(dir.path(), "flake.nix")?;

    fs::write(
        &flake_path,
        format!("{VALID_FLAKE_CONTENT}\n# local dirty edit\n"),
    )?;

    write_new_flake_file(
        dir.path(),
        VALID_FLAKE_CONTENT,
        OVERRIDE,
        QUIET,
        false,
        TIMEOUT,
    )?;

    assert_file_matches(&flake_path, VALID_FLAKE_CONTENT)?;

    Ok(())
}
