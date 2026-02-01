import { z } from "zod";

// WebSocket message types
export const WSMessageTypeSchema = z.enum([
  "subscribe",
  "unsubscribe",
  "price_update",
  "market_update",
  "error",
  "subscribed",
  "unsubscribed",
]);

export type WSMessageType = z.infer<typeof WSMessageTypeSchema>;

// Subscribe message
export const SubscribeMessageSchema = z.object({
  type: z.literal("subscribe"),
  assets: z.array(z.string()).min(1).max(50),
});

export type SubscribeMessage = z.infer<typeof SubscribeMessageSchema>;

// Unsubscribe message
export const UnsubscribeMessageSchema = z.object({
  type: z.literal("unsubscribe"),
  assets: z.array(z.string()).optional(),
});

export type UnsubscribeMessage = z.infer<typeof UnsubscribeMessageSchema>;

// Client message (union of all client-sent message types)
export const ClientMessageSchema = z.discriminatedUnion("type", [
  SubscribeMessageSchema,
  UnsubscribeMessageSchema,
]);

export type ClientMessage = z.infer<typeof ClientMessageSchema>;

// Price update (server -> client)
export const PriceUpdateSchema = z.object({
  id: z.number(),
  symbol: z.string(),
  price: z.number(),
  change24h: z.number(),
  volume24h: z.number(),
  timestamp: z.string(),
});

export type PriceUpdate = z.infer<typeof PriceUpdateSchema>;

// Market update (server -> client)
export const MarketUpdateSchema = z.object({
  totalMarketCap: z.number(),
  totalVolume24h: z.number(),
  btcDominance: z.number(),
  timestamp: z.string(),
});

export type MarketUpdate = z.infer<typeof MarketUpdateSchema>;

// Server messages
export const PriceUpdateMessageSchema = z.object({
  type: z.literal("price_update"),
  data: PriceUpdateSchema,
});

export const MarketUpdateMessageSchema = z.object({
  type: z.literal("market_update"),
  data: MarketUpdateSchema,
});

export const SubscribedMessageSchema = z.object({
  type: z.literal("subscribed"),
  assets: z.array(z.string()),
});

export const UnsubscribedMessageSchema = z.object({
  type: z.literal("unsubscribed"),
  assets: z.array(z.string()),
});

export const ErrorMessageSchema = z.object({
  type: z.literal("error"),
  error: z.string(),
});

export const ServerMessageSchema = z.discriminatedUnion("type", [
  PriceUpdateMessageSchema,
  MarketUpdateMessageSchema,
  SubscribedMessageSchema,
  UnsubscribedMessageSchema,
  ErrorMessageSchema,
]);

export type ServerMessage = z.infer<typeof ServerMessageSchema>;
