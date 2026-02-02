# Haunt Documentation

Haunt is a Rust-based cryptocurrency price aggregation backend that provides real-time prices, historical charts, and market data.

## Features

- Multi-source price aggregation (Coinbase, Binance, CoinGecko, etc.)
- Real-time WebSocket price updates
- Historical OHLC chart data
- Redis-backed persistence
- CoinMarketCap data integration

## Quick Start

```bash
# Set environment variables
export CMC_API_KEY=your_key
export REDIS_URL=redis://localhost:6379

# Run the server
cargo run

# Server starts at http://localhost:3000
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `CMC_API_KEY` | CoinMarketCap API key | Required |
| `COINGECKO_API_KEY` | CoinGecko API key (optional) | - |
| `CRYPTOCOMPARE_API_KEY` | CryptoCompare API key (optional) | - |
| `REDIS_URL` | Redis connection URL | `redis://localhost:6379` |
| `PORT` | Server port | `3000` |

## Project Structure

```
Haunt/
├── src/
│   ├── api/            # REST API endpoints
│   │   ├── crypto.rs   # /api/crypto/* routes
│   │   ├── health.rs   # Health check
│   │   └── market.rs   # Market data
│   ├── services/       # Business logic
│   │   ├── historical.rs   # Historical data seeding
│   │   ├── chart_store.rs  # In-memory chart data
│   │   ├── price_cache.rs  # Price aggregation
│   │   └── multi_source.rs # Exchange coordinator
│   ├── sources/        # Exchange integrations
│   │   ├── binance.rs
│   │   ├── coinbase_ws.rs
│   │   └── ...
│   ├── websocket/      # WebSocket handling
│   │   ├── handler.rs
│   │   └── room_manager.rs
│   └── types/          # Data structures
├── tests/              # Integration tests
└── docs/               # Documentation
```

## Documentation

- [API Reference](./api-reference.md) - REST API documentation
- [WebSocket Protocol](./websocket-protocol.md) - Real-time updates
- [Data Flow](./data-flow.md) - How data flows through the system

## Testing

```bash
# Run all tests
cargo test

# Run specific test file
cargo test --test api_sanity_test
```
