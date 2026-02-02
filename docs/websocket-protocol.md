# Haunt WebSocket Protocol

Connect to the WebSocket endpoint at `ws://localhost:3000/ws`.

## Connection

```javascript
const ws = new WebSocket('ws://localhost:3000/ws');

ws.onopen = () => {
  console.log('Connected to Haunt');
};

ws.onmessage = (event) => {
  const message = JSON.parse(event.data);
  handleMessage(message);
};
```

## Client Messages

### Subscribe

Subscribe to real-time price updates for specific assets.

```json
{
  "type": "subscribe",
  "assets": ["btc", "eth", "sol"]
}
```

**Response:**
```json
{
  "type": "subscribed",
  "assets": ["btc", "eth", "sol"]
}
```

### Unsubscribe

Stop receiving updates for specific assets.

```json
{
  "type": "unsubscribe",
  "assets": ["sol"]
}
```

**Response:**
```json
{
  "type": "unsubscribed",
  "assets": ["sol"]
}
```

## Server Messages

### Price Update

Sent when a subscribed asset's price changes.

```json
{
  "type": "price_update",
  "data": {
    "id": "btc",
    "symbol": "btc",
    "price": 50000.00,
    "previousPrice": 49500.00,
    "change24h": 2.5,
    "volume24h": 50000000000,
    "tradeDirection": "up",
    "source": "coinbase",
    "sources": ["coinbase", "binance", "coingecko"],
    "timestamp": 1704067200000
  }
}
```

**Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Asset identifier |
| `symbol` | string | Asset symbol (lowercase) |
| `price` | float | Current price in USD |
| `previousPrice` | float? | Previous price (for direction) |
| `change24h` | float? | 24-hour percentage change |
| `volume24h` | float? | 24-hour trading volume |
| `tradeDirection` | string? | `"up"` or `"down"` |
| `source` | string | Primary price source |
| `sources` | array | All contributing sources |
| `timestamp` | integer | Unix timestamp in milliseconds |

### Market Update

Sent periodically with global market data.

```json
{
  "type": "market_update",
  "data": {
    "totalMarketCap": 2000000000000,
    "totalVolume24h": 100000000000,
    "btcDominance": 45.5,
    "timestamp": 1704067200000
  }
}
```

### Seeding Progress

Sent when historical data seeding status changes.

```json
{
  "type": "seeding_progress",
  "data": {
    "symbol": "btc",
    "status": "in_progress",
    "progress": 50,
    "points": 500,
    "message": "Fetching from CoinGecko..."
  }
}
```

**Status Values:**
- `in_progress` - Actively fetching data
- `complete` - Finished successfully
- `failed` - Fetching failed

**Progress:** 0-100 percentage complete

### Error

Sent when an error occurs processing a client message.

```json
{
  "type": "error",
  "error": "Invalid message format"
}
```

## Price Sources

| Source | Type | Description |
|--------|------|-------------|
| `coinbase` | WebSocket | Real-time trades |
| `binance` | WebSocket | Real-time trades |
| `kraken` | REST | Periodic polling |
| `kucoin` | WebSocket | Real-time trades |
| `okx` | WebSocket | Real-time trades |
| `huobi` | WebSocket | Real-time trades |
| `coingecko` | REST | Aggregated prices |
| `coinmarketcap` | REST | Market data |
| `cryptocompare` | REST | Historical data |

## Trade Direction

The `tradeDirection` field indicates short-term price movement:

- `"up"` - Price increased from previous update
- `"down"` - Price decreased from previous update
- `null` - No previous price available

## Example Usage

```javascript
const ws = new WebSocket('ws://localhost:3000/ws');

ws.onopen = () => {
  // Subscribe to Bitcoin and Ethereum
  ws.send(JSON.stringify({
    type: 'subscribe',
    assets: ['btc', 'eth']
  }));
};

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);

  switch (msg.type) {
    case 'price_update':
      updatePrice(msg.data);
      break;
    case 'market_update':
      updateMarket(msg.data);
      break;
    case 'seeding_progress':
      updateSeedingStatus(msg.data);
      break;
    case 'subscribed':
      console.log('Subscribed to:', msg.assets);
      break;
    case 'error':
      console.error('WebSocket error:', msg.error);
      break;
  }
};

function updatePrice(data) {
  console.log(`${data.symbol}: $${data.price} (${data.tradeDirection})`);
}
```

## Reconnection

The server does not send heartbeat messages. Clients should implement reconnection logic:

```javascript
function connect() {
  const ws = new WebSocket('ws://localhost:3000/ws');

  ws.onclose = () => {
    console.log('Disconnected, reconnecting in 5s...');
    setTimeout(connect, 5000);
  };

  ws.onerror = (err) => {
    console.error('WebSocket error:', err);
    ws.close();
  };

  return ws;
}
```
