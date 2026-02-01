import { z } from "zod";

// Asset schema
export const AssetSchema = z.object({
  id: z.number(),
  rank: z.number(),
  name: z.string(),
  symbol: z.string(),
  image: z.string().url(),
  price: z.number(),
  change1h: z.number(),
  change24h: z.number(),
  change7d: z.number(),
  marketCap: z.number(),
  volume24h: z.number(),
  circulatingSupply: z.number(),
  maxSupply: z.number().optional(),
  sparkline: z.array(z.number()),
});

export type AssetSchema = z.infer<typeof AssetSchema>;

// Listings query parameters
export const ListingsQuerySchema = z.object({
  start: z.coerce.number().int().min(1).default(1),
  limit: z.coerce.number().int().min(1).max(500).default(100),
  sort: z
    .enum([
      "market_cap",
      "name",
      "symbol",
      "date_added",
      "price",
      "circulating_supply",
      "total_supply",
      "max_supply",
      "num_market_pairs",
      "volume_24h",
      "percent_change_1h",
      "percent_change_24h",
      "percent_change_7d",
    ])
    .default("market_cap"),
  sort_dir: z.enum(["asc", "desc"]).default("desc"),
});

export type ListingsQuery = z.infer<typeof ListingsQuerySchema>;

// Listings response
export const ListingsResponseSchema = z.object({
  data: z.array(AssetSchema),
  meta: z.object({
    total: z.number(),
    start: z.number(),
    limit: z.number(),
    cached: z.boolean(),
  }),
});

export type ListingsResponse = z.infer<typeof ListingsResponseSchema>;

// Single asset response
export const AssetResponseSchema = z.object({
  data: AssetSchema,
  meta: z.object({
    cached: z.boolean(),
  }),
});

export type AssetResponse = z.infer<typeof AssetResponseSchema>;

// Quotes response (same as asset but potentially multiple)
export const QuotesResponseSchema = z.object({
  data: z.array(AssetSchema),
  meta: z.object({
    cached: z.boolean(),
  }),
});

export type QuotesResponse = z.infer<typeof QuotesResponseSchema>;

// Search query parameters
export const SearchQuerySchema = z.object({
  q: z.string().min(1).max(100),
  limit: z.coerce.number().int().min(1).max(50).default(20),
});

export type SearchQuery = z.infer<typeof SearchQuerySchema>;

// Search response
export const SearchResponseSchema = z.object({
  data: z.array(AssetSchema),
  meta: z.object({
    query: z.string(),
    total: z.number(),
  }),
});

export type SearchResponse = z.infer<typeof SearchResponseSchema>;

// ID parameter
export const IdParamSchema = z.object({
  id: z.coerce.number().int().positive(),
});

export type IdParam = z.infer<typeof IdParamSchema>;
