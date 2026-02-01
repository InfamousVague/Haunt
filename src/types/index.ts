// Asset types that match the frontend
export type Asset = {
  id: number;
  rank: number;
  name: string;
  symbol: string;
  image: string;
  price: number;
  change1h: number;
  change24h: number;
  change7d: number;
  marketCap: number;
  volume24h: number;
  circulatingSupply: number;
  maxSupply?: number;
  sparkline: number[];
};

// CoinMarketCap API types
export type CMCCryptocurrency = {
  id: number;
  name: string;
  symbol: string;
  slug: string;
  cmc_rank: number;
  num_market_pairs: number;
  circulating_supply: number;
  total_supply: number;
  max_supply: number | null;
  infinite_supply: boolean;
  last_updated: string;
  date_added: string;
  tags: string[];
  platform: {
    id: number;
    name: string;
    symbol: string;
    slug: string;
    token_address: string;
  } | null;
  self_reported_circulating_supply: number | null;
  self_reported_market_cap: number | null;
  quote: {
    USD: {
      price: number;
      volume_24h: number;
      volume_change_24h: number;
      percent_change_1h: number;
      percent_change_24h: number;
      percent_change_7d: number;
      percent_change_30d: number;
      market_cap: number;
      market_cap_dominance: number;
      fully_diluted_market_cap: number;
      last_updated: string;
    };
  };
};

export type CMCListingsResponse = {
  status: {
    timestamp: string;
    error_code: number;
    error_message: string | null;
    elapsed: number;
    credit_count: number;
  };
  data: CMCCryptocurrency[];
};

export type CMCFearGreedIndex = {
  value: number;
  value_classification: string;
  timestamp: string;
  update_time: string;
};

export type CMCFearGreedResponse = {
  status: {
    timestamp: string;
    error_code: number;
    error_message: string | null;
    elapsed: number;
    credit_count: number;
  };
  data: CMCFearGreedIndex;
};

export type CMCGlobalMetrics = {
  active_cryptocurrencies: number;
  total_cryptocurrencies: number;
  active_market_pairs: number;
  active_exchanges: number;
  total_exchanges: number;
  eth_dominance: number;
  btc_dominance: number;
  eth_dominance_yesterday: number;
  btc_dominance_yesterday: number;
  eth_dominance_24h_percentage_change: number;
  btc_dominance_24h_percentage_change: number;
  defi_volume_24h: number;
  defi_volume_24h_reported: number;
  defi_market_cap: number;
  defi_24h_percentage_change: number;
  stablecoin_volume_24h: number;
  stablecoin_volume_24h_reported: number;
  stablecoin_market_cap: number;
  stablecoin_24h_percentage_change: number;
  derivatives_volume_24h: number;
  derivatives_volume_24h_reported: number;
  derivatives_24h_percentage_change: number;
  quote: {
    USD: {
      total_market_cap: number;
      total_volume_24h: number;
      total_volume_24h_reported: number;
      altcoin_volume_24h: number;
      altcoin_volume_24h_reported: number;
      altcoin_market_cap: number;
      defi_volume_24h: number;
      defi_volume_24h_reported: number;
      defi_24h_percentage_change: number;
      defi_market_cap: number;
      stablecoin_volume_24h: number;
      stablecoin_volume_24h_reported: number;
      stablecoin_24h_percentage_change: number;
      stablecoin_market_cap: number;
      derivatives_volume_24h: number;
      derivatives_volume_24h_reported: number;
      derivatives_24h_percentage_change: number;
      total_market_cap_yesterday: number;
      total_volume_24h_yesterday: number;
      total_market_cap_yesterday_percentage_change: number;
      total_volume_24h_yesterday_percentage_change: number;
      last_updated: string;
    };
  };
  last_updated: string;
};

export type CMCGlobalMetricsResponse = {
  status: {
    timestamp: string;
    error_code: number;
    error_message: string | null;
    elapsed: number;
    credit_count: number;
  };
  data: CMCGlobalMetrics;
};

// Simplified global metrics for API response
export type GlobalMetrics = {
  totalMarketCap: number;
  totalVolume24h: number;
  btcDominance: number;
  ethDominance: number;
  activeCryptocurrencies: number;
  activeExchanges: number;
  marketCapChange24h: number;
  volumeChange24h: number;
  lastUpdated: string;
};

// Fear & Greed response
export type FearGreedData = {
  value: number;
  classification: string;
  timestamp: string;
};

// WebSocket message types
export type WSMessageType =
  | "subscribe"
  | "unsubscribe"
  | "price_update"
  | "market_update"
  | "error"
  | "subscribed"
  | "unsubscribed";

export type WSMessage = {
  type: WSMessageType;
  data?: unknown;
  assets?: string[];
  error?: string;
};

export type PriceUpdate = {
  id: number;
  symbol: string;
  price: number;
  change24h: number;
  volume24h: number;
  timestamp: string;
};

export type MarketUpdate = {
  totalMarketCap: number;
  totalVolume24h: number;
  btcDominance: number;
  timestamp: string;
};

// Cache configuration
export type CacheConfig = {
  listings: { ttl: number };
  quotes: { ttl: number };
  globalMetrics: { ttl: number };
  fearGreed: { ttl: number };
};
