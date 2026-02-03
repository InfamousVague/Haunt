# Haunt API Documentation

> A high-performance cryptocurrency data aggregation server built with Rust and Axum.

## Overview

Haunt is a real-time cryptocurrency data server that aggregates price data from multiple exchanges, calculates technical indicators, and provides trading signals. It features WebSocket support for live updates and a peer mesh network for distributed data collection.

## Quick Start

### Base URL

```
http://localhost:3000
```

### Health Check

```bash
curl http://localhost:3000/api/health
```

### Get Cryptocurrency Listings

```bash
curl "http://localhost:3000/api/crypto/listings?limit=10"
```

### Get Asset Details

```bash
curl http://localhost:3000/api/crypto/btc
```

## Features

- **Multi-Source Price Aggregation** - Data from 12+ exchanges including Binance, Coinbase, Kraken, and more
- **Technical Indicators** - RSI, MACD, Bollinger Bands, EMA, SMA, Stochastic, and more
- **Trading Signals** - AI-powered buy/sell signals with accuracy tracking
- **Real-time Updates** - WebSocket connections for live price feeds
- **Order Book Aggregation** - Combined order books from multiple exchanges
- **Peer Mesh Network** - Distributed data collection across regions

## API Categories

| Category | Description |
|----------|-------------|
| [Health](/api-reference#health) | Server health and status |
| [Authentication](/api-reference#authentication) | Wallet-based auth with challenge signing |
| [Cryptocurrency](/api-reference#cryptocurrency) | Asset listings, details, charts, and seeding |
| [Market Data](/api-reference#market-data) | Global metrics, fear/greed, movers |
| [Trading Signals](/api-reference#trading-signals) | Technical signals and predictions |
| [Order Book](/api-reference#order-book) | Aggregated order book data |
| [Peer Mesh](/api-reference#peer-mesh) | Distributed network status |

## Postman Collection

Download the complete Postman collection to test all API endpoints:

<a href="haunt-api.postman_collection.json" download class="download-btn">Download Postman Collection</a>

### Import Instructions

1. Open Postman
2. Click **Import** in the top left
3. Drag and drop the downloaded JSON file
4. Set the `baseUrl` variable to your server address
5. For authenticated endpoints, set `authToken` after logging in

## WebSocket

Connect to the WebSocket endpoint for real-time updates:

```javascript
const ws = new WebSocket('ws://localhost:3000/ws');

ws.onopen = () => {
  ws.send(JSON.stringify({
    type: 'subscribe',
    symbols: ['btc', 'eth']
  }));
};

ws.onmessage = (event) => {
  const data = JSON.parse(event.data);
  console.log('Price update:', data);
};
```

See the [WebSocket Protocol](websocket-protocol.md) documentation for full details.
