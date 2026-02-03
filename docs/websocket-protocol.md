# WebSocket Protocol

> Real-time data streaming via WebSocket connections.

## Connection

Connect to the WebSocket endpoint:

```
ws://localhost:3000/ws
```

## Message Format

All messages are JSON-encoded with a `type` field indicating the message type.

---

## Client Messages

### Subscribe

Subscribe to price updates for specific symbols.

```json
{
  "type": "subscribe",
  "symbols": ["btc", "eth", "sol"]
}
```

### Unsubscribe

Unsubscribe from price updates.

```json
{
  "type": "unsubscribe",
  "symbols": ["sol"]
}
```

### Ping

Keep the connection alive.

```json
{
  "type": "ping"
}
```

---

## Server Messages

### Price Update

Sent when a subscribed symbol's price changes.

```json
{
  "type": "price_update",
  "symbol": "btc",
  "price": 67234.56,
  "change_24h": 2.34,
  "volume_24h": 28500000000,
  "timestamp": 1700000000000
}
```

### Order Book Update

Sent when order book data changes (if subscribed to order book channel).

```json
{
  "type": "orderbook_update",
  "symbol": "btc",
  "bids": [
    { "price": 67230.00, "quantity": 1.5 },
    { "price": 67225.00, "quantity": 2.3 }
  ],
  "asks": [
    { "price": 67235.00, "quantity": 0.8 },
    { "price": 67240.00, "quantity": 1.2 }
  ],
  "timestamp": 1700000000000
}
```

### Signal Update

Sent when a trading signal is generated.

```json
{
  "type": "signal_update",
  "symbol": "btc",
  "indicator": "rsi",
  "signal": "buy",
  "value": 28.5,
  "confidence": 0.85,
  "timestamp": 1700000000000
}
```

### Server Status

Periodic server health updates.

```json
{
  "type": "server_status",
  "connected_peers": 5,
  "active_sources": 12,
  "symbols_tracked": 150,
  "uptime_seconds": 86400
}
```

### Pong

Response to ping messages.

```json
{
  "type": "pong",
  "timestamp": 1700000000000
}
```

### Error

Sent when an error occurs.

```json
{
  "type": "error",
  "code": "invalid_symbol",
  "message": "Symbol 'xyz' not found"
}
```

---

## Example Client

### JavaScript

```javascript
const ws = new WebSocket('ws://localhost:3000/ws');

ws.onopen = () => {
  console.log('Connected to Haunt WebSocket');

  // Subscribe to symbols
  ws.send(JSON.stringify({
    type: 'subscribe',
    symbols: ['btc', 'eth', 'sol']
  }));
};

ws.onmessage = (event) => {
  const message = JSON.parse(event.data);

  switch (message.type) {
    case 'price_update':
      console.log(`${message.symbol}: $${message.price}`);
      break;
    case 'signal_update':
      console.log(`Signal: ${message.indicator} ${message.signal}`);
      break;
    case 'error':
      console.error(`Error: ${message.message}`);
      break;
  }
};

ws.onerror = (error) => {
  console.error('WebSocket error:', error);
};

ws.onclose = () => {
  console.log('Disconnected from Haunt WebSocket');
};

// Keep connection alive with periodic pings
setInterval(() => {
  if (ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'ping' }));
  }
}, 30000);
```

### Rust

```rust
use tokio_tungstenite::connect_async;
use futures_util::{StreamExt, SinkExt};
use serde_json::json;

#[tokio::main]
async fn main() {
    let (mut ws, _) = connect_async("ws://localhost:3000/ws")
        .await
        .expect("Failed to connect");

    // Subscribe to symbols
    ws.send(json!({
        "type": "subscribe",
        "symbols": ["btc", "eth"]
    }).to_string().into())
    .await
    .unwrap();

    // Handle messages
    while let Some(msg) = ws.next().await {
        if let Ok(text) = msg.unwrap().into_text() {
            let data: serde_json::Value = serde_json::from_str(&text).unwrap();
            println!("Received: {:?}", data);
        }
    }
}
```

---

## Rate Limits

- Maximum 100 subscriptions per connection
- Maximum 10 messages per second
- Connections are closed after 5 minutes of inactivity (unless ping/pong is active)

## Error Codes

| Code | Description |
|------|-------------|
| `invalid_message` | Malformed JSON or missing required fields |
| `invalid_symbol` | Requested symbol does not exist |
| `rate_limited` | Too many messages sent |
| `subscription_limit` | Maximum subscriptions exceeded |
| `internal_error` | Server-side error occurred |
