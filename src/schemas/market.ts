import { z } from "zod";

// Global metrics schema
export const GlobalMetricsSchema = z.object({
  totalMarketCap: z.number(),
  totalVolume24h: z.number(),
  btcDominance: z.number(),
  ethDominance: z.number(),
  activeCryptocurrencies: z.number(),
  activeExchanges: z.number(),
  marketCapChange24h: z.number(),
  volumeChange24h: z.number(),
  lastUpdated: z.string(),
});

export type GlobalMetricsSchema = z.infer<typeof GlobalMetricsSchema>;

// Global metrics response
export const GlobalMetricsResponseSchema = z.object({
  data: GlobalMetricsSchema,
  meta: z.object({
    cached: z.boolean(),
  }),
});

export type GlobalMetricsResponse = z.infer<typeof GlobalMetricsResponseSchema>;

// Fear & Greed schema
export const FearGreedSchema = z.object({
  value: z.number().min(0).max(100),
  classification: z.string(),
  timestamp: z.string(),
});

export type FearGreedSchema = z.infer<typeof FearGreedSchema>;

// Fear & Greed response
export const FearGreedResponseSchema = z.object({
  data: FearGreedSchema,
  meta: z.object({
    cached: z.boolean(),
  }),
});

export type FearGreedResponse = z.infer<typeof FearGreedResponseSchema>;
