import type { WebSocket } from "ws";
import { logger } from "../utils/logger.js";

/**
 * Manages WebSocket subscription rooms.
 * Clients can subscribe to specific assets or market updates.
 */
export class RoomManager {
  // Map of asset symbol -> Set of subscribed clients
  private assetRooms = new Map<string, Set<WebSocket>>();

  // Set of clients subscribed to market updates
  private marketRoom = new Set<WebSocket>();

  // Map of client -> Set of subscribed assets
  private clientSubscriptions = new Map<WebSocket, Set<string>>();

  /**
   * Subscribe a client to specific assets.
   */
  subscribe(client: WebSocket, assets: string[]): string[] {
    const normalizedAssets = assets.map((a) => a.toLowerCase());
    const subscribed: string[] = [];

    // Get or create client subscription set
    let clientSubs = this.clientSubscriptions.get(client);
    if (!clientSubs) {
      clientSubs = new Set();
      this.clientSubscriptions.set(client, clientSubs);
    }

    for (const asset of normalizedAssets) {
      // Get or create room for asset
      let room = this.assetRooms.get(asset);
      if (!room) {
        room = new Set();
        this.assetRooms.set(asset, room);
      }

      // Add client to room
      room.add(client);
      clientSubs.add(asset);
      subscribed.push(asset);

      logger.ws("subscribe", this.getClientId(client), { asset });
    }

    // Also subscribe to market updates
    this.marketRoom.add(client);

    return subscribed;
  }

  /**
   * Unsubscribe a client from specific assets or all assets.
   */
  unsubscribe(client: WebSocket, assets?: string[]): string[] {
    const clientSubs = this.clientSubscriptions.get(client);
    if (!clientSubs) return [];

    const unsubscribed: string[] = [];
    const assetsToRemove = assets
      ? assets.map((a) => a.toLowerCase())
      : Array.from(clientSubs);

    for (const asset of assetsToRemove) {
      const room = this.assetRooms.get(asset);
      if (room) {
        room.delete(client);
        // Clean up empty rooms
        if (room.size === 0) {
          this.assetRooms.delete(asset);
        }
      }
      clientSubs.delete(asset);
      unsubscribed.push(asset);

      logger.ws("unsubscribe", this.getClientId(client), { asset });
    }

    // If no more subscriptions, remove from market room
    if (clientSubs.size === 0) {
      this.marketRoom.delete(client);
    }

    return unsubscribed;
  }

  /**
   * Remove a client from all rooms (on disconnect).
   */
  removeClient(client: WebSocket): void {
    const clientSubs = this.clientSubscriptions.get(client);
    if (clientSubs) {
      for (const asset of clientSubs) {
        const room = this.assetRooms.get(asset);
        if (room) {
          room.delete(client);
          if (room.size === 0) {
            this.assetRooms.delete(asset);
          }
        }
      }
      this.clientSubscriptions.delete(client);
    }

    this.marketRoom.delete(client);
    logger.ws("disconnect", this.getClientId(client));
  }

  /**
   * Get all clients subscribed to a specific asset.
   */
  getAssetSubscribers(asset: string): Set<WebSocket> {
    return this.assetRooms.get(asset.toLowerCase()) || new Set();
  }

  /**
   * Get all clients subscribed to market updates.
   */
  getMarketSubscribers(): Set<WebSocket> {
    return this.marketRoom;
  }

  /**
   * Get subscriptions for a client.
   */
  getClientSubscriptions(client: WebSocket): string[] {
    const subs = this.clientSubscriptions.get(client);
    return subs ? Array.from(subs) : [];
  }

  /**
   * Get statistics about rooms.
   */
  stats(): {
    totalClients: number;
    totalRooms: number;
    rooms: Record<string, number>;
  } {
    const rooms: Record<string, number> = {};
    for (const [asset, clients] of this.assetRooms) {
      rooms[asset] = clients.size;
    }

    return {
      totalClients: this.clientSubscriptions.size,
      totalRooms: this.assetRooms.size,
      rooms,
    };
  }

  /**
   * Generate a unique client ID for logging.
   */
  private clientIdCounter = 0;
  private clientIds = new WeakMap<WebSocket, string>();

  private getClientId(client: WebSocket): string {
    let id = this.clientIds.get(client);
    if (!id) {
      id = `client-${++this.clientIdCounter}`;
      this.clientIds.set(client, id);
    }
    return id;
  }
}

// Default room manager instance
export const roomManager = new RoomManager();
