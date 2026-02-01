import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { app } from "../../src/app.js";
import { cache } from "../../src/services/cache.js";
import { CACHE_KEYS, CACHE_CONFIG } from "../../src/services/cache.js";
import type { Asset } from "../../src/types/index.js";

// Mock asset for testing
const mockAsset: Asset = {
  id: 1,
  rank: 1,
  name: "Bitcoin",
  symbol: "BTC",
  image: "https://s2.coinmarketcap.com/static/img/coins/64x64/1.png",
  price: 50000,
  change1h: 0.5,
  change24h: 2.5,
  change7d: 10,
  marketCap: 950000000000,
  volume24h: 25000000000,
  circulatingSupply: 19000000,
  maxSupply: 21000000,
  sparkline: [45000, 46000, 47000, 48000, 49000, 49500, 50000],
};

const mockAsset2: Asset = {
  id: 1027,
  rank: 2,
  name: "Ethereum",
  symbol: "ETH",
  image: "https://s2.coinmarketcap.com/static/img/coins/64x64/1027.png",
  price: 3000,
  change1h: 0.3,
  change24h: 1.5,
  change7d: 8,
  marketCap: 350000000000,
  volume24h: 15000000000,
  circulatingSupply: 120000000,
  sparkline: [2700, 2750, 2800, 2850, 2900, 2950, 3000],
};

describe("Crypto Routes", () => {
  beforeEach(() => {
    cache.clear();
  });

  afterEach(() => {
    cache.clear();
  });

  describe("GET /api/crypto/listings", () => {
    it("should return cached listings if available", async () => {
      // Pre-populate cache
      cache.set(CACHE_KEYS.listings(1, 100), [mockAsset, mockAsset2], CACHE_CONFIG.listings);

      const res = await app.request("/api/crypto/listings");
      expect(res.status).toBe(200);

      const body = await res.json();
      expect(body.data).toHaveLength(2);
      expect(body.meta.cached).toBe(true);
      expect(body.data[0].symbol).toBe("BTC");
      expect(body.data[1].symbol).toBe("ETH");
    });

    it("should respect limit query parameter", async () => {
      cache.set(CACHE_KEYS.listings(1, 1), [mockAsset], CACHE_CONFIG.listings);

      const res = await app.request("/api/crypto/listings?limit=1");
      expect(res.status).toBe(200);

      const body = await res.json();
      expect(body.meta.limit).toBe(1);
    });

    it("should respect start query parameter", async () => {
      cache.set(CACHE_KEYS.listings(101, 100), [mockAsset2], CACHE_CONFIG.listings);

      const res = await app.request("/api/crypto/listings?start=101");
      expect(res.status).toBe(200);

      const body = await res.json();
      expect(body.meta.start).toBe(101);
    });

    it("should validate limit parameter", async () => {
      const res = await app.request("/api/crypto/listings?limit=1000");
      expect(res.status).toBe(400);
    });

    it("should validate start parameter", async () => {
      const res = await app.request("/api/crypto/listings?start=0");
      expect(res.status).toBe(400);
    });
  });

  describe("GET /api/crypto/search", () => {
    beforeEach(() => {
      cache.set(CACHE_KEYS.listings(1, 100), [mockAsset, mockAsset2], CACHE_CONFIG.listings);
    });

    it("should search by name", async () => {
      const res = await app.request("/api/crypto/search?q=Bitcoin");
      expect(res.status).toBe(200);

      const body = await res.json();
      expect(body.data).toHaveLength(1);
      expect(body.data[0].name).toBe("Bitcoin");
      expect(body.meta.query).toBe("Bitcoin");
    });

    it("should search by symbol", async () => {
      const res = await app.request("/api/crypto/search?q=ETH");
      expect(res.status).toBe(200);

      const body = await res.json();
      expect(body.data).toHaveLength(1);
      expect(body.data[0].symbol).toBe("ETH");
    });

    it("should be case insensitive", async () => {
      const res = await app.request("/api/crypto/search?q=bitcoin");
      expect(res.status).toBe(200);

      const body = await res.json();
      expect(body.data).toHaveLength(1);
    });

    it("should require query parameter", async () => {
      const res = await app.request("/api/crypto/search");
      expect(res.status).toBe(400);
    });
  });

  describe("GET /api/crypto/:id", () => {
    it("should return cached asset if available", async () => {
      cache.set(CACHE_KEYS.asset(1), mockAsset, CACHE_CONFIG.asset);

      const res = await app.request("/api/crypto/1");
      expect(res.status).toBe(200);

      const body = await res.json();
      expect(body.data.id).toBe(1);
      expect(body.data.symbol).toBe("BTC");
      expect(body.meta.cached).toBe(true);
    });

    it("should validate id parameter", async () => {
      const res = await app.request("/api/crypto/invalid");
      expect(res.status).toBe(400);
    });

    it("should reject negative id", async () => {
      const res = await app.request("/api/crypto/-1");
      expect(res.status).toBe(400);
    });
  });

  describe("GET /api/crypto/:id/quotes", () => {
    it("should return cached quotes if available", async () => {
      cache.set(CACHE_KEYS.quotes(1), mockAsset, CACHE_CONFIG.quotes);

      const res = await app.request("/api/crypto/1/quotes");
      expect(res.status).toBe(200);

      const body = await res.json();
      expect(body.data.id).toBe(1);
      expect(body.data.price).toBe(50000);
      expect(body.meta.cached).toBe(true);
    });
  });
});
