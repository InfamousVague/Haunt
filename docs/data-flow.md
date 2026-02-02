# Data Flow

This document describes how data flows through the Haunt system from external APIs to clients.

## Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        External Price Sources                            │
├─────────────────────────────────────────────────────────────────────────┤
│  WebSocket Sources          │  REST Sources                             │
│  ├─ Coinbase (real-time)    │  ├─ CoinGecko (aggregated)               │
│  ├─ Binance (real-time)     │  ├─ CoinMarketCap (market data)          │
│  ├─ KuCoin (real-time)      │  ├─ CryptoCompare (historical)           │
│  ├─ OKX (real-time)         │  └─ Kraken (periodic)                    │
│  └─ Huobi (real-time)       │                                           │
└───────────────┬─────────────┴───────────────────────────────────────────┘
                │
                ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                      Multi-Source Coordinator                            │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐               │
│  │ Price Cache   │  │ Chart Store   │  │ Historical    │               │
│  │ (in-memory)   │  │ (in-memory)   │  │ Service       │               │
│  │               │  │               │  │ (Redis)       │               │
│  │ - Aggregation │  │ - OHLC data   │  │               │               │
│  │ - Direction   │  │ - Sparklines  │  │ - Seeding     │               │
│  │ - Sources     │  │ - Buckets     │  │ - Persistence │               │
│  └───────┬───────┘  └───────┬───────┘  └───────┬───────┘               │
└──────────┼──────────────────┼──────────────────┼────────────────────────┘
           │                  │                  │
           └──────────────────┼──────────────────┘
                              │
           ┌──────────────────┼──────────────────┐
           │                  │                  │
           ▼                  ▼                  ▼
    ┌──────────────┐  ┌──────────────┐  ┌──────────────┐
    │  REST API    │  │  WebSocket   │  │  Background  │
    │  /api/*      │  │  /ws         │  │  Tasks       │
    └──────┬───────┘  └──────┬───────┘  └──────────────┘
           │                 │
           └────────┬────────┘
                    │
                    ▼
    ┌─────────────────────────────────────────────────┐
    │                    Clients                       │
    │  (Wraith Frontend, Mobile Apps, Third Parties)  │
    └─────────────────────────────────────────────────┘
```

## Price Aggregation Flow

### 1. Price Reception

Prices arrive from multiple sources:

```rust
// WebSocket price update from Coinbase
{
    "symbol": "btc",
    "price": 50000.00,
    "source": "coinbase",
    "timestamp": 1704067200000
}
```

### 2. Multi-Source Aggregation

The `MultiSourceCoordinator` aggregates prices from all sources:

```rust
// Aggregation strategy
fn aggregate_price(prices: Vec<PricePoint>) -> AggregatedPrice {
    // 1. Filter stale prices (> 30 seconds old)
    // 2. Calculate weighted average based on source reliability
    // 3. Track all contributing sources
    // 4. Determine trade direction (up/down)
}
```

**Source Weights:**
| Source | Weight | Reason |
|--------|--------|--------|
| Coinbase | 1.0 | High liquidity, reliable |
| Binance | 1.0 | Highest volume |
| KuCoin | 0.8 | Good liquidity |
| OKX | 0.8 | Major exchange |
| Huobi | 0.7 | Regional exchange |
| CoinGecko | 0.5 | Aggregated (delayed) |

### 3. Price Cache Update

The `PriceCache` stores the latest aggregated price:

```rust
pub struct CachedPrice {
    pub symbol: String,
    pub price: f64,
    pub previous_price: Option<f64>,
    pub change_24h: Option<f64>,
    pub volume_24h: Option<f64>,
    pub trade_direction: Option<TradeDirection>,
    pub source: String,
    pub sources: Vec<String>,
    pub timestamp: u64,
}
```

### 4. Client Notification

WebSocket clients subscribed to the asset receive updates:

```json
{
  "type": "price_update",
  "data": {
    "id": "btc",
    "symbol": "btc",
    "price": 50000.00,
    "previousPrice": 49500.00,
    "tradeDirection": "up",
    "source": "coinbase",
    "sources": ["coinbase", "binance", "coingecko"],
    "timestamp": 1704067200000
  }
}
```

## Chart Data Flow

### 1. Chart Request

Client requests chart data:

```
GET /api/crypto/1/chart?range=1d
```

### 2. Data Resolution

The system checks multiple sources for data:

```rust
async fn get_chart_data(symbol: &str, range: &str) -> ChartData {
    // 1. Check in-memory chart store
    // 2. Check Redis cache
    // 3. If inadequate, trigger background seeding
    // 4. Return available data with seeding status
}
```

### 3. OHLC Bucket Calculation

Chart data is organized into OHLC buckets:

| Range | Bucket Size | Expected Points |
|-------|-------------|-----------------|
| 1h | 1 minute | 60 |
| 4h | 5 minutes | 48 |
| 1d | 5 minutes | 288 |
| 1w | 1 hour | 168 |
| 1m | 1 hour | 720 |

### 4. Real-Time Updates

As new prices arrive, chart buckets are updated:

```rust
fn update_chart_bucket(symbol: &str, price: f64, timestamp: u64) {
    let bucket = get_current_bucket(timestamp, bucket_size);

    if bucket.is_new() {
        // Create new OHLC point
        bucket.open = price;
        bucket.high = price;
        bucket.low = price;
        bucket.close = price;
    } else {
        // Update existing bucket
        bucket.high = bucket.high.max(price);
        bucket.low = bucket.low.min(price);
        bucket.close = price;
    }
}
```

## Historical Data Seeding Flow

### 1. Seeding Trigger

Seeding is triggered when:
- Chart data is requested but inadequate
- Manual seed request via API
- Batch seeding for top assets

### 2. Multi-Source Fetching

```rust
async fn seed_historical_data(symbol: &str) {
    // Phase 1: CoinGecko (0-50%)
    let cg_data = fetch_coingecko_market_chart(symbol).await?;
    emit_progress(symbol, 50);

    // Phase 2: CryptoCompare hourly (50-90%)
    let cc_hourly = fetch_cryptocompare_hourly(symbol).await?;
    emit_progress(symbol, 90);

    // Phase 3: CryptoCompare daily (90-100%)
    let cc_daily = fetch_cryptocompare_daily(symbol).await?;
    emit_progress(symbol, 100);

    // Merge and store
    merge_and_store(cg_data, cc_hourly, cc_daily).await?;
}
```

### 3. Progress Notification

Clients receive progress updates via WebSocket:

```json
{
  "type": "seeding_progress",
  "data": {
    "symbol": "btc",
    "status": "in_progress",
    "progress": 50,
    "message": "Fetching from CoinGecko..."
  }
}
```

### 4. Data Persistence

Seeded data is stored in Redis:

```
Key: haunt:chart:{symbol}:{range}
TTL: 24 hours for short ranges, 7 days for long ranges
```

## Listing Data Flow

### 1. CoinMarketCap Integration

Listings are fetched from CoinMarketCap:

```
GET https://pro-api.coinmarketcap.com/v1/cryptocurrency/listings/latest
```

### 2. Data Enrichment

CMC data is enriched with real-time prices:

```rust
fn enrich_listing(cmc_asset: CmcAsset, cache: &PriceCache) -> Asset {
    let real_time = cache.get(&cmc_asset.symbol);

    Asset {
        id: cmc_asset.id,
        name: cmc_asset.name,
        symbol: cmc_asset.symbol,
        price: real_time.map(|r| r.price).unwrap_or(cmc_asset.price),
        trade_direction: real_time.and_then(|r| r.trade_direction),
        // ... other fields
    }
}
```

### 3. Caching Strategy

| Data Type | Cache Location | TTL |
|-----------|---------------|-----|
| Listings | Redis | 5 minutes |
| Asset details | Redis | 5 minutes |
| Real-time prices | In-memory | N/A (streaming) |
| Chart data | Redis + In-memory | 24h / 7d |

## Error Handling

### Source Failures

When a price source fails:

1. Log the error with source details
2. Continue aggregating from remaining sources
3. Mark source as unhealthy
4. Retry connection with exponential backoff

### Data Gaps

When chart data has gaps:

1. Interpolate small gaps (< 3 buckets)
2. Mark larger gaps as missing
3. Return `dataCompleteness` percentage to client
4. Client decides whether to show partial data

## Performance Considerations

### Memory Usage

- Price cache: ~100 bytes per asset × 1000 assets = ~100KB
- Chart store: ~50 bytes per point × 1000 points × 100 assets = ~5MB
- WebSocket connections: ~1KB per connection

### Latency

| Operation | Target Latency |
|-----------|---------------|
| Price update (source → client) | < 100ms |
| REST API response | < 50ms |
| Chart data fetch (cached) | < 20ms |
| Chart data fetch (seeding) | 5-30 seconds |

### Throughput

- WebSocket: 10,000+ concurrent connections
- REST API: 1,000+ requests/second
- Price updates: 100+ updates/second across all assets
