# Haunt API Reference

> Complete REST API documentation for the Haunt cryptocurrency data server.

**Base URL:** `http://localhost:3000`

**Download:** [Postman Collection](./haunt-api.postman_collection.json)

---

## Table of Contents

- [Health](#health)
- [Authentication](#authentication)
- [Cryptocurrency](#cryptocurrency)
- [Market Data](#market-data)
- [Trading Signals](#trading-signals)
- [Order Book](#order-book)
- [Peer Mesh](#peer-mesh)
- [Error Handling](#error-handling)

---

## Health

### GET /api/health

Check server health status.

**Response:**
```json
{
  "status": "ok",
  "version": "0.1.0"
}
```

---

## Authentication

Authentication uses a challenge-response flow with cryptographic signatures.

### GET /api/auth/challenge

Get a challenge string to sign for authentication.

**Response:**
```json
{
  "data": {
    "challenge": "haunt:1700000000000:abc123def456",
    "expiresAt": 1700000300000
  }
}
```

### POST /api/auth/verify

Verify a signed challenge and create a session.

**Request Body:**
```json
{
  "publicKey": "0x1234567890abcdef...",
  "signature": "0xabcdef1234567890...",
  "challenge": "haunt:1700000000000:abc123def456"
}
```

**Response:**
```json
{
  "data": {
    "authenticated": true,
    "publicKey": "0x1234567890abcdef...",
    "sessionToken": "eyJhbGciOiJIUzI1NiIs...",
    "expiresAt": 1700086400000,
    "profile": {
      "displayName": "User",
      "settings": {}
    }
  }
}
```

### GET /api/auth/me

Get the current authenticated user's profile.

**Headers:**
```
Authorization: Bearer <sessionToken>
```

**Response:**
```json
{
  "data": {
    "displayName": "User",
    "settings": {
      "theme": "dark",
      "notifications": true
    }
  }
}
```

### PUT /api/auth/profile

Update the authenticated user's profile settings.

**Headers:**
```
Authorization: Bearer <sessionToken>
```

**Request Body:**
```json
{
  "theme": "light",
  "notifications": false
}
```

**Response:**
```json
{
  "data": {
    "displayName": "User",
    "settings": {
      "theme": "light",
      "notifications": false
    }
  }
}
```

### POST /api/auth/logout

Logout and invalidate the current session.

**Headers:**
```
Authorization: Bearer <sessionToken>
```

**Response:**
```json
{
  "data": {
    "success": true
  }
}
```

---

## Cryptocurrency

### GET /api/crypto/listings

Get paginated cryptocurrency listings with filtering and sorting.

**Query Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `start` | integer | 1 | Starting position |
| `limit` | integer | 20 | Number of results (max 100) |
| `sort` | string | `market_cap` | Sort field |
| `sort_dir` | string | `desc` | Sort direction (`asc`/`desc`) |
| `filter` | string | `all` | Filter type |
| `min_change` | float | - | Minimum 24h change % |
| `max_change` | float | - | Maximum 24h change % |

**Sort Fields:**
- `market_cap` - Market capitalization
- `price` - Current price
- `volume_24h` - 24-hour trading volume
- `percent_change_1h` - 1-hour price change
- `percent_change_24h` - 24-hour price change
- `percent_change_7d` - 7-day price change
- `name` - Asset name alphabetically

**Filters:**
- `all` - All assets
- `gainers` - Positive 24h change
- `losers` - Negative 24h change
- `most_volatile` - Sorted by absolute change
- `top_volume` - Sorted by volume

**Response:**
```json
{
  "data": [
    {
      "id": 1,
      "rank": 1,
      "name": "Bitcoin",
      "symbol": "BTC",
      "image": "https://s2.coinmarketcap.com/static/img/coins/64x64/1.png",
      "price": 50000.00,
      "change1h": 0.5,
      "change24h": 2.5,
      "change7d": 5.0,
      "marketCap": 1000000000000,
      "volume24h": 50000000000,
      "circulatingSupply": 19000000,
      "maxSupply": 21000000,
      "sparkline": [49000, 49500, 50000],
      "tradeDirection": "up"
    }
  ],
  "meta": {
    "cached": false,
    "total": 100,
    "start": 1,
    "limit": 20
  }
}
```

### GET /api/crypto/search

Search for assets by name or symbol.

**Query Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `q` | string | required | Search query |
| `limit` | integer | 10 | Number of results (max 50) |

**Response:**
```json
{
  "data": [
    {
      "id": 1,
      "rank": 1,
      "name": "Bitcoin",
      "symbol": "BTC",
      "image": "https://...",
      "price": 50000.00
    }
  ],
  "meta": {
    "query": "bit",
    "limit": 10
  }
}
```

### GET /api/crypto/:id

Get detailed information for a specific asset.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | string | Asset ID or symbol (e.g., `1` or `btc`) |

**Response:**
```json
{
  "data": {
    "id": 1,
    "rank": 1,
    "name": "Bitcoin",
    "symbol": "BTC",
    "image": "https://...",
    "price": 50000.00,
    "change1h": 0.5,
    "change24h": 2.5,
    "change7d": 5.0,
    "marketCap": 1000000000000,
    "volume24h": 50000000000,
    "circulatingSupply": 19000000,
    "maxSupply": 21000000,
    "sparkline": [49000, 49500, 50000],
    "tradeDirection": "up"
  },
  "meta": {
    "cached": false
  }
}
```

### GET /api/crypto/:id/quotes

Get price quote data for an asset.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | string | Asset ID or symbol |

**Response:**
```json
{
  "data": {
    "symbol": "btc",
    "price": 50000.00,
    "volume24h": 50000000000,
    "marketCap": 1000000000000,
    "change1h": 0.5,
    "change24h": 2.5,
    "change7d": 5.0,
    "lastUpdated": 1700000000000
  },
  "meta": {
    "cached": false
  }
}
```

### GET /api/crypto/:id/chart

Get OHLC chart data for an asset.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | string | Asset ID or symbol |

**Query Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `range` | string | `1d` | Time range |

**Range Values:**
- `1h` - 1 hour (1-minute buckets)
- `4h` - 4 hours (5-minute buckets)
- `1d` - 1 day (5-minute buckets)
- `1w` - 1 week (1-hour buckets)
- `1m` - 1 month (1-hour buckets)

**Response:**
```json
{
  "data": {
    "symbol": "btc",
    "range": "1d",
    "data": [
      {
        "time": 1704067200,
        "open": 50000.0,
        "high": 50500.0,
        "low": 49800.0,
        "close": 50200.0,
        "volume": 1000000000
      }
    ],
    "seeding": false,
    "seedingStatus": "complete",
    "seedingProgress": 100,
    "dataCompleteness": 95,
    "expectedPoints": 288
  },
  "meta": {
    "cached": false
  }
}
```

**Seeding Status Values:**
- `not_started` - No historical data fetch attempted
- `in_progress` - Currently fetching data
- `complete` - Data fetch complete
- `failed` - Data fetch failed

### POST /api/crypto/seed

Trigger historical data seeding for a symbol.

**Request Body:**
```json
{
  "symbol": "btc"
}
```

**Response:**
```json
{
  "data": {
    "symbol": "btc",
    "status": "seeding",
    "message": "Historical data seeding started"
  }
}
```

### POST /api/crypto/seed/batch

Trigger historical data seeding for multiple symbols.

**Request Body:**
```json
{
  "symbols": ["btc", "eth", "sol"]
}
```

**Response:**
```json
{
  "data": [
    { "symbol": "btc", "status": "seeding", "message": "Seeding started" },
    { "symbol": "eth", "status": "seeded", "message": "Already seeded" },
    { "symbol": "sol", "status": "seeding", "message": "Seeding started" }
  ]
}
```

### GET /api/crypto/seed/status

Get seeding status for all known symbols.

**Response:**
```json
{
  "data": [
    { "symbol": "btc", "status": "seeded", "message": "" },
    { "symbol": "eth", "status": "seeding", "message": "" },
    { "symbol": "sol", "status": "not_seeded", "message": "" }
  ]
}
```

---

## Market Data

### GET /api/market/global

Get global cryptocurrency market metrics.

**Response:**
```json
{
  "data": {
    "totalMarketCap": 2500000000000,
    "totalVolume24h": 100000000000,
    "btcDominance": 48.5,
    "ethDominance": 17.2,
    "activeCoins": 10000,
    "markets": 800,
    "marketCapChange24h": 2.5
  },
  "meta": {
    "cached": false
  }
}
```

### GET /api/market/fear-greed

Get the Fear and Greed Index data.

**Response:**
```json
{
  "data": {
    "value": 65,
    "classification": "Greed",
    "timestamp": 1700000000000,
    "previousClose": 60,
    "change": 5
  },
  "meta": {
    "cached": false
  }
}
```

**Classification Values:**
- `Extreme Fear` (0-24)
- `Fear` (25-44)
- `Neutral` (45-55)
- `Greed` (56-75)
- `Extreme Greed` (76-100)

### GET /api/market/exchanges

Get statistics for all tracked exchanges.

**Response:**
```json
{
  "data": [
    {
      "source": "binance",
      "updateCount": 150000,
      "updatePercent": 35.5,
      "online": true,
      "lastError": null,
      "lastUpdate": 1700000000000
    },
    {
      "source": "coinbase",
      "updateCount": 120000,
      "updatePercent": 28.3,
      "online": true,
      "lastError": null,
      "lastUpdate": 1700000000000
    }
  ],
  "meta": {
    "cached": false
  }
}
```

### GET /api/market/stats

Get overall market statistics and server metrics.

**Response:**
```json
{
  "data": {
    "totalUpdates": 1000000,
    "tps": 125.5,
    "uptimeSecs": 86400,
    "activeSymbols": 500,
    "onlineSources": 8,
    "totalSources": 12,
    "exchanges": [...]
  },
  "meta": {
    "cached": false
  }
}
```

### GET /api/market/source-stats/:symbol

Get per-source statistics for a specific symbol.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `symbol` | string | Asset symbol (e.g., `btc`) |

**Response:**
```json
{
  "data": {
    "symbol": "btc",
    "sources": [
      {
        "source": "binance",
        "updateCount": 5000,
        "updatePercent": 50.0,
        "online": true
      },
      {
        "source": "coinbase",
        "updateCount": 3000,
        "updatePercent": 30.0,
        "online": true
      }
    ],
    "totalUpdates": 10000,
    "timestamp": 1700000000
  },
  "meta": {
    "cached": false
  }
}
```

### GET /api/market/confidence/:symbol

Get data confidence metrics for a symbol.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `symbol` | string | Asset symbol (e.g., `btc`) |

**Response:**
```json
{
  "data": {
    "symbol": "btc",
    "confidence": {
      "score": 85,
      "sourceCount": 8,
      "onlineSources": 7,
      "totalUpdates": 10000,
      "currentPrice": 50000.0,
      "priceSpreadPercent": 0.5,
      "secondsSinceUpdate": 5,
      "factors": {
        "sourceDiversity": 25,
        "updateFrequency": 20,
        "dataRecency": 22,
        "priceConsistency": 18
      }
    },
    "chartDataPoints": 1440,
    "timestamp": 1700000000
  },
  "meta": {
    "cached": false
  }
}
```

### GET /api/market/movers

Get top gainers and losers by timeframe.

**Query Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `timeframe` | string | `1h` | Time period for change calculation |
| `limit` | integer | 10 | Number of results (max 50) |
| `asset_type` | string | `all` | Asset type filter |

**Timeframe Values:**
- `1m` - 1 minute
- `5m` - 5 minutes
- `15m` - 15 minutes
- `1h` - 1 hour
- `4h` - 4 hours
- `24h` - 24 hours

**Asset Type Values:**
- `all` - All assets
- `crypto` - Cryptocurrencies only
- `stock` - Stocks only
- `etf` - ETFs only

**Response:**
```json
{
  "data": {
    "timeframe": "1h",
    "gainers": [
      {
        "symbol": "SOL",
        "price": 150.0,
        "changePercent": 8.5,
        "volume24h": 5000000000
      }
    ],
    "losers": [
      {
        "symbol": "XRP",
        "price": 0.50,
        "changePercent": -5.2,
        "volume24h": 2000000000
      }
    ],
    "timestamp": 1700000000
  },
  "meta": {
    "cached": false
  }
}
```

---

## Trading Signals

### GET /api/signals/:symbol

Get all trading signals for a symbol.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `symbol` | string | Asset symbol (e.g., `btc`) |

**Query Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `timeframe` | string | `day_trading` | Trading style timeframe |

**Timeframe Values:**
- `scalping` - Very short-term (minutes)
- `day_trading` - Intraday (hours)
- `swing_trading` - Short-term (days)
- `position_trading` - Long-term (weeks/months)

**Response:**
```json
{
  "data": {
    "symbol": "BTC",
    "timeframe": "day_trading",
    "signals": [
      {
        "indicator": "rsi",
        "value": 65.5,
        "signal": "neutral",
        "strength": 0.3,
        "explanation": "RSI at 65.5 indicates slightly overbought conditions"
      },
      {
        "indicator": "macd",
        "value": 150.0,
        "signal": "buy",
        "strength": 0.7,
        "explanation": "MACD crossed above signal line"
      }
    ],
    "consensus": {
      "signal": "buy",
      "strength": 0.55,
      "bullishCount": 8,
      "bearishCount": 3,
      "neutralCount": 2
    },
    "lastUpdated": 1700000000000
  },
  "meta": {
    "cached": false
  }
}
```

### POST /api/signals/:symbol/generate

Generate fresh predictions for a symbol (bypasses cache).

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `symbol` | string | Asset symbol |

**Query Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `timeframe` | string | `day_trading` | Trading style timeframe |

**Response:** Same as GET /api/signals/:symbol

### GET /api/signals/:symbol/recommendation

Get accuracy-weighted recommendation for a symbol.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `symbol` | string | Asset symbol |

**Query Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `timeframe` | string | `day_trading` | Trading style timeframe |

**Response:**
```json
{
  "data": {
    "symbol": "BTC",
    "recommendation": "buy",
    "confidence": 0.72,
    "weightedScore": 0.65,
    "topIndicators": [
      {
        "indicator": "macd",
        "signal": "buy",
        "accuracy": 0.68
      }
    ],
    "riskLevel": "medium",
    "timestamp": 1700000000000
  },
  "meta": {
    "cached": false
  }
}
```

### GET /api/signals/:symbol/accuracy

Get accuracy statistics for a specific symbol.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `symbol` | string | Asset symbol |

**Response:**
```json
{
  "data": {
    "symbol": "BTC",
    "accuracies": [
      {
        "indicator": "rsi",
        "totalPredictions": 1000,
        "correctPredictions": 620,
        "accuracy": 0.62,
        "avgProfit": 2.5
      },
      {
        "indicator": "macd",
        "totalPredictions": 1000,
        "correctPredictions": 680,
        "accuracy": 0.68,
        "avgProfit": 3.2
      }
    ],
    "timestamp": 1700000000000
  },
  "meta": {
    "cached": false
  }
}
```

### GET /api/signals/:symbol/predictions

Get predictions for a symbol with optional filtering.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `symbol` | string | Asset symbol |

**Query Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `status` | string | `all` | Filter by status |
| `limit` | integer | 50 | Number of results (max 500) |

**Status Values:**
- `all` - All predictions
- `validated` - Predictions with outcomes
- `pending` - Predictions awaiting validation

**Response:**
```json
{
  "data": {
    "symbol": "BTC",
    "predictions": [
      {
        "id": "pred_123",
        "indicator": "rsi",
        "signal": "buy",
        "priceAtPrediction": 50000.0,
        "predictedAt": 1700000000000,
        "outcome5m": { "correct": true, "priceChange": 0.5 },
        "outcome1h": { "correct": true, "priceChange": 1.2 },
        "outcome4h": null,
        "outcome24h": null
      }
    ],
    "timestamp": 1700000000000
  },
  "meta": {
    "cached": false
  }
}
```

### GET /api/signals/accuracy/:indicator

Get global accuracy statistics for a specific indicator.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `indicator` | string | Indicator name (e.g., `rsi`, `macd`) |

**Response:**
```json
{
  "data": [
    {
      "symbol": "BTC",
      "indicator": "rsi",
      "totalPredictions": 5000,
      "correctPredictions": 3100,
      "accuracy": 0.62
    },
    {
      "symbol": "ETH",
      "indicator": "rsi",
      "totalPredictions": 4500,
      "correctPredictions": 2790,
      "accuracy": 0.62
    }
  ],
  "meta": {
    "cached": false
  }
}
```

---

## Order Book

### GET /api/orderbook/:symbol

Get aggregated order book data from multiple exchanges.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `symbol` | string | Asset symbol (e.g., `btc`) |

**Query Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `depth` | integer | 50 | Number of depth levels (max 100) |

**Response:**
```json
{
  "data": {
    "symbol": "btc",
    "bids": [
      { "price": 49990.0, "amount": 1.5, "total": 74985.0 },
      { "price": 49985.0, "amount": 2.0, "total": 99970.0 }
    ],
    "asks": [
      { "price": 50010.0, "amount": 1.2, "total": 60012.0 },
      { "price": 50015.0, "amount": 1.8, "total": 90027.0 }
    ],
    "spread": 20.0,
    "spreadPercent": 0.04,
    "exchanges": ["binance", "coinbase", "kraken"],
    "timestamp": 1700000000000
  }
}
```

---

## Peer Mesh

### GET /api/peers

Get the current peer mesh status.

**Response:**
```json
{
  "data": {
    "serverId": "us-east",
    "serverRegion": "US East",
    "peers": [
      {
        "id": "eu-west",
        "region": "EU West",
        "status": "connected",
        "latencyMs": 85.5,
        "avgLatencyMs": 88.2,
        "minLatencyMs": 75.0,
        "maxLatencyMs": 120.0,
        "pingCount": 1000,
        "failedPings": 5,
        "uptimePercent": 99.5,
        "lastPingAt": 1700000000000,
        "lastAttemptAt": 1700000000000
      }
    ],
    "connectedCount": 2,
    "totalPeers": 3,
    "timestamp": 1700000000000
  }
}
```

### GET /api/peers/:peer_id

Get a specific peer's status.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `peer_id` | string | Peer server ID |

**Response:**
```json
{
  "id": "eu-west",
  "region": "EU West",
  "status": "connected",
  "latencyMs": 85.5,
  "avgLatencyMs": 88.2,
  "minLatencyMs": 75.0,
  "maxLatencyMs": 120.0,
  "pingCount": 1000,
  "failedPings": 5,
  "uptimePercent": 99.5,
  "lastPingAt": 1700000000000,
  "lastAttemptAt": 1700000000000
}
```

### GET /api/mesh/servers

Get all mesh servers for frontend discovery.

**Response:**
```json
{
  "data": {
    "selfId": "us-east",
    "selfRegion": "US East",
    "selfApiUrl": "https://us.example.com",
    "selfWsUrl": "wss://us.example.com/ws",
    "servers": [
      {
        "id": "us-east",
        "region": "US East",
        "apiUrl": "https://us.example.com",
        "wsUrl": "wss://us.example.com/ws",
        "status": "online",
        "latencyMs": 0
      },
      {
        "id": "eu-west",
        "region": "EU West",
        "apiUrl": "https://eu.example.com",
        "wsUrl": "wss://eu.example.com/ws",
        "status": "online",
        "latencyMs": 85.0
      }
    ],
    "meshKeyHash": "abcd1234",
    "timestamp": 1700000000000
  }
}
```

---

## Error Handling

All endpoints return errors in a consistent format:

**400 Bad Request:**
```json
{
  "error": "Invalid parameter: range must be one of: 1h, 4h, 1d, 1w, 1m"
}
```

**401 Unauthorized:**
```json
{
  "error": "Unauthorized: missing or invalid token"
}
```

**404 Not Found:**
```json
{
  "error": "Asset not found: unknown_symbol"
}
```

**500 Internal Server Error:**
```json
{
  "error": "Internal server error"
}
```

---

## Rate Limiting

The API does not currently implement rate limiting, but excessive requests may be throttled by upstream providers (CoinMarketCap, exchanges, etc.).

Recommended best practices:
- Cache responses where appropriate
- Use WebSocket for real-time data instead of polling
- Batch requests when possible (e.g., seed/batch)
