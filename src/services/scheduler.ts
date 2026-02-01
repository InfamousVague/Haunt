import { logger } from "../utils/logger.js";
import { cache, CACHE_CONFIG, CACHE_KEYS } from "./cache.js";
import {
  cmcClient,
  cmcToAsset,
  cmcToGlobalMetrics,
  cmcToFearGreed,
} from "./coinmarketcap.js";
import type { Asset, GlobalMetrics, FearGreedData } from "../types/index.js";

type SchedulerCallback = (data: {
  type: "listings" | "globalMetrics" | "fearGreed";
  data: Asset[] | GlobalMetrics | FearGreedData;
}) => void;

/**
 * Scheduler that polls CoinMarketCap API at intervals matching cache TTL.
 * Pushes updates via callback when cache is refreshed.
 */
export class Scheduler {
  private intervals: ReturnType<typeof setInterval>[] = [];
  private callbacks: Set<SchedulerCallback> = new Set();
  private isRunning = false;

  /**
   * Register a callback to receive updates when data is refreshed.
   */
  onUpdate(callback: SchedulerCallback): () => void {
    this.callbacks.add(callback);
    return () => this.callbacks.delete(callback);
  }

  /**
   * Notify all registered callbacks of a data update.
   */
  private notify(data: Parameters<SchedulerCallback>[0]): void {
    for (const callback of this.callbacks) {
      try {
        callback(data);
      } catch (error) {
        logger.error("Scheduler callback error", error);
      }
    }
  }

  /**
   * Fetch and cache listings data.
   */
  async refreshListings(): Promise<Asset[]> {
    try {
      logger.info("Refreshing listings...");
      const response = await cmcClient.getListings({ limit: 100 });
      const assets = response.data.map(cmcToAsset);

      // Cache the full list
      cache.set(CACHE_KEYS.listings(1, 100), assets, CACHE_CONFIG.listings);

      // Also cache individual assets
      for (const asset of assets) {
        cache.set(CACHE_KEYS.asset(asset.id), asset, CACHE_CONFIG.asset);
      }

      this.notify({ type: "listings", data: assets });
      logger.info(`Listings refreshed: ${assets.length} assets`);

      return assets;
    } catch (error) {
      logger.error("Failed to refresh listings", error);
      throw error;
    }
  }

  /**
   * Fetch and cache global metrics.
   */
  async refreshGlobalMetrics(): Promise<GlobalMetrics> {
    try {
      logger.info("Refreshing global metrics...");
      const response = await cmcClient.getGlobalMetrics();
      const metrics = cmcToGlobalMetrics(response);

      cache.set(CACHE_KEYS.globalMetrics(), metrics, CACHE_CONFIG.globalMetrics);

      this.notify({ type: "globalMetrics", data: metrics });
      logger.info("Global metrics refreshed");

      return metrics;
    } catch (error) {
      logger.error("Failed to refresh global metrics", error);
      throw error;
    }
  }

  /**
   * Fetch and cache fear & greed index.
   */
  async refreshFearGreed(): Promise<FearGreedData> {
    try {
      logger.info("Refreshing fear & greed...");
      const response = await cmcClient.getFearAndGreed();
      const fearGreed = cmcToFearGreed(response);

      cache.set(CACHE_KEYS.fearGreed(), fearGreed, CACHE_CONFIG.fearGreed);

      this.notify({ type: "fearGreed", data: fearGreed });
      logger.info("Fear & greed refreshed");

      return fearGreed;
    } catch (error) {
      logger.error("Failed to refresh fear & greed", error);
      throw error;
    }
  }

  /**
   * Start the scheduler with polling intervals matching cache TTL.
   */
  start(): void {
    if (this.isRunning) {
      logger.warn("Scheduler already running");
      return;
    }

    logger.info("Starting scheduler...");
    this.isRunning = true;

    // Initial fetch
    this.refreshListings().catch(() => {});
    this.refreshGlobalMetrics().catch(() => {});
    this.refreshFearGreed().catch(() => {});

    // Set up recurring intervals
    this.intervals.push(
      setInterval(() => {
        this.refreshListings().catch(() => {});
      }, CACHE_CONFIG.listings.ttl)
    );

    this.intervals.push(
      setInterval(() => {
        this.refreshGlobalMetrics().catch(() => {});
      }, CACHE_CONFIG.globalMetrics.ttl)
    );

    this.intervals.push(
      setInterval(() => {
        this.refreshFearGreed().catch(() => {});
      }, CACHE_CONFIG.fearGreed.ttl)
    );

    logger.info("Scheduler started with intervals:", {
      listings: `${CACHE_CONFIG.listings.ttl / 1000}s`,
      globalMetrics: `${CACHE_CONFIG.globalMetrics.ttl / 1000}s`,
      fearGreed: `${CACHE_CONFIG.fearGreed.ttl / 1000}s`,
    });
  }

  /**
   * Stop the scheduler.
   */
  stop(): void {
    if (!this.isRunning) return;

    logger.info("Stopping scheduler...");
    this.isRunning = false;

    for (const interval of this.intervals) {
      clearInterval(interval);
    }
    this.intervals = [];
    this.callbacks.clear();

    logger.info("Scheduler stopped");
  }

  /**
   * Check if the scheduler is running.
   */
  running(): boolean {
    return this.isRunning;
  }
}

// Default scheduler instance
export const scheduler = new Scheduler();
