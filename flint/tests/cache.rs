use std::{
    collections::HashMap,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use flint::{
    cache::{read_cache, write_cache},
    errors::FetchError,
    metadata::get_all_remote_modified_times,
    modified_time::RemoteInput,
};
use serde_json::Value;
use tempfile::tempdir;
use tracing_test::traced_test;

fn remote_input(input_name: &str, input_url: &str) -> RemoteInput {
    RemoteInput {
        input_name: input_name.to_owned(),
        input_url:  input_url.to_owned(),
    }
}

fn set_cache_fetch_time(cache_file_path: &Path, fetch_time: u64) -> Result<()> {
    let contents =
        std::fs::read_to_string(cache_file_path).with_context(|| {
            format!("failed to read {}", cache_file_path.display())
        })?;

    let mut json: Value = serde_json::from_str(&contents)
        .context("failed to parse cache file as JSON")?;

    json["fetch_time"] = Value::from(fetch_time);

    std::fs::write(
        cache_file_path,
        serde_json::to_string_pretty(&json)
            .context("failed to serialize modified cache JSON")?,
    )
    .with_context(|| {
        format!("failed to write {}", cache_file_path.display())
    })?;

    Ok(())
}

#[test]
#[traced_test]
fn write_cache_creates_parent_directories_and_read_cache_returns_entries()
-> Result<()> {
    let dir = tempdir()?;
    let cache_file_path = dir.path().join("nested/cache/flint.json");

    let cached_ok = remote_input("nixpkgs", "github:NixOS/nixpkgs");
    let cached_err = remote_input("broken", "not-a-valid-flake-url");

    let entries = HashMap::from([
        (cached_ok.clone(), Ok(1_700_000_000)),
        (
            cached_err.clone(),
            Err(FetchError::CachedFetchError(
                "cached fetch failed".to_owned(),
            )),
        ),
    ]);

    write_cache(&entries, cache_file_path.clone())?;

    assert!(
        cache_file_path.is_file(),
        "expected cache file to be created at {}",
        cache_file_path.display(),
    );

    let cached = read_cache(&cache_file_path, 86_400)
        .expect("expected fresh cache to be readable");

    assert_eq!(
        cached.len(),
        2,
        "expected both cached entries to round-trip, got {cached:#?}",
    );

    assert_eq!(
        cached
            .get(&cached_ok)
            .and_then(|result| result.as_ref().ok())
            .copied(),
        Some(1_700_000_000),
    );

    let err = cached
        .get(&cached_err)
        .expect("expected cached error entry")
        .as_ref()
        .expect_err("expected cached entry to remain an error");

    assert!(
        err.to_string().contains("cached fetch failed"),
        "expected cached error message to round-trip, got {err}",
    );

    Ok(())
}

#[test]
#[traced_test]
fn read_cache_returns_none_for_missing_invalid_and_expired_cache() -> Result<()>
{
    let dir = tempdir()?;

    let missing_cache_file = dir.path().join("missing/flint.json");
    assert!(
        read_cache(&missing_cache_file, 86_400).is_none(),
        "missing cache files should be ignored",
    );

    let invalid_cache_file = dir.path().join("invalid.json");
    std::fs::write(&invalid_cache_file, "not valid json")?;
    assert!(
        read_cache(&invalid_cache_file, 86_400).is_none(),
        "invalid cache contents should be ignored",
    );

    let expired_cache_file = dir.path().join("expired/flint.json");
    let input = remote_input("nixpkgs", "github:NixOS/nixpkgs");

    write_cache(
        &HashMap::from([(input, Ok(1_700_000_000))]),
        expired_cache_file.clone(),
    )?;

    set_cache_fetch_time(&expired_cache_file, 0)?;

    assert!(
        read_cache(&expired_cache_file, 86_400).is_none(),
        "cache with old fetch_time should be treated as expired",
    );

    Ok(())
}

#[test]
#[traced_test]
fn get_all_remote_modified_times_returns_cached_entries_without_fetching()
-> Result<()> {
    let dir = tempdir()?;
    let cache_file_path = dir.path().join("flint.json");

    let input = remote_input("cached-input", "not-a-valid-flake-url");
    write_cache(
        &HashMap::from([(input.clone(), Ok(4_242))]),
        cache_file_path.clone(),
    )?;

    let fetched = get_all_remote_modified_times(
        std::slice::from_ref(&input),
        Duration::from_millis(1),
        cache_file_path,
        86_400,
    );

    assert_eq!(
        fetched
            .get(&input)
            .and_then(|result| result.as_ref().ok())
            .copied(),
        Some(4_242),
        "expected cached value to be returned instead of fetching remote input",
    );

    Ok(())
}

#[test_group::group(nix_sandbox_incompatible)]
#[test]
#[traced_test]
fn get_all_remote_modified_times_writes_cache_after_cache_miss() -> Result<()> {
    let dir = tempdir()?;
    let cache_file_path = dir.path().join("new/cache/flint.json");

    let input = remote_input("invalid-input", "not-a-valid-flake-url");

    let fetched = get_all_remote_modified_times(
        std::slice::from_ref(&input),
        Duration::from_secs(10),
        cache_file_path.clone(),
        86_400,
    );

    assert!(
        fetched
            .get(&input)
            .expect("expected result for requested input")
            .is_err(),
        "invalid remote input should produce a cached fetch error",
    );

    assert!(
        cache_file_path.is_file(),
        "expected cache file to be written after cache miss",
    );

    let cached = read_cache(&cache_file_path, 86_400)
        .expect("expected newly written cache to be readable");

    assert!(
        cached
            .get(&input)
            .expect("expected failed fetch to be cached")
            .is_err(),
        "expected failed fetch result to be persisted in cache",
    );

    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let contents = std::fs::read_to_string(&cache_file_path)?;
    let json: Value = serde_json::from_str(&contents)?;

    let fetch_time = json
        .get("fetch_time")
        .and_then(Value::as_u64)
        .expect("cache should include numeric fetch_time");

    assert!(
        fetch_time <= now,
        "cache fetch_time should not be in the future",
    );

    Ok(())
}
