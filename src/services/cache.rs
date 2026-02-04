#![allow(dead_code)]

use dashmap::DashMap;
use std::time::{Duration, Instant};

/// A thread-safe cache with TTL support.
pub struct Cache<V> {
    data: DashMap<String, CacheEntry<V>>,
    default_ttl: Duration,
}

struct CacheEntry<V> {
    value: V,
    expires_at: Instant,
}

impl<V: Clone> Cache<V> {
    /// Create a new cache with the given default TTL.
    pub fn new(default_ttl: Duration) -> Self {
        Self {
            data: DashMap::new(),
            default_ttl,
        }
    }

    /// Get a value from the cache.
    pub fn get(&self, key: &str) -> Option<V> {
        let entry = self.data.get(key)?;
        if entry.expires_at > Instant::now() {
            Some(entry.value.clone())
        } else {
            drop(entry);
            self.data.remove(key);
            None
        }
    }

    /// Set a value in the cache with the default TTL.
    pub fn set(&self, key: String, value: V) {
        self.set_with_ttl(key, value, self.default_ttl);
    }

    /// Set a value in the cache with a custom TTL.
    pub fn set_with_ttl(&self, key: String, value: V, ttl: Duration) {
        self.data.insert(
            key,
            CacheEntry {
                value,
                expires_at: Instant::now() + ttl,
            },
        );
    }

    /// Check if a key exists and is not expired.
    pub fn contains(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    /// Remove a value from the cache.
    pub fn remove(&self, key: &str) -> Option<V> {
        self.data.remove(key).map(|(_, entry)| entry.value)
    }

    /// Clear all entries from the cache.
    pub fn clear(&self) {
        self.data.clear();
    }

    /// Remove all expired entries from the cache.
    pub fn cleanup(&self) {
        let now = Instant::now();
        self.data.retain(|_, entry| entry.expires_at > now);
    }

    /// Get the number of entries in the cache (including expired).
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_basic() {
        let cache = Cache::new(Duration::from_secs(60));
        cache.set("key1".to_string(), "value1".to_string());
        assert_eq!(cache.get("key1"), Some("value1".to_string()));
        assert_eq!(cache.get("key2"), None);
    }

    #[test]
    fn test_cache_expiration() {
        let cache = Cache::new(Duration::from_millis(10));
        cache.set("key1".to_string(), "value1".to_string());
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(cache.get("key1"), None);
    }

    #[test]
    fn test_cache_custom_ttl() {
        let cache = Cache::new(Duration::from_secs(60));
        cache.set_with_ttl(
            "short".to_string(),
            "value".to_string(),
            Duration::from_millis(10),
        );
        cache.set_with_ttl(
            "long".to_string(),
            "value".to_string(),
            Duration::from_secs(60),
        );

        std::thread::sleep(Duration::from_millis(20));

        assert_eq!(cache.get("short"), None);
        assert_eq!(cache.get("long"), Some("value".to_string()));
    }

    #[test]
    fn test_cache_contains() {
        let cache = Cache::new(Duration::from_secs(60));
        cache.set("key".to_string(), "value".to_string());

        assert!(cache.contains("key"));
        assert!(!cache.contains("nonexistent"));
    }

    #[test]
    fn test_cache_remove() {
        let cache = Cache::new(Duration::from_secs(60));
        cache.set("key".to_string(), "value".to_string());

        let removed = cache.remove("key");
        assert_eq!(removed, Some("value".to_string()));
        assert_eq!(cache.get("key"), None);

        // Remove nonexistent key
        let removed = cache.remove("nonexistent");
        assert_eq!(removed, None);
    }

    #[test]
    fn test_cache_clear() {
        let cache = Cache::new(Duration::from_secs(60));
        cache.set("key1".to_string(), "value1".to_string());
        cache.set("key2".to_string(), "value2".to_string());

        assert_eq!(cache.len(), 2);
        cache.clear();
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_cleanup() {
        let cache = Cache::new(Duration::from_millis(10));
        cache.set("key1".to_string(), "value1".to_string());
        cache.set_with_ttl(
            "key2".to_string(),
            "value2".to_string(),
            Duration::from_secs(60),
        );

        std::thread::sleep(Duration::from_millis(20));
        cache.cleanup();

        // key1 should be removed (expired), key2 should remain
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get("key2"), Some("value2".to_string()));
    }

    #[test]
    fn test_cache_len_and_is_empty() {
        let cache: Cache<String> = Cache::new(Duration::from_secs(60));

        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);

        cache.set("key".to_string(), "value".to_string());
        assert!(!cache.is_empty());
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_cache_overwrite() {
        let cache = Cache::new(Duration::from_secs(60));
        cache.set("key".to_string(), "value1".to_string());
        cache.set("key".to_string(), "value2".to_string());

        assert_eq!(cache.get("key"), Some("value2".to_string()));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_cache_numeric_values() {
        let cache: Cache<i32> = Cache::new(Duration::from_secs(60));
        cache.set("count".to_string(), 42);

        assert_eq!(cache.get("count"), Some(42));
    }
}
