import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { app } from "../../src/app.js";
import { cache, CACHE_KEYS, CACHE_CONFIG } from "../../src/services/cache.js";
import type { GlobalMetrics, FearGreedData } from "../../src/types/index.js";

const mockGlobalMetrics: GlobalMetrics = {
  totalMarketCap: 2000000000000,
  totalVolume24h: 100000000000,
  btcDominance: 45,
  ethDominance: 18,
  activeCryptocurrencies: 10000,
  activeExchanges: 500,
  marketCapChange24h: 2.5,
  volumeChange24h: 5.2,
  lastUpdated: "2024-01-01T00:00:00.000Z",
};

const mockFearGreed: FearGreedData = {
  value: 75,
  classification: "Greed",
  timestamp: "2024-01-01T00:00:00.000Z",
};

describe("Market Routes", () => {
  beforeEach(() => {
    cache.clear();
  });

  afterEach(() => {
    cache.clear();
  });

  describe("GET /api/market/global", () => {
    it("should return cached global metrics if available", async () => {
      cache.set(CACHE_KEYS.globalMetrics(), mockGlobalMetrics, CACHE_CONFIG.globalMetrics);

      const res = await app.request("/api/market/global");
      expect(res.status).toBe(200);

      const body = await res.json();
      expect(body.data.totalMarketCap).toBe(2000000000000);
      expect(body.data.btcDominance).toBe(45);
      expect(body.data.ethDominance).toBe(18);
      expect(body.meta.cached).toBe(true);
    });

    it("should include all expected fields", async () => {
      cache.set(CACHE_KEYS.globalMetrics(), mockGlobalMetrics, CACHE_CONFIG.globalMetrics);

      const res = await app.request("/api/market/global");
      const body = await res.json();

      expect(body.data).toHaveProperty("totalMarketCap");
      expect(body.data).toHaveProperty("totalVolume24h");
      expect(body.data).toHaveProperty("btcDominance");
      expect(body.data).toHaveProperty("ethDominance");
      expect(body.data).toHaveProperty("activeCryptocurrencies");
      expect(body.data).toHaveProperty("activeExchanges");
      expect(body.data).toHaveProperty("marketCapChange24h");
      expect(body.data).toHaveProperty("volumeChange24h");
      expect(body.data).toHaveProperty("lastUpdated");
    });
  });

  describe("GET /api/market/fear-greed", () => {
    it("should return cached fear & greed if available", async () => {
      cache.set(CACHE_KEYS.fearGreed(), mockFearGreed, CACHE_CONFIG.fearGreed);

      const res = await app.request("/api/market/fear-greed");
      expect(res.status).toBe(200);

      const body = await res.json();
      expect(body.data.value).toBe(75);
      expect(body.data.classification).toBe("Greed");
      expect(body.meta.cached).toBe(true);
    });

    it("should include all expected fields", async () => {
      cache.set(CACHE_KEYS.fearGreed(), mockFearGreed, CACHE_CONFIG.fearGreed);

      const res = await app.request("/api/market/fear-greed");
      const body = await res.json();

      expect(body.data).toHaveProperty("value");
      expect(body.data).toHaveProperty("classification");
      expect(body.data).toHaveProperty("timestamp");
    });

    it("should have value between 0 and 100", async () => {
      cache.set(CACHE_KEYS.fearGreed(), mockFearGreed, CACHE_CONFIG.fearGreed);

      const res = await app.request("/api/market/fear-greed");
      const body = await res.json();

      expect(body.data.value).toBeGreaterThanOrEqual(0);
      expect(body.data.value).toBeLessThanOrEqual(100);
    });
  });
});
