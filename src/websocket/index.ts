import { WebSocketServer, type WebSocket } from "ws";
import type { Server } from "http";
import { logger } from "../utils/logger.js";
import { scheduler } from "../services/scheduler.js";
import {
  handleMessage,
  handleConnection,
  handleDisconnect,
  broadcastPriceUpdate,
  broadcastMarketUpdate,
} from "./handlers.js";
import type { Asset, GlobalMetrics } from "../types/index.js";

let wss: WebSocketServer | null = null;

/**
 * Initialize the WebSocket server.
 */
export function initWebSocket(server: Server): WebSocketServer {
  wss = new WebSocketServer({ server, path: "/ws" });

  wss.on("connection", (client: WebSocket) => {
    handleConnection(client);

    client.on("message", (data) => {
      handleMessage(client, data.toString());
    });

    client.on("close", () => {
      handleDisconnect(client);
    });

    client.on("error", (error) => {
      logger.error("WebSocket client error", error);
    });
  });

  // Subscribe to scheduler updates
  scheduler.onUpdate(({ type, data }) => {
    switch (type) {
      case "listings": {
        // Broadcast price updates for each asset
        const assets = data as Asset[];
        for (const asset of assets) {
          broadcastPriceUpdate({
            id: asset.id,
            symbol: asset.symbol.toLowerCase(),
            price: asset.price,
            change24h: asset.change24h,
            volume24h: asset.volume24h,
            timestamp: new Date().toISOString(),
          });
        }
        break;
      }

      case "globalMetrics": {
        // Broadcast market update
        const metrics = data as GlobalMetrics;
        broadcastMarketUpdate({
          totalMarketCap: metrics.totalMarketCap,
          totalVolume24h: metrics.totalVolume24h,
          btcDominance: metrics.btcDominance,
          timestamp: new Date().toISOString(),
        });
        break;
      }
    }
  });

  logger.info("WebSocket server initialized at /ws");

  return wss;
}

/**
 * Get the WebSocket server instance.
 */
export function getWebSocketServer(): WebSocketServer | null {
  return wss;
}

/**
 * Close the WebSocket server.
 */
export function closeWebSocket(): Promise<void> {
  return new Promise((resolve, reject) => {
    if (!wss) {
      resolve();
      return;
    }

    wss.close((err) => {
      if (err) {
        reject(err);
      } else {
        wss = null;
        resolve();
      }
    });
  });
}
