import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { Cache, CACHE_CONFIG, CACHE_KEYS } from "../../src/services/cache.js";

describe("Cache", () => {
  let cache: Cache;

  beforeEach(() => {
    vi.useFakeTimers();
    cache = new Cache(60_000);
  });

  afterEach(() => {
    cache.destroy();
    vi.useRealTimers();
  });

  describe("set and get", () => {
    it("should store and retrieve a value", () => {
      cache.set("test-key", { foo: "bar" }, { ttl: 1000 });
      const result = cache.get<{ foo: string }>("test-key");
      expect(result).toEqual({ foo: "bar" });
    });

    it("should return undefined for non-existent key", () => {
      const result = cache.get("non-existent");
      expect(result).toBeUndefined();
    });

    it("should return undefined for expired key", () => {
      cache.set("test-key", { foo: "bar" }, { ttl: 1000 });

      // Advance time past TTL
      vi.advanceTimersByTime(1500);

      const result = cache.get("test-key");
      expect(result).toBeUndefined();
    });

    it("should overwrite existing value", () => {
      cache.set("test-key", { value: 1 }, { ttl: 1000 });
      cache.set("test-key", { value: 2 }, { ttl: 1000 });

      const result = cache.get<{ value: number }>("test-key");
      expect(result).toEqual({ value: 2 });
    });
  });

  describe("has", () => {
    it("should return true for existing key", () => {
      cache.set("test-key", "value", { ttl: 1000 });
      expect(cache.has("test-key")).toBe(true);
    });

    it("should return false for non-existent key", () => {
      expect(cache.has("non-existent")).toBe(false);
    });

    it("should return false for expired key", () => {
      cache.set("test-key", "value", { ttl: 1000 });
      vi.advanceTimersByTime(1500);
      expect(cache.has("test-key")).toBe(false);
    });
  });

  describe("delete", () => {
    it("should delete existing key", () => {
      cache.set("test-key", "value", { ttl: 1000 });
      expect(cache.delete("test-key")).toBe(true);
      expect(cache.has("test-key")).toBe(false);
    });

    it("should return false for non-existent key", () => {
      expect(cache.delete("non-existent")).toBe(false);
    });
  });

  describe("clear", () => {
    it("should remove all entries", () => {
      cache.set("key1", "value1", { ttl: 1000 });
      cache.set("key2", "value2", { ttl: 1000 });
      cache.set("key3", "value3", { ttl: 1000 });

      cache.clear();

      expect(cache.stats().size).toBe(0);
    });
  });

  describe("ttl", () => {
    it("should return remaining TTL for existing key", () => {
      cache.set("test-key", "value", { ttl: 1000 });
      vi.advanceTimersByTime(300);

      const remaining = cache.ttl("test-key");
      expect(remaining).toBe(700);
    });

    it("should return -1 for non-existent key", () => {
      expect(cache.ttl("non-existent")).toBe(-1);
    });

    it("should return -1 for expired key", () => {
      cache.set("test-key", "value", { ttl: 1000 });
      vi.advanceTimersByTime(1500);
      expect(cache.ttl("test-key")).toBe(-1);
    });
  });

  describe("stats", () => {
    it("should return correct statistics", () => {
      cache.set("key1", "value1", { ttl: 1000 });
      cache.set("key2", "value2", { ttl: 1000 });

      const stats = cache.stats();
      expect(stats.size).toBe(2);
      expect(stats.keys).toContain("key1");
      expect(stats.keys).toContain("key2");
    });
  });
});

describe("CACHE_KEYS", () => {
  it("should generate correct listings key", () => {
    expect(CACHE_KEYS.listings(1, 100)).toBe("listings:1:100");
    expect(CACHE_KEYS.listings(101, 50)).toBe("listings:101:50");
  });

  it("should generate correct asset key", () => {
    expect(CACHE_KEYS.asset(1)).toBe("asset:1");
    expect(CACHE_KEYS.asset(12345)).toBe("asset:12345");
  });

  it("should generate correct quotes key", () => {
    expect(CACHE_KEYS.quotes(1)).toBe("quotes:1");
  });

  it("should generate correct global metrics key", () => {
    expect(CACHE_KEYS.globalMetrics()).toBe("global-metrics");
  });

  it("should generate correct fear greed key", () => {
    expect(CACHE_KEYS.fearGreed()).toBe("fear-greed");
  });
});

describe("CACHE_CONFIG", () => {
  it("should have correct TTL values", () => {
    expect(CACHE_CONFIG.listings.ttl).toBe(30_000);
    expect(CACHE_CONFIG.quotes.ttl).toBe(15_000);
    expect(CACHE_CONFIG.globalMetrics.ttl).toBe(60_000);
    expect(CACHE_CONFIG.fearGreed.ttl).toBe(300_000);
  });
});
