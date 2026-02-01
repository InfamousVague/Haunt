import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { RoomManager } from "../../src/websocket/rooms.js";
import type { WebSocket } from "ws";

// Mock WebSocket
function createMockClient(): WebSocket {
  return {
    readyState: 1, // OPEN
    OPEN: 1,
    send: vi.fn(),
    on: vi.fn(),
    close: vi.fn(),
  } as unknown as WebSocket;
}

describe("RoomManager", () => {
  let roomManager: RoomManager;

  beforeEach(() => {
    roomManager = new RoomManager();
  });

  describe("subscribe", () => {
    it("should subscribe client to assets", () => {
      const client = createMockClient();
      const subscribed = roomManager.subscribe(client, ["btc", "eth"]);

      expect(subscribed).toContain("btc");
      expect(subscribed).toContain("eth");
    });

    it("should normalize asset names to lowercase", () => {
      const client = createMockClient();
      roomManager.subscribe(client, ["BTC", "ETH"]);

      const subs = roomManager.getClientSubscriptions(client);
      expect(subs).toContain("btc");
      expect(subs).toContain("eth");
    });

    it("should add client to asset rooms", () => {
      const client = createMockClient();
      roomManager.subscribe(client, ["btc"]);

      const subscribers = roomManager.getAssetSubscribers("btc");
      expect(subscribers.has(client)).toBe(true);
    });

    it("should add client to market room", () => {
      const client = createMockClient();
      roomManager.subscribe(client, ["btc"]);

      const marketSubscribers = roomManager.getMarketSubscribers();
      expect(marketSubscribers.has(client)).toBe(true);
    });

    it("should handle multiple clients subscribing to same asset", () => {
      const client1 = createMockClient();
      const client2 = createMockClient();

      roomManager.subscribe(client1, ["btc"]);
      roomManager.subscribe(client2, ["btc"]);

      const subscribers = roomManager.getAssetSubscribers("btc");
      expect(subscribers.size).toBe(2);
    });
  });

  describe("unsubscribe", () => {
    it("should unsubscribe client from specific assets", () => {
      const client = createMockClient();
      roomManager.subscribe(client, ["btc", "eth", "sol"]);

      const unsubscribed = roomManager.unsubscribe(client, ["btc"]);

      expect(unsubscribed).toContain("btc");
      expect(roomManager.getAssetSubscribers("btc").has(client)).toBe(false);
      expect(roomManager.getAssetSubscribers("eth").has(client)).toBe(true);
    });

    it("should unsubscribe client from all assets when no assets specified", () => {
      const client = createMockClient();
      roomManager.subscribe(client, ["btc", "eth"]);

      roomManager.unsubscribe(client);

      expect(roomManager.getClientSubscriptions(client)).toHaveLength(0);
    });

    it("should remove empty rooms", () => {
      const client = createMockClient();
      roomManager.subscribe(client, ["btc"]);
      roomManager.unsubscribe(client, ["btc"]);

      const stats = roomManager.stats();
      expect(stats.rooms["btc"]).toBeUndefined();
    });

    it("should remove client from market room when no subscriptions remain", () => {
      const client = createMockClient();
      roomManager.subscribe(client, ["btc"]);
      roomManager.unsubscribe(client);

      expect(roomManager.getMarketSubscribers().has(client)).toBe(false);
    });
  });

  describe("removeClient", () => {
    it("should remove client from all rooms", () => {
      const client = createMockClient();
      roomManager.subscribe(client, ["btc", "eth"]);

      roomManager.removeClient(client);

      expect(roomManager.getAssetSubscribers("btc").has(client)).toBe(false);
      expect(roomManager.getAssetSubscribers("eth").has(client)).toBe(false);
      expect(roomManager.getMarketSubscribers().has(client)).toBe(false);
    });

    it("should clean up empty rooms", () => {
      const client = createMockClient();
      roomManager.subscribe(client, ["btc"]);
      roomManager.removeClient(client);

      const stats = roomManager.stats();
      expect(stats.totalRooms).toBe(0);
    });
  });

  describe("getClientSubscriptions", () => {
    it("should return subscribed assets for client", () => {
      const client = createMockClient();
      roomManager.subscribe(client, ["btc", "eth"]);

      const subs = roomManager.getClientSubscriptions(client);
      expect(subs).toContain("btc");
      expect(subs).toContain("eth");
    });

    it("should return empty array for unknown client", () => {
      const client = createMockClient();
      const subs = roomManager.getClientSubscriptions(client);
      expect(subs).toHaveLength(0);
    });
  });

  describe("stats", () => {
    it("should return correct statistics", () => {
      const client1 = createMockClient();
      const client2 = createMockClient();

      roomManager.subscribe(client1, ["btc", "eth"]);
      roomManager.subscribe(client2, ["btc", "sol"]);

      const stats = roomManager.stats();
      expect(stats.totalClients).toBe(2);
      expect(stats.totalRooms).toBe(3); // btc, eth, sol
      expect(stats.rooms["btc"]).toBe(2);
      expect(stats.rooms["eth"]).toBe(1);
      expect(stats.rooms["sol"]).toBe(1);
    });
  });
});
