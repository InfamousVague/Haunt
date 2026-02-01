import type { WebSocket } from "ws";
import { logger } from "../utils/logger.js";
import { roomManager } from "./rooms.js";
import { ClientMessageSchema } from "../schemas/websocket.js";
import type { ServerMessage, PriceUpdate, MarketUpdate } from "../schemas/websocket.js";

/**
 * Send a message to a WebSocket client.
 */
function send(client: WebSocket, message: ServerMessage): void {
  if (client.readyState === client.OPEN) {
    client.send(JSON.stringify(message));
  }
}

/**
 * Send an error message to a client.
 */
function sendError(client: WebSocket, error: string): void {
  send(client, { type: "error", error });
}

/**
 * Handle incoming WebSocket messages.
 */
export function handleMessage(client: WebSocket, data: string): void {
  let message: unknown;

  try {
    message = JSON.parse(data);
  } catch {
    sendError(client, "Invalid JSON");
    return;
  }

  // Validate message against schema
  const result = ClientMessageSchema.safeParse(message);

  if (!result.success) {
    sendError(client, `Invalid message: ${result.error.message}`);
    return;
  }

  const validMessage = result.data;

  switch (validMessage.type) {
    case "subscribe": {
      const subscribed = roomManager.subscribe(client, validMessage.assets);
      send(client, { type: "subscribed", assets: subscribed });
      break;
    }

    case "unsubscribe": {
      const unsubscribed = roomManager.unsubscribe(client, validMessage.assets);
      send(client, { type: "unsubscribed", assets: unsubscribed });
      break;
    }
  }
}

/**
 * Handle client connection.
 */
export function handleConnection(client: WebSocket): void {
  logger.info("WebSocket client connected");
}

/**
 * Handle client disconnection.
 */
export function handleDisconnect(client: WebSocket): void {
  roomManager.removeClient(client);
  logger.info("WebSocket client disconnected");
}

/**
 * Broadcast a price update to subscribed clients.
 */
export function broadcastPriceUpdate(update: PriceUpdate): void {
  const subscribers = roomManager.getAssetSubscribers(update.symbol);

  for (const client of subscribers) {
    send(client, { type: "price_update", data: update });
  }

  if (subscribers.size > 0) {
    logger.debug(`Price update broadcast to ${subscribers.size} clients`, {
      symbol: update.symbol,
    });
  }
}

/**
 * Broadcast a market update to all subscribed clients.
 */
export function broadcastMarketUpdate(update: MarketUpdate): void {
  const subscribers = roomManager.getMarketSubscribers();

  for (const client of subscribers) {
    send(client, { type: "market_update", data: update });
  }

  if (subscribers.size > 0) {
    logger.debug(`Market update broadcast to ${subscribers.size} clients`);
  }
}
