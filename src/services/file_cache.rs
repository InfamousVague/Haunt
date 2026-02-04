//! File-based cache for persistent storage.
//!
//! Provides disk persistence for API data so the service can continue
//! operating even if external APIs go down.

#![allow(dead_code)]

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

        let entry = CacheEntry { data, timestamp };

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn create_test_cache(name: &str) -> FileCache {
        let cache_dir = PathBuf::from(format!(".test_cache_{}", name));
        if cache_dir.exists() {
            let _ = fs::remove_dir_all(&cache_dir);
        }
        let _ = fs::create_dir_all(&cache_dir);
        FileCache { cache_dir }
    }

    fn cleanup_test_cache(cache: &FileCache) {
        let _ = fs::remove_dir_all(&cache.cache_dir);
    }

    #[test]
    fn test_file_cache_set_and_get() {
        let cache = create_test_cache("set_get");

        cache.set("test_key", &"test_value".to_string());
        let result: Option<String> = cache.get("test_key", Duration::from_secs(60));

        assert_eq!(result, Some("test_value".to_string()));
        cleanup_test_cache(&cache);
    }

    #[test]
    fn test_file_cache_get_nonexistent() {
        let cache = create_test_cache("nonexistent");

        let result: Option<String> = cache.get("nonexistent_key", Duration::from_secs(60));

        assert!(result.is_none());
        cleanup_test_cache(&cache);
    }

    #[test]
    fn test_file_cache_expired_entry() {
        let cache = create_test_cache("expired");

        cache.set("test_key", &"test_value".to_string());
        // Sleep for 2 seconds to ensure expiry (file cache uses second granularity)
        thread::sleep(Duration::from_secs(2));
        // Use 1 second max_age - entry should be expired
        let result: Option<String> = cache.get("test_key", Duration::from_secs(1));

        assert!(result.is_none(), "Expected entry to be expired");
        cleanup_test_cache(&cache);
    }

    #[test]
    fn test_file_cache_get_stale() {
        let cache = create_test_cache("stale");

        cache.set("test_key", &"test_value".to_string());
        // Sleep for 2 seconds to ensure expiry (file cache uses second granularity)
        thread::sleep(Duration::from_secs(2));

        // get() with 1 second max_age should fail due to expiry
        let fresh: Option<String> = cache.get("test_key", Duration::from_secs(1));
        assert!(
            fresh.is_none(),
            "Expected fresh check to fail due to expiry"
        );

        // get_stale() should still work regardless of age
        let stale: Option<String> = cache.get_stale("test_key");
        assert_eq!(stale, Some("test_value".to_string()));

        cleanup_test_cache(&cache);
    }

    #[test]
    fn test_file_cache_remove() {
        let cache = create_test_cache("remove");

        cache.set("test_key", &"test_value".to_string());
        cache.remove("test_key");
        let result: Option<String> = cache.get_stale("test_key");

        assert!(result.is_none());
        cleanup_test_cache(&cache);
    }

    #[test]
    fn test_file_cache_struct_data() {
        let cache = create_test_cache("struct");

        #[derive(Debug, Clone, PartialEq, Serialize, serde::Deserialize)]
        struct TestData {
            name: String,
            value: i32,
        }

        let data = TestData {
            name: "test".to_string(),
            value: 42,
        };

        cache.set("struct_key", &data);
        let result: Option<TestData> = cache.get("struct_key", Duration::from_secs(60));

        assert_eq!(result, Some(data));
        cleanup_test_cache(&cache);
    }

    #[test]
    fn test_file_cache_key_sanitization() {
        let cache = create_test_cache("sanitize");

        // Keys with special characters should be sanitized
        cache.set("test/key:with*special?chars", &"value".to_string());
        let result: Option<String> =
            cache.get("test/key:with*special?chars", Duration::from_secs(60));

        assert_eq!(result, Some("value".to_string()));
        cleanup_test_cache(&cache);
    }

    #[test]
    fn test_file_cache_overwrite() {
        let cache = create_test_cache("overwrite");

        cache.set("key", &"value1".to_string());
        cache.set("key", &"value2".to_string());
        let result: Option<String> = cache.get("key", Duration::from_secs(60));

        assert_eq!(result, Some("value2".to_string()));
        cleanup_test_cache(&cache);
    }

    #[test]
    fn test_file_cache_vec_data() {
        let cache = create_test_cache("vec");

        let data = vec![1, 2, 3, 4, 5];
        cache.set("vec_key", &data);
        let result: Option<Vec<i32>> = cache.get("vec_key", Duration::from_secs(60));

        assert_eq!(result, Some(data));
        cleanup_test_cache(&cache);
    }
}
