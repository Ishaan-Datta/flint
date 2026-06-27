use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{
    errors::{FetchError, cache::CacheError},
    modified_time::RemoteInput,
};

/// JSON-serializable cache payload for remote flake input checks.
///
/// `fetch_time` records when the cache was written, as seconds since the Unix
/// epoch. `cached_entries` stores one cached fetch result per remote input.
#[derive(Serialize, Deserialize)]
pub struct UpdateCache {
    fetch_time:     u64,
    cached_entries: Vec<CacheEntry>,
}

/// Cached fetch result for a single remote flake input.
///
/// The result is either the remote `lastModified` timestamp or the fetch error
/// that occurred while querying that input. Cached `FetchError` values are
/// serialized as their display strings and deserialize as
/// `FetchError::CachedFetchError`.
#[derive(Serialize, Deserialize)]
pub struct CacheEntry {
    input:  RemoteInput,
    result: Result<u64, FetchError>,
}

/// Write remote modified-time fetch results to a cache file.
///
/// Creates the parent cache directory when it does not already exist. The cache
/// file is written as pretty-printed JSON with a single `fetch_time` shared by
/// all entries.
///
/// # Arguments
///
/// * `entries` - Remote inputs mapped to their fetched timestamps or fetch
///   errors.
/// * `cache_file_path` - Destination path for the cache file.
///
/// # Errors
///
/// Returns `CacheError` if the cache directory cannot be created, the cache
/// payload cannot be serialized, or the cache file cannot be written.
///
/// # Panics
///
/// Panics if `cache_file_path` has no parent directory, or if the system time
/// is earlier than the Unix epoch.
pub fn write_cache(
    entries: &HashMap<RemoteInput, Result<u64, FetchError>>,
    cache_file_path: PathBuf,
) -> Result<(), CacheError> {
    let cache_dir = cache_file_path
        .parent()
        .expect("Cache file path should have parent");

    if !cache_dir.exists() {
        std::fs::create_dir_all(cache_dir)?;
    }

    let cache = UpdateCache {
        fetch_time:     SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time travel detected")
            .as_secs(),
        cached_entries: entries
            .iter()
            .map(|(input, result)| {
                CacheEntry {
                    input:  input.clone(),
                    result: result.clone(),
                }
            })
            .collect(),
    };

    let json = serde_json::to_string_pretty(&cache)?;
    std::fs::write(cache_file_path, json)?;

    Ok(())
}

/// Read remote modified-time fetch results from a cache file.
///
/// Invalid, unreadable, expired, malformed, or future-dated cache files are
/// treated as cache misses and return `None`.
///
/// # Arguments
///
/// * `cache_file_path` - Path to the cache file.
/// * `cache_expiry` - Maximum cache age in seconds.
///
/// # Returns
///
/// Returns `Some` with cached remote inputs mapped to their timestamps or
/// cached fetch errors when the cache exists and is still valid. Returns `None`
/// when the cache cannot be used.
///
/// Cached errors deserialize as `FetchError::CachedFetchError` because the
/// original error variant cannot be reconstructed from the serialized display
/// string.
///
/// # Panics
///
/// Panics if the system time is earlier than the Unix epoch.
pub fn read_cache(
    cache_file_path: &Path,
    cache_expiry: u64,
) -> Option<HashMap<RemoteInput, Result<u64, FetchError>>> {
    if !cache_file_path.exists() {
        tracing::debug!("Cache file does not exist");
        return None;
    }

    let contents = match std::fs::read_to_string(cache_file_path) {
        Ok(val) => val,
        Err(e) => {
            tracing::debug!("Failed to read file to string: {e}");
            return None;
        },
    };

    let cache: UpdateCache = match serde_json::from_str(&contents) {
        Ok(cache) => cache,
        Err(e) => {
            tracing::debug!("Failed to parse cache file contents: {e}");
            return None;
        },
    };

    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect(
            "Time travel detected (System time set earlier than the unix \
             epoch)",
        )
        .as_secs();

    let difference = match current_time.checked_sub(cache.fetch_time) {
        Some(diff) => diff,
        None => {
            tracing::debug!("Cache fetch_time is in the future");
            return None;
        },
    };

    if difference >= cache_expiry {
        tracing::debug!("Cache expired: {difference}s old");
        None
    } else {
        Some(
            cache
                .cached_entries
                .into_iter()
                .map(|entry| (entry.input, entry.result))
                .collect(),
        )
    }
}
