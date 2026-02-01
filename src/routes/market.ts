import { Hono } from "hono";
import { cache, CACHE_KEYS, CACHE_CONFIG } from "../services/cache.js";
import {
  cmcClient,
  cmcToGlobalMetrics,
  cmcToFearGreed,
} from "../services/coinmarketcap.js";
import type { GlobalMetrics, FearGreedData } from "../types/index.js";

const market = new Hono();

/**
 * GET /api/market/global
 * Get global cryptocurrency market metrics
 */
market.get("/global", async (c) => {
  // Check cache first
  const cacheKey = CACHE_KEYS.globalMetrics();
  const cached = cache.get<GlobalMetrics>(cacheKey);

  if (cached) {
    return c.json({
      data: cached,
      meta: { cached: true },
    });
  }

  // Fetch from API
  const response = await cmcClient.getGlobalMetrics();
  const metrics = cmcToGlobalMetrics(response);

  // Cache the result
  cache.set(cacheKey, metrics, CACHE_CONFIG.globalMetrics);

  return c.json({
    data: metrics,
    meta: { cached: false },
  });
});

/**
 * GET /api/market/fear-greed
 * Get the Fear & Greed Index
 */
market.get("/fear-greed", async (c) => {
  // Check cache first
  const cacheKey = CACHE_KEYS.fearGreed();
  const cached = cache.get<FearGreedData>(cacheKey);

  if (cached) {
    return c.json({
      data: cached,
      meta: { cached: true },
    });
  }

  // Fetch from API
  const response = await cmcClient.getFearAndGreed();
  const fearGreed = cmcToFearGreed(response);

  // Cache the result
  cache.set(cacheKey, fearGreed, CACHE_CONFIG.fearGreed);

  return c.json({
    data: fearGreed,
    meta: { cached: false },
  });
});

export { market };
