# Haunt API Reference

Base URL: `http://localhost:3000`

## Endpoints

### Health Check

```
GET /health
```

**Response:**
```json
{
  "status": "ok",
  "version": "0.1.0"
}
```

---

### Crypto Listings

```
GET /api/crypto/listings
```

**Query Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `start` | integer | 1 | Starting position |
| `limit` | integer | 20 | Number of results (max 100) |
| `sort` | string | `market_cap` | Sort field |
| `sort_dir` | string | `desc` | Sort direction (`asc`/`desc`) |
| `filter` | string | - | Filter type |
| `min_change` | float | - | Minimum 24h change |
| `max_change` | float | - | Maximum 24h change |

**Sort Fields:**
- `market_cap`
- `price`
- `volume_24h`
- `percent_change_1h`
- `percent_change_24h`
- `percent_change_7d`
- `name`

**Filters:**
- `all` - All assets
- `gainers` - 24h change > 0
- `losers` - 24h change < 0
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

---

### Search Assets

```
GET /api/crypto/search?q={query}&limit={limit}
```

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

---

### Get Asset

```
GET /api/crypto/:id
```

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

---

### Get Chart Data

```
GET /api/crypto/:id/chart?range={range}
```

**Query Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `range` | string | `1d` | Time range |

**Ranges:**
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

---

### Trigger Seeding

```
POST /api/crypto/seed
```

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

---

### Batch Seeding

```
POST /api/crypto/seed/batch
```

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
    { "symbol": "eth", "status": "seeded", "message": "Already seeded" }
  ]
}
```

---

### Get Seeding Status

```
GET /api/crypto/seed/status
```

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

## Error Responses

**400 Bad Request:**
```json
{
  "error": "Invalid range: invalid"
}
```

**404 Not Found:**
```json
{
  "error": "Asset 999999 not found"
}
```

**500 Internal Server Error:**
```json
{
  "error": "Internal server error"
}
```
