import { Hono } from "hono";
import { zValidator } from "@hono/zod-validator";
import { cache, CACHE_KEYS, CACHE_CONFIG } from "../services/cache.js";
import { cmcClient, cmcToAsset } from "../services/coinmarketcap.js";
import { NotFoundError } from "../utils/errors.js";
import {
  ListingsQuerySchema,
  IdParamSchema,
  SearchQuerySchema,
} from "../schemas/crypto.js";
import type { Asset } from "../types/index.js";

const crypto = new Hono();

/**
 * GET /api/crypto/listings
 * Get paginated cryptocurrency listings
 */
crypto.get("/listings", zValidator("query", ListingsQuerySchema), async (c) => {
  const query = c.req.valid("query");
  const { start, limit, sort, sort_dir } = query;

  // Check cache first
  const cacheKey = CACHE_KEYS.listings(start, limit);
  const cached = cache.get<Asset[]>(cacheKey);

  if (cached) {
    return c.json({
      data: cached,
      meta: {
        total: cached.length,
        start,
        limit,
        cached: true,
      },
    });
  }

  // Fetch from API
  const response = await cmcClient.getListings({
    start,
    limit,
    sort,
    sort_dir,
  });

  const assets = response.data.map(cmcToAsset);

  // Cache the results
  cache.set(cacheKey, assets, CACHE_CONFIG.listings);

  // Also cache individual assets
  for (const asset of assets) {
    cache.set(CACHE_KEYS.asset(asset.id), asset, CACHE_CONFIG.asset);
  }

  return c.json({
    data: assets,
    meta: {
      total: assets.length,
      start,
      limit,
      cached: false,
    },
  });
});

/**
 * GET /api/crypto/search
 * Search cryptocurrencies by name or symbol
 */
crypto.get("/search", zValidator("query", SearchQuerySchema), async (c) => {
  const { q, limit } = c.req.valid("query");
  const queryLower = q.toLowerCase();

  // First try to get from cache
  const cachedListings = cache.get<Asset[]>(CACHE_KEYS.listings(1, 100));

  let searchPool: Asset[];

  if (cachedListings) {
    searchPool = cachedListings;
  } else {
    // Fetch a larger set to search from
    const response = await cmcClient.getListings({ limit: 500 });
    searchPool = response.data.map(cmcToAsset);

    // Cache individual assets from search results
    for (const asset of searchPool) {
      cache.set(CACHE_KEYS.asset(asset.id), asset, CACHE_CONFIG.asset);
    }
  }

  // Filter by query
  const results = searchPool
    .filter(
      (asset) =>
        asset.name.toLowerCase().includes(queryLower) ||
        asset.symbol.toLowerCase().includes(queryLower)
    )
    .slice(0, limit);

  return c.json({
    data: results,
    meta: {
      query: q,
      total: results.length,
    },
  });
});

/**
 * GET /api/crypto/:id
 * Get a single cryptocurrency by ID
 */
crypto.get("/:id", zValidator("param", IdParamSchema), async (c) => {
  const { id } = c.req.valid("param");

  // Check cache first
  const cacheKey = CACHE_KEYS.asset(id);
  const cached = cache.get<Asset>(cacheKey);

  if (cached) {
    return c.json({
      data: cached,
      meta: { cached: true },
    });
  }

  // Fetch from API
  const response = await cmcClient.getQuotes([id]);

  if (!response.data || response.data.length === 0) {
    throw new NotFoundError(`Cryptocurrency with ID ${id}`);
  }

  const asset = cmcToAsset(response.data[0]);

  // Cache the result
  cache.set(cacheKey, asset, CACHE_CONFIG.asset);

  return c.json({
    data: asset,
    meta: { cached: false },
  });
});

/**
 * GET /api/crypto/:id/quotes
 * Get latest quotes for a cryptocurrency
 */
crypto.get("/:id/quotes", zValidator("param", IdParamSchema), async (c) => {
  const { id } = c.req.valid("param");

  // Check cache first
  const cacheKey = CACHE_KEYS.quotes(id);
  const cached = cache.get<Asset>(cacheKey);

  if (cached) {
    return c.json({
      data: cached,
      meta: { cached: true },
    });
  }

  // Fetch from API
  const response = await cmcClient.getQuotes([id]);

  if (!response.data || response.data.length === 0) {
    throw new NotFoundError(`Cryptocurrency with ID ${id}`);
  }

  const asset = cmcToAsset(response.data[0]);

  // Cache with shorter TTL for quotes
  cache.set(cacheKey, asset, CACHE_CONFIG.quotes);

  return c.json({
    data: asset,
    meta: { cached: false },
  });
});

export { crypto };
