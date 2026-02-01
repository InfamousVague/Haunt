import { logger } from "../utils/logger.js";

type CacheEntry<T> = {
  data: T;
  expiresAt: number;
  createdAt: number;
};

type CacheOptions = {
  ttl: number; // Time to live in milliseconds
};

/**
 * In-memory cache with TTL support.
 * Provides type-safe caching for API responses.
 */
export class Cache {
  private store = new Map<string, CacheEntry<unknown>>();
  private cleanupInterval: ReturnType<typeof setInterval> | null = null;

  constructor(cleanupIntervalMs = 60_000) {
    // Periodically clean up expired entries
    this.cleanupInterval = setInterval(() => this.cleanup(), cleanupIntervalMs);
  }

  /**
   * Get a value from the cache.
   * Returns undefined if not found or expired.
   */
  get<T>(key: string): T | undefined {
    const entry = this.store.get(key) as CacheEntry<T> | undefined;

    if (!entry) {
      logger.cache("get", key, false);
      return undefined;
    }

    if (Date.now() > entry.expiresAt) {
      logger.cache("expired", key);
      this.store.delete(key);
      return undefined;
    }

    logger.cache("get", key, true);
    return entry.data;
  }

  /**
   * Set a value in the cache with TTL.
   */
  set<T>(key: string, data: T, options: CacheOptions): void {
    const now = Date.now();
    const entry: CacheEntry<T> = {
      data,
      expiresAt: now + options.ttl,
      createdAt: now,
    };

    this.store.set(key, entry);
    logger.cache("set", key);
  }

  /**
   * Check if a key exists and is not expired.
   */
  has(key: string): boolean {
    const entry = this.store.get(key);
    if (!entry) return false;
    if (Date.now() > entry.expiresAt) {
      this.store.delete(key);
      return false;
    }
    return true;
  }

  /**
   * Delete a specific key from the cache.
   */
  delete(key: string): boolean {
    logger.cache("delete", key);
    return this.store.delete(key);
  }

  /**
   * Clear all entries from the cache.
   */
  clear(): void {
    logger.cache("clear", "all");
    this.store.clear();
  }

  /**
   * Get the remaining TTL for a key in milliseconds.
   * Returns -1 if the key doesn't exist.
   */
  ttl(key: string): number {
    const entry = this.store.get(key);
    if (!entry) return -1;

    const remaining = entry.expiresAt - Date.now();
    return remaining > 0 ? remaining : -1;
  }

  /**
   * Get cache statistics.
   */
  stats(): { size: number; keys: string[] } {
    return {
      size: this.store.size,
      keys: Array.from(this.store.keys()),
    };
  }

  /**
   * Remove expired entries from the cache.
   */
  private cleanup(): void {
    const now = Date.now();
    let cleaned = 0;

    for (const [key, entry] of this.store.entries()) {
      if (now > entry.expiresAt) {
        this.store.delete(key);
        cleaned++;
      }
    }

    if (cleaned > 0) {
      logger.debug(`Cache cleanup: removed ${cleaned} expired entries`);
    }
  }

  /**
   * Stop the cleanup interval (for graceful shutdown).
   */
  destroy(): void {
    if (this.cleanupInterval) {
      clearInterval(this.cleanupInterval);
      this.cleanupInterval = null;
    }
    this.clear();
  }
}

// Cache configuration with TTL values
export const CACHE_CONFIG = {
  listings: { ttl: 30_000 }, // 30 seconds
  quotes: { ttl: 15_000 }, // 15 seconds
  globalMetrics: { ttl: 60_000 }, // 1 minute
  fearGreed: { ttl: 300_000 }, // 5 minutes
  asset: { ttl: 30_000 }, // 30 seconds
};

// Cache keys
export const CACHE_KEYS = {
  listings: (start: number, limit: number) => `listings:${start}:${limit}`,
  asset: (id: number) => `asset:${id}`,
  quotes: (id: number) => `quotes:${id}`,
  globalMetrics: () => "global-metrics",
  fearGreed: () => "fear-greed",
};

// Default cache instance
export const cache = new Cache();
