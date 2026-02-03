//! File-based cache for persistent storage.
//!
//! Provides disk persistence for API data so the service can continue
//! operating even if external APIs go down.

use serde::{de::DeserializeOwned, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};

/// Cache directory path
const CACHE_DIR: &str = ".haunt_cache";

/// File cache entry with timestamp
#[derive(Debug, Serialize, serde::Deserialize)]
struct CacheEntry<T> {
    data: T,
    timestamp: u64,
}

/// File-based cache service.
pub struct FileCache {
    cache_dir: PathBuf,
}

impl FileCache {
    /// Create a new file cache.
    pub fn new() -> Self {
        let cache_dir = PathBuf::from(CACHE_DIR);
        if !cache_dir.exists() {
            if let Err(e) = fs::create_dir_all(&cache_dir) {
                warn!("Failed to create cache directory: {}", e);
            }
        }
        Self { cache_dir }
    }

    /// Get the cache file path for a key.
    fn get_path(&self, key: &str) -> PathBuf {
        // Sanitize key for filesystem
        let safe_key = key.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
        self.cache_dir.join(format!("{}.json", safe_key))
    }

    /// Get data from cache if not expired.
    pub fn get<T: DeserializeOwned>(&self, key: &str, max_age: Duration) -> Option<T> {
        let path = self.get_path(key);

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return None,
        };

        let entry: CacheEntry<T> = match serde_json::from_str(&content) {
            Ok(e) => e,
            Err(e) => {
                warn!("Failed to parse cache entry {}: {}", key, e);
                return None;
            }
        };

        // Check if expired
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if now - entry.timestamp > max_age.as_secs() {
            debug!("Cache entry {} expired", key);
            return None;
        }

        Some(entry.data)
    }

    /// Get data from cache regardless of age (fallback mode).
    pub fn get_stale<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let path = self.get_path(key);

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return None,
        };

        let entry: CacheEntry<T> = match serde_json::from_str(&content) {
            Ok(e) => e,
            Err(e) => {
                warn!("Failed to parse stale cache entry {}: {}", key, e);
                return None;
            }
        };

        debug!("Using stale cache for {}", key);
        Some(entry.data)
    }

    /// Set data in cache.
    pub fn set<T: Serialize>(&self, key: &str, data: &T) {
        let path = self.get_path(key);

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let entry = CacheEntry {
            data,
            timestamp,
        };

        match serde_json::to_string(&entry) {
            Ok(content) => {
                if let Err(e) = fs::write(&path, content) {
                    warn!("Failed to write cache {}: {}", key, e);
                } else {
                    debug!("Cached {} to disk", key);
                }
            }
            Err(e) => {
                warn!("Failed to serialize cache {}: {}", key, e);
            }
        }
    }

    /// Remove a cache entry.
    pub fn remove(&self, key: &str) {
        let path = self.get_path(key);
        let _ = fs::remove_file(path);
    }

    /// Clean up old cache files.
    pub fn cleanup(&self, max_age: Duration) {
        let Ok(entries) = fs::read_dir(&self.cache_dir) else {
            return;
        };

        let now = SystemTime::now();

        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(age) = now.duration_since(modified) {
                        if age > max_age {
                            let _ = fs::remove_file(entry.path());
                            debug!("Removed old cache file: {:?}", entry.path());
                        }
                    }
                }
            }
        }
    }
}

impl Default for FileCache {
    fn default() -> Self {
        Self::new()
    }
}
