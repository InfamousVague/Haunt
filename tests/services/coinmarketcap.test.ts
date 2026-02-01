import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { cmcToAsset, cmcToGlobalMetrics, cmcToFearGreed } from "../../src/services/coinmarketcap.js";
import type { CMCCryptocurrency, CMCGlobalMetricsResponse, CMCFearGreedResponse } from "../../src/types/index.js";

describe("cmcToAsset", () => {
  const mockCrypto: CMCCryptocurrency = {
    id: 1,
    name: "Bitcoin",
    symbol: "BTC",
    slug: "bitcoin",
    cmc_rank: 1,
    num_market_pairs: 10000,
    circulating_supply: 19000000,
    total_supply: 21000000,
    max_supply: 21000000,
    infinite_supply: false,
    last_updated: "2024-01-01T00:00:00.000Z",
    date_added: "2013-04-28T00:00:00.000Z",
    tags: ["mineable", "pow"],
    platform: null,
    self_reported_circulating_supply: null,
    self_reported_market_cap: null,
    quote: {
      USD: {
        price: 50000,
        volume_24h: 25000000000,
        volume_change_24h: 5.5,
        percent_change_1h: 0.5,
        percent_change_24h: 2.5,
        percent_change_7d: 10,
        percent_change_30d: 15,
        market_cap: 950000000000,
        market_cap_dominance: 45,
        fully_diluted_market_cap: 1050000000000,
        last_updated: "2024-01-01T00:00:00.000Z",
      },
    },
  };

  it("should convert CMC crypto to Asset", () => {
    const asset = cmcToAsset(mockCrypto);

    expect(asset.id).toBe(1);
    expect(asset.rank).toBe(1);
    expect(asset.name).toBe("Bitcoin");
    expect(asset.symbol).toBe("BTC");
    expect(asset.price).toBe(50000);
    expect(asset.change1h).toBe(0.5);
    expect(asset.change24h).toBe(2.5);
    expect(asset.change7d).toBe(10);
    expect(asset.marketCap).toBe(950000000000);
    expect(asset.volume24h).toBe(25000000000);
    expect(asset.circulatingSupply).toBe(19000000);
    expect(asset.maxSupply).toBe(21000000);
  });

  it("should generate correct image URL", () => {
    const asset = cmcToAsset(mockCrypto);
    expect(asset.image).toBe("https://s2.coinmarketcap.com/static/img/coins/64x64/1.png");
  });

  it("should generate sparkline with 7 points", () => {
    const asset = cmcToAsset(mockCrypto);
    expect(asset.sparkline).toHaveLength(7);
    expect(asset.sparkline[asset.sparkline.length - 1]).toBe(50000);
  });

  it("should handle null max supply", () => {
    const cryptoWithNullMax = { ...mockCrypto, max_supply: null };
    const asset = cmcToAsset(cryptoWithNullMax);
    expect(asset.maxSupply).toBeUndefined();
  });
});

describe("cmcToGlobalMetrics", () => {
  const mockResponse: CMCGlobalMetricsResponse = {
    status: {
      timestamp: "2024-01-01T00:00:00.000Z",
      error_code: 0,
      error_message: null,
      elapsed: 10,
      credit_count: 1,
    },
    data: {
      active_cryptocurrencies: 10000,
      total_cryptocurrencies: 25000,
      active_market_pairs: 50000,
      active_exchanges: 500,
      total_exchanges: 1000,
      eth_dominance: 18,
      btc_dominance: 45,
      eth_dominance_yesterday: 17.5,
      btc_dominance_yesterday: 44.5,
      eth_dominance_24h_percentage_change: 0.5,
      btc_dominance_24h_percentage_change: 0.5,
      defi_volume_24h: 5000000000,
      defi_volume_24h_reported: 5000000000,
      defi_market_cap: 50000000000,
      defi_24h_percentage_change: 2,
      stablecoin_volume_24h: 50000000000,
      stablecoin_volume_24h_reported: 50000000000,
      stablecoin_market_cap: 150000000000,
      stablecoin_24h_percentage_change: 0.1,
      derivatives_volume_24h: 100000000000,
      derivatives_volume_24h_reported: 100000000000,
      derivatives_24h_percentage_change: 3,
      quote: {
        USD: {
          total_market_cap: 2000000000000,
          total_volume_24h: 100000000000,
          total_volume_24h_reported: 100000000000,
          altcoin_volume_24h: 50000000000,
          altcoin_volume_24h_reported: 50000000000,
          altcoin_market_cap: 1000000000000,
          defi_volume_24h: 5000000000,
          defi_volume_24h_reported: 5000000000,
          defi_24h_percentage_change: 2,
          defi_market_cap: 50000000000,
          stablecoin_volume_24h: 50000000000,
          stablecoin_volume_24h_reported: 50000000000,
          stablecoin_24h_percentage_change: 0.1,
          stablecoin_market_cap: 150000000000,
          derivatives_volume_24h: 100000000000,
          derivatives_volume_24h_reported: 100000000000,
          derivatives_24h_percentage_change: 3,
          total_market_cap_yesterday: 1950000000000,
          total_volume_24h_yesterday: 95000000000,
          total_market_cap_yesterday_percentage_change: 2.5,
          total_volume_24h_yesterday_percentage_change: 5.2,
          last_updated: "2024-01-01T00:00:00.000Z",
        },
      },
      last_updated: "2024-01-01T00:00:00.000Z",
    },
  };

  it("should convert CMC global metrics to GlobalMetrics", () => {
    const metrics = cmcToGlobalMetrics(mockResponse);

    expect(metrics.totalMarketCap).toBe(2000000000000);
    expect(metrics.totalVolume24h).toBe(100000000000);
    expect(metrics.btcDominance).toBe(45);
    expect(metrics.ethDominance).toBe(18);
    expect(metrics.activeCryptocurrencies).toBe(10000);
    expect(metrics.activeExchanges).toBe(500);
    expect(metrics.marketCapChange24h).toBe(2.5);
    expect(metrics.volumeChange24h).toBe(5.2);
    expect(metrics.lastUpdated).toBe("2024-01-01T00:00:00.000Z");
  });
});

describe("cmcToFearGreed", () => {
  const mockResponse: CMCFearGreedResponse = {
    status: {
      timestamp: "2024-01-01T00:00:00.000Z",
      error_code: 0,
      error_message: null,
      elapsed: 10,
      credit_count: 1,
    },
    data: {
      value: 75,
      value_classification: "Greed",
      timestamp: "2024-01-01T00:00:00.000Z",
      update_time: "2024-01-01T00:00:00.000Z",
    },
  };

  it("should convert CMC fear & greed to FearGreedData", () => {
    const fearGreed = cmcToFearGreed(mockResponse);

    expect(fearGreed.value).toBe(75);
    expect(fearGreed.classification).toBe("Greed");
    expect(fearGreed.timestamp).toBe("2024-01-01T00:00:00.000Z");
  });
});
