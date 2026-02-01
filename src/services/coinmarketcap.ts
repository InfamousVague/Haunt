import { logger } from "../utils/logger.js";
import { CMCError } from "../utils/errors.js";
import type {
  Asset,
  CMCCryptocurrency,
  CMCListingsResponse,
  CMCFearGreedResponse,
  CMCGlobalMetricsResponse,
  GlobalMetrics,
  FearGreedData,
} from "../types/index.js";

const BASE_URL = "https://pro-api.coinmarketcap.com";

export type ListingsParams = {
  start?: number;
  limit?: number;
  sort?:
    | "market_cap"
    | "name"
    | "symbol"
    | "date_added"
    | "price"
    | "circulating_supply"
    | "total_supply"
    | "max_supply"
    | "num_market_pairs"
    | "volume_24h"
    | "percent_change_1h"
    | "percent_change_24h"
    | "percent_change_7d";
  sort_dir?: "asc" | "desc";
  cryptocurrency_type?: "all" | "coins" | "tokens";
  tag?: string;
  aux?: string;
};

/**
 * CoinMarketCap API client for server-side usage.
 */
export class CoinMarketCapClient {
  private apiKey: string;

  constructor(apiKey?: string) {
    this.apiKey = apiKey || process.env.CMC_API_KEY || "";
    if (!this.apiKey) {
      logger.warn("No CMC_API_KEY provided - API calls will fail");
    }
  }

  private async fetch<T>(
    endpoint: string,
    params?: Record<string, string | number>
  ): Promise<T> {
    const url = new URL(`${BASE_URL}${endpoint}`);

    if (params) {
      Object.entries(params).forEach(([key, value]) => {
        if (value !== undefined) {
          url.searchParams.append(key, String(value));
        }
      });
    }

    const headers: Record<string, string> = {
      Accept: "application/json",
      "X-CMC_PRO_API_KEY": this.apiKey,
    };

    logger.request("GET", endpoint, params);
    const startTime = performance.now();

    try {
      const response = await fetch(url.toString(), { headers });
      const duration = Math.round(performance.now() - startTime);

      if (!response.ok) {
        const errorText = await response.text();
        logger.error(`CMC API error: ${response.status}`, errorText);
        throw new CMCError(
          `${response.status} ${response.statusText}`,
          response.status >= 500 ? 502 : response.status
        );
      }

      const data = (await response.json()) as T;
      logger.response(endpoint, response.status, duration);

      return data;
    } catch (error) {
      if (error instanceof CMCError) throw error;

      logger.error("CMC fetch failed", error);
      throw new CMCError(
        error instanceof Error ? error.message : "Network error"
      );
    }
  }

  /**
   * Get a paginated list of all active cryptocurrencies with latest market data.
   */
  async getListings(params: ListingsParams = {}): Promise<CMCListingsResponse> {
    return this.fetch("/v1/cryptocurrency/listings/latest", {
      start: params.start || 1,
      limit: params.limit || 100,
      sort: params.sort || "market_cap",
      sort_dir: params.sort_dir || "desc",
      cryptocurrency_type: params.cryptocurrency_type || "all",
      ...(params.tag && { tag: params.tag }),
      ...(params.aux && { aux: params.aux }),
    });
  }

  /**
   * Get latest quotes for specific cryptocurrencies by ID.
   */
  async getQuotes(ids: number[]): Promise<CMCListingsResponse> {
    const response = await this.fetch<{
      status: CMCListingsResponse["status"];
      data: Record<string, CMCCryptocurrency>;
    }>("/v2/cryptocurrency/quotes/latest", {
      id: ids.join(","),
    });

    // Convert the object format to array format
    return {
      status: response.status,
      data: Object.values(response.data),
    };
  }

  /**
   * Get global cryptocurrency market metrics.
   */
  async getGlobalMetrics(): Promise<CMCGlobalMetricsResponse> {
    return this.fetch("/v1/global-metrics/quotes/latest");
  }

  /**
   * Get the Fear & Greed Index.
   */
  async getFearAndGreed(): Promise<CMCFearGreedResponse> {
    return this.fetch("/v3/fear-and-greed/latest");
  }

  /**
   * Get metadata for specific cryptocurrencies by ID.
   */
  async getMetadata(
    ids: number[]
  ): Promise<{ data: Record<string, unknown> }> {
    return this.fetch("/v2/cryptocurrency/info", {
      id: ids.join(","),
    });
  }
}

/**
 * Generate synthetic sparkline data based on current price and 7-day change.
 * Creates a realistic-looking trend with some noise.
 */
function generateSparkline(
  currentPrice: number,
  change7d: number,
  points = 7
): number[] {
  // Calculate the starting price based on the 7-day change
  const startPrice = currentPrice / (1 + change7d / 100);
  const sparkline: number[] = [];

  // Generate points with some random variation
  for (let i = 0; i < points; i++) {
    const progress = i / (points - 1);
    // Linear interpolation with some noise
    const baseValue = startPrice + (currentPrice - startPrice) * progress;
    // Add random noise (+-2% of the price range)
    const noise =
      (Math.random() - 0.5) * Math.abs(currentPrice - startPrice) * 0.4;
    sparkline.push(baseValue + noise);
  }

  // Ensure the last point is the current price
  sparkline[sparkline.length - 1] = currentPrice;

  return sparkline;
}

/**
 * Convert CMC cryptocurrency data to our Asset type.
 */
export function cmcToAsset(crypto: CMCCryptocurrency): Asset {
  const quote = crypto.quote.USD;
  return {
    id: crypto.id,
    rank: crypto.cmc_rank,
    name: crypto.name,
    symbol: crypto.symbol,
    image: `https://s2.coinmarketcap.com/static/img/coins/64x64/${crypto.id}.png`,
    price: quote.price,
    change1h: quote.percent_change_1h,
    change24h: quote.percent_change_24h,
    change7d: quote.percent_change_7d,
    marketCap: quote.market_cap,
    volume24h: quote.volume_24h,
    circulatingSupply: crypto.circulating_supply,
    maxSupply: crypto.max_supply || undefined,
    sparkline: generateSparkline(quote.price, quote.percent_change_7d),
  };
}

/**
 * Convert CMC global metrics to simplified format.
 */
export function cmcToGlobalMetrics(
  response: CMCGlobalMetricsResponse
): GlobalMetrics {
  const data = response.data;
  const quote = data.quote.USD;

  return {
    totalMarketCap: quote.total_market_cap,
    totalVolume24h: quote.total_volume_24h,
    btcDominance: data.btc_dominance,
    ethDominance: data.eth_dominance,
    activeCryptocurrencies: data.active_cryptocurrencies,
    activeExchanges: data.active_exchanges,
    marketCapChange24h: quote.total_market_cap_yesterday_percentage_change,
    volumeChange24h: quote.total_volume_24h_yesterday_percentage_change,
    lastUpdated: data.last_updated,
  };
}

/**
 * Convert CMC fear & greed to simplified format.
 */
export function cmcToFearGreed(response: CMCFearGreedResponse): FearGreedData {
  return {
    value: response.data.value,
    classification: response.data.value_classification,
    timestamp: response.data.timestamp,
  };
}

// Default client instance
export const cmcClient = new CoinMarketCapClient();
