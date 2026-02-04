# Haunt Trading API Reference

> Complete REST API documentation for the Paper Trading system, including portfolios, orders, positions, strategies, options, backtesting, and margin trading.

**Base URL:** `http://localhost:3000`

---

## Table of Contents

- [Overview](#overview)
- [Portfolios](#portfolios)
- [Orders](#orders)
- [Positions](#positions)
- [Trades](#trades)
- [Strategies](#strategies)
- [Backtesting](#backtesting)
- [Options Trading](#options-trading)
- [Margin & Liquidation](#margin--liquidation)
- [WebSocket Subscriptions](#websocket-subscriptions)
- [Type Definitions](#type-definitions)
- [Error Codes](#error-codes)

---

## Overview

The Haunt Paper Trading API provides a complete simulated trading environment for:

- **Spot Trading**: Crypto, stocks, ETFs with real-time prices
- **Perpetual Futures**: Up to 100x leverage with funding rates
- **Options**: Black-Scholes pricing, Greeks, multi-leg strategies
- **Auto-Trading**: Rule-based automated strategies with 13 technical indicators
- **Backtesting**: Historical strategy testing with Monte Carlo simulation

### Authentication

All trading endpoints require authentication via Bearer token:
```
Authorization: Bearer <sessionToken>
```

### Starting Balance

New portfolios start with **$5,000,000 USD** in simulated funds.

---

## Portfolios

### GET /api/trading/portfolios

List all portfolios for the authenticated user.

**Query Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `user_id` | string | Yes | User's public key |

**Response:**
```json
{
  "data": [
    {
      "id": "portfolio-uuid",
      "userId": "0x1234...",
      "name": "Main Portfolio",
      "description": "My trading portfolio",
      "baseCurrency": "USD",
      "startingBalance": 5000000.0,
      "cashBalance": 4500000.0,
      "marginUsed": 250000.0,
      "marginAvailable": 4250000.0,
      "unrealizedPnl": 15000.0,
      "realizedPnl": 5000.0,
      "totalValue": 4515000.0,
      "costBasisMethod": "fifo",
      "riskSettings": {
        "maxPositionSizePct": 0.25,
        "dailyLossLimitPct": 0.10,
        "maxOpenPositions": 20,
        "riskPerTradePct": 0.02,
        "portfolioStopPct": 0.25
      },
      "isCompetition": false,
      "createdAt": 1700000000000,
      "updatedAt": 1700000000000
    }
  ]
}
```

### POST /api/trading/portfolios

Create a new portfolio.

**Request Body:**
```json
{
  "userId": "0x1234...",
  "name": "My Portfolio",
  "description": "Optional description",
  "riskSettings": {
    "maxPositionSizePct": 0.25,
    "dailyLossLimitPct": 0.10,
    "maxOpenPositions": 20,
    "riskPerTradePct": 0.02,
    "portfolioStopPct": 0.25
  }
}
```

**Response:** Returns the created Portfolio object.

### GET /api/trading/portfolios/:id

Get portfolio details by ID.

**Response:** Returns the Portfolio object.

### GET /api/trading/portfolios/:id/summary

Get portfolio summary with performance metrics.

**Response:**
```json
{
  "data": {
    "portfolioId": "portfolio-uuid",
    "totalValue": 5150000.0,
    "cashBalance": 4500000.0,
    "unrealizedPnl": 15000.0,
    "realizedPnl": 5000.0,
    "totalReturnPct": 3.0,
    "marginUsed": 250000.0,
    "marginAvailable": 4250000.0,
    "marginLevel": 1800.0,
    "openPositions": 3,
    "openOrders": 5
  }
}
```

### PUT /api/trading/portfolios/:id

Update portfolio risk settings.

**Request Body:**
```json
{
  "maxPositionSizePct": 0.30,
  "dailyLossLimitPct": 0.15,
  "maxOpenPositions": 25,
  "riskPerTradePct": 0.03,
  "portfolioStopPct": 0.30
}
```

**Response:** Returns updated Portfolio object.

### POST /api/trading/portfolios/:id/reset

Reset portfolio to starting balance, closing all positions and cancelling all orders.

**Response:** Returns reset Portfolio object.

### DELETE /api/trading/portfolios/:id

Delete a portfolio and all associated data.

**Response:**
```json
{
  "data": {
    "deleted": true,
    "id": "portfolio-uuid"
  }
}
```

---

## Orders

### GET /api/trading/orders

List orders for a portfolio.

**Query Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `portfolio_id` | string | required | Portfolio ID |
| `status` | string | all | Filter by status: `open`, `filled`, `cancelled` |
| `limit` | integer | 100 | Maximum results |

**Response:**
```json
{
  "data": [
    {
      "id": "order-uuid",
      "portfolioId": "portfolio-uuid",
      "symbol": "BTC",
      "assetClass": "crypto_spot",
      "side": "buy",
      "orderType": "limit",
      "quantity": 1.5,
      "filledQuantity": 0.0,
      "price": 50000.0,
      "stopPrice": null,
      "trailAmount": null,
      "trailPercent": null,
      "timeInForce": "gtc",
      "status": "open",
      "linkedOrderId": null,
      "bracketId": null,
      "leverage": 1.0,
      "fills": [],
      "avgFillPrice": null,
      "totalFees": 0.0,
      "clientOrderId": "my-order-123",
      "createdAt": 1700000000000,
      "updatedAt": 1700000000000,
      "expiresAt": null
    }
  ]
}
```

### POST /api/trading/orders

Place a new order.

**Request Body:**
```json
{
  "portfolioId": "portfolio-uuid",
  "symbol": "BTC",
  "assetClass": "crypto_spot",
  "side": "buy",
  "orderType": "limit",
  "quantity": 1.5,
  "price": 50000.0,
  "stopPrice": null,
  "trailAmount": null,
  "trailPercent": null,
  "timeInForce": "gtc",
  "leverage": 1.0,
  "stopLoss": 48000.0,
  "takeProfit": 55000.0,
  "clientOrderId": "my-order-123"
}
```

**Order Types:**
- `market` - Execute immediately at best price
- `limit` - Execute at specified price or better
- `stop_loss` - Trigger sell when price drops to threshold
- `take_profit` - Trigger sell when price reaches target
- `stop_limit` - Stop that becomes limit order when triggered
- `trailing_stop` - Dynamic stop that follows price

**Time in Force:**
- `gtc` - Good Till Cancelled (default)
- `gtd` - Good Till Date (requires `expiresAt`)
- `fok` - Fill or Kill (entire order must fill immediately)
- `ioc` - Immediate or Cancel (fill what's available, cancel rest)

**Asset Classes:**
- `crypto_spot` - Cryptocurrency spot trading (max 10x leverage)
- `stock` - Stocks (max 4x leverage)
- `etf` - ETFs (max 4x leverage)
- `perp` - Perpetual futures (max 100x leverage)
- `option` - Options contracts (no leverage)
- `forex` - Foreign exchange (max 50x leverage)

**Response:** Returns created Order object.

### GET /api/trading/orders/:id

Get order details by ID.

**Response:** Returns Order object.

### DELETE /api/trading/orders/:id

Cancel an order.

**Response:** Returns cancelled Order object with status `cancelled`.

---

## Positions

### GET /api/trading/positions

List open positions for a portfolio.

**Query Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `portfolio_id` | string | Yes | Portfolio ID |

**Response:**
```json
{
  "data": [
    {
      "id": "position-uuid",
      "portfolioId": "portfolio-uuid",
      "symbol": "BTC",
      "assetClass": "crypto_spot",
      "side": "long",
      "quantity": 2.5,
      "entryPrice": 50000.0,
      "currentPrice": 51000.0,
      "unrealizedPnl": 2500.0,
      "unrealizedPnlPct": 2.0,
      "realizedPnl": 0.0,
      "marginUsed": 12750.0,
      "leverage": 10.0,
      "marginMode": "isolated",
      "liquidationPrice": 45500.0,
      "stopLoss": 48000.0,
      "takeProfit": 55000.0,
      "fundingPayments": -15.0,
      "createdAt": 1700000000000,
      "updatedAt": 1700000000000
    }
  ]
}
```

### GET /api/trading/positions/:id

Get position details by ID.

**Response:** Returns Position object.

### PUT /api/trading/positions/:id

Modify position stop loss and take profit.

**Request Body:**
```json
{
  "stopLoss": 47000.0,
  "takeProfit": 56000.0
}
```

**Response:** Returns updated Position object.

### DELETE /api/trading/positions/:id

Close a position at market price.

**Query Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `price` | float | Yes | Current market price for execution |

**Response:** Returns Trade object with realized P&L.

---

## Trades

### GET /api/trading/trades

List trade history for a portfolio.

**Query Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `portfolio_id` | string | required | Portfolio ID |
| `limit` | integer | 100 | Maximum results |

**Response:**
```json
{
  "data": [
    {
      "id": "trade-uuid",
      "orderId": "order-uuid",
      "portfolioId": "portfolio-uuid",
      "positionId": "position-uuid",
      "symbol": "BTC",
      "assetClass": "crypto_spot",
      "side": "sell",
      "quantity": 1.0,
      "price": 52000.0,
      "fee": 52.0,
      "slippage": 5.2,
      "realizedPnl": 1947.8,
      "executedAt": 1700000000000
    }
  ]
}
```

---

## Strategies

Auto-trading strategies allow rule-based automated trading using technical indicators.

### POST /api/trading/strategies

Create a new trading strategy.

**Request Body:**
```json
{
  "portfolioId": "portfolio-uuid",
  "name": "RSI Momentum Strategy",
  "description": "Buy when RSI oversold, sell when overbought",
  "symbols": ["BTC", "ETH"],
  "assetClass": "crypto_spot",
  "rules": [
    {
      "name": "Buy Signal",
      "conditions": [
        {
          "indicator": "rsi",
          "period": 14,
          "operator": "less_than",
          "value": 30.0
        }
      ],
      "conditionOperator": "and",
      "action": {
        "actionType": "market_buy",
        "sizeType": "portfolio_percent",
        "sizeValue": 5.0,
        "stopLossPct": 5.0,
        "takeProfitPct": 10.0,
        "leverage": 1.0
      },
      "enabled": true,
      "priority": 0
    },
    {
      "name": "Sell Signal",
      "conditions": [
        {
          "indicator": "rsi",
          "period": 14,
          "operator": "greater_than",
          "value": 70.0
        }
      ],
      "conditionOperator": "and",
      "action": {
        "actionType": "close_position",
        "sizeType": "portfolio_percent",
        "sizeValue": 100.0,
        "leverage": 1.0
      },
      "enabled": true,
      "priority": 1
    }
  ],
  "cooldownSeconds": 3600,
  "maxPositions": 3,
  "maxPositionSizePct": 0.10
}
```

**Available Indicators:**
- `rsi` - Relative Strength Index
- `macd` - Moving Average Convergence Divergence
- `ema` - Exponential Moving Average
- `sma` - Simple Moving Average
- `bollinger` - Bollinger Bands
- `atr` - Average True Range
- `adx` - Average Directional Index
- `stochastic` - Stochastic Oscillator
- `obv` - On-Balance Volume
- `vwap` - Volume Weighted Average Price
- `cci` - Commodity Channel Index
- `mfi` - Money Flow Index
- `price` - Raw price value

**Comparison Operators:**
- `less_than` / `lt`
- `less_than_or_equal` / `lte`
- `greater_than` / `gt`
- `greater_than_or_equal` / `gte`
- `equal` / `eq`
- `not_equal` / `ne`
- `crosses_above` - Indicator crosses above value
- `crosses_below` - Indicator crosses below value

**Action Types:**
- `market_buy` - Place market buy order
- `market_sell` - Place market sell order
- `limit_buy` - Place limit buy order
- `limit_sell` - Place limit sell order
- `close_position` - Close existing position
- `close_partial` - Close partial position

**Position Size Types:**
- `fixed_amount` - Fixed dollar amount
- `portfolio_percent` - Percentage of portfolio
- `risk_percent` - Risk-based sizing with stop loss
- `fixed_units` - Fixed number of units/shares

**Response:** Returns created TradingStrategy object.

### GET /api/trading/strategies

List strategies for a portfolio.

**Query Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `portfolio_id` | string | Yes | Portfolio ID |

**Response:**
```json
{
  "data": [
    {
      "id": "strategy-uuid",
      "portfolioId": "portfolio-uuid",
      "name": "RSI Momentum Strategy",
      "description": "Buy when RSI oversold",
      "symbols": ["BTC", "ETH"],
      "assetClass": "crypto_spot",
      "rules": [...],
      "status": "active",
      "cooldownSeconds": 3600,
      "maxPositions": 3,
      "maxPositionSizePct": 0.10,
      "lastTradeAt": 1700000000000,
      "totalTrades": 42,
      "winningTrades": 28,
      "losingTrades": 14,
      "realizedPnl": 15000.0,
      "createdAt": 1700000000000,
      "updatedAt": 1700000000000
    }
  ]
}
```

### PUT /api/trading/strategies/:id/activate

Activate a strategy to start auto-trading.

**Response:** Returns updated strategy with `status: "active"`.

### PUT /api/trading/strategies/:id/pause

Pause a strategy (no new trades).

**Response:** Returns updated strategy with `status: "paused"`.

### DELETE /api/trading/strategies/:id

Delete a strategy.

**Response:**
```json
{
  "data": {
    "deleted": true,
    "id": "strategy-uuid"
  }
}
```

---

## Backtesting

Test strategies against historical data.

### POST /api/trading/backtests

Run a new backtest.

**Request Body:**
```json
{
  "strategyId": "strategy-uuid",
  "symbols": ["BTC", "ETH"],
  "startTime": 1672531200000,
  "endTime": 1704067200000,
  "initialBalance": 100000.0,
  "commissionRate": 0.001,
  "slippagePct": 0.0005,
  "useOhlc": true,
  "candleInterval": 300,
  "enableMargin": false,
  "monteCarloRuns": 1000
}
```

**Response:**
```json
{
  "data": {
    "id": "backtest-uuid",
    "strategyId": "strategy-uuid",
    "status": "pending",
    "config": {...},
    "createdAt": 1700000000000
  }
}
```

### GET /api/trading/backtests/:id

Get backtest result.

**Response:**
```json
{
  "data": {
    "id": "backtest-uuid",
    "strategyId": "strategy-uuid",
    "status": "completed",
    "config": {...},
    "metrics": {
      "totalReturnPct": 45.5,
      "annualizedReturnPct": 125.2,
      "totalPnl": 45500.0,
      "grossProfit": 62000.0,
      "grossLoss": 16500.0,
      "profitFactor": 3.76,
      "maxDrawdownPct": 12.5,
      "maxDrawdown": 14200.0,
      "avgDrawdownPct": 4.2,
      "sharpeRatio": 2.15,
      "sortinoRatio": 3.42,
      "calmarRatio": 10.0,
      "dailyVolatility": 0.025,
      "totalTrades": 156,
      "winningTrades": 98,
      "losingTrades": 58,
      "winRatePct": 62.8,
      "avgTradePnl": 291.67,
      "avgWin": 632.65,
      "avgLoss": 284.48,
      "largestWin": 5200.0,
      "largestLoss": 1850.0,
      "avgTradeDurationMs": 14400000,
      "expectancy": 214.5,
      "maxConsecutiveWins": 12,
      "maxConsecutiveLosses": 5,
      "currentStreak": 3,
      "timeInMarketPct": 68.5,
      "totalCommission": 312.0,
      "totalSlippage": 156.0
    },
    "trades": [
      {
        "id": "trade-uuid",
        "symbol": "BTC",
        "side": "buy",
        "entryPrice": 42000.0,
        "exitPrice": 44500.0,
        "quantity": 0.5,
        "entryTime": 1672531200000,
        "exitTime": 1672617600000,
        "pnl": 1247.5,
        "pnlPct": 5.9,
        "commission": 2.5,
        "entryRuleId": "rule-uuid-1",
        "exitRuleId": "rule-uuid-2",
        "isWinner": true,
        "maxFavorableExcursion": 1350.0,
        "maxAdverseExcursion": -150.0
      }
    ],
    "equityCurve": [
      {
        "timestamp": 1672531200000,
        "equity": 100000.0,
        "cash": 100000.0,
        "positionsValue": 0.0,
        "realizedPnl": 0.0,
        "unrealizedPnl": 0.0,
        "drawdownPct": 0.0
      }
    ],
    "buyAndHold": {
      "bnhReturnPct": 32.5,
      "outperformancePct": 13.0,
      "strategyMaxDd": 12.5,
      "bnhMaxDd": 25.8,
      "strategySharpe": 2.15,
      "bnhSharpe": 1.12
    },
    "monteCarlo": {
      "numRuns": 1000,
      "returnP5": 18.2,
      "returnP25": 32.5,
      "returnP50": 45.5,
      "returnP75": 58.2,
      "returnP95": 78.5,
      "maxDdP5": 8.5,
      "maxDdP50": 12.5,
      "maxDdP95": 22.5,
      "probabilityOfProfit": 0.95,
      "probabilityOfRuin": 0.02
    },
    "finalBalance": 145500.0,
    "createdAt": 1700000000000,
    "startedAt": 1700000001000,
    "completedAt": 1700000005000,
    "executionTimeMs": 4000
  }
}
```

### GET /api/trading/strategies/:id/backtests

List all backtests for a strategy.

**Response:** Returns array of BacktestResult summaries.

---

## Options Trading

### GET /api/trading/options/chains/:symbol

Get options chain for an underlying symbol.

**Query Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `expiration` | integer | - | Filter by expiration timestamp (ms) |

**Response:**
```json
{
  "data": {
    "underlyingSymbol": "AAPL",
    "underlyingPrice": 185.50,
    "expiration": 1705622400000,
    "calls": [
      {
        "contractSymbol": "AAPL240119C00180000",
        "underlyingSymbol": "AAPL",
        "optionType": "call",
        "strike": 180.0,
        "expiration": 1705622400000,
        "style": "american",
        "bid": 7.50,
        "ask": 7.75,
        "last": 7.65,
        "volume": 1523,
        "openInterest": 15420,
        "impliedVolatility": 0.25,
        "greeks": {
          "delta": 0.72,
          "gamma": 0.045,
          "theta": -0.15,
          "vega": 0.18,
          "rho": 0.08
        },
        "multiplier": 100
      }
    ],
    "puts": [...],
    "timestamp": 1700000000000
  }
}
```

### POST /api/trading/options/positions

Open an options position.

**Request Body:**
```json
{
  "portfolioId": "portfolio-uuid",
  "contractSymbol": "AAPL240119C00180000",
  "contracts": 10,
  "premium": 7.65
}
```

**Response:** Returns created OptionPosition object.

### GET /api/trading/options/positions

List options positions.

**Query Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `portfolio_id` | string | Yes | Portfolio ID |

**Response:**
```json
{
  "data": [
    {
      "id": "position-uuid",
      "portfolioId": "portfolio-uuid",
      "contractSymbol": "AAPL240119C00180000",
      "underlyingSymbol": "AAPL",
      "optionType": "call",
      "strike": 180.0,
      "expiration": 1705622400000,
      "style": "american",
      "contracts": 10,
      "multiplier": 100,
      "entryPremium": 7.65,
      "currentPremium": 8.25,
      "underlyingPrice": 186.50,
      "unrealizedPnl": 600.0,
      "realizedPnl": 0.0,
      "greeks": {
        "delta": 0.75,
        "gamma": 0.042,
        "theta": -0.18,
        "vega": 0.16,
        "rho": 0.09
      },
      "entryIv": 0.25,
      "currentIv": 0.27,
      "createdAt": 1700000000000,
      "updatedAt": 1700000000000
    }
  ]
}
```

### POST /api/trading/options/strategies

Create a multi-leg options strategy.

**Request Body:**
```json
{
  "portfolioId": "portfolio-uuid",
  "strategyType": "iron_condor",
  "underlyingSymbol": "AAPL",
  "legs": [
    {
      "contractSymbol": "AAPL240119P00170000",
      "contracts": -1,
      "premium": 2.50
    },
    {
      "contractSymbol": "AAPL240119P00165000",
      "contracts": 1,
      "premium": 1.50
    },
    {
      "contractSymbol": "AAPL240119C00195000",
      "contracts": -1,
      "premium": 2.25
    },
    {
      "contractSymbol": "AAPL240119C00200000",
      "contracts": 1,
      "premium": 1.25
    }
  ]
}
```

**Strategy Types:**
- `single` - Single call or put
- `covered_call` - Long stock + short call
- `protective_put` - Long stock + long put
- `bull_call_spread` - Buy call, sell higher strike call
- `bear_put_spread` - Buy put, sell lower strike put
- `bear_call_spread` - Sell call, buy higher strike call
- `bull_put_spread` - Sell put, buy lower strike put
- `straddle` - Long call + long put at same strike
- `strangle` - Long call + long put at different strikes
- `iron_condor` - Short call spread + short put spread
- `iron_butterfly` - Short straddle + long wings
- `calendar_spread` - Same strike, different expirations
- `custom` - Custom multi-leg

**Response:** Returns created OptionStrategy object.

---

## Margin & Liquidation

### Leverage Tiers (Perpetual Futures)

| Position Size | Max Leverage | Initial Margin | Maintenance Margin |
|--------------|--------------|----------------|-------------------|
| < $50,000 | 100x | 1% | 0.5% |
| < $250,000 | 50x | 2% | 1% |
| < $1,000,000 | 20x | 5% | 2.5% |
| < $5,000,000 | 10x | 10% | 5% |
| > $5,000,000 | 5x | 20% | 10% |

### Margin Levels & Warnings

| Warning Level | Margin Level | Action |
|--------------|--------------|--------|
| Warning80 | 125% | First warning notification |
| Warning90 | 111% | Second warning notification |
| Warning95 | 105% | Final warning notification |
| Liquidation | 100% | Position liquidated |

### GET /api/trading/portfolios/:id/margin

Get portfolio margin status.

**Response:**
```json
{
  "data": {
    "portfolioId": "portfolio-uuid",
    "marginUsed": 250000.0,
    "marginAvailable": 4250000.0,
    "marginLevel": 1800.0,
    "warningLevel": null,
    "positions": [
      {
        "positionId": "position-uuid",
        "symbol": "BTC-PERP",
        "marginUsed": 125000.0,
        "maintenanceMarginRequired": 6250.0,
        "liquidationPrice": 45500.0,
        "leverageTier": {
          "maxPositionSize": 250000.0,
          "maxLeverage": 50.0,
          "initialMarginRate": 0.02,
          "maintenanceMarginRate": 0.01
        }
      }
    ]
  }
}
```

### GET /api/trading/portfolios/:id/funding

Get funding payment history.

**Response:**
```json
{
  "data": [
    {
      "id": "payment-uuid",
      "positionId": "position-uuid",
      "portfolioId": "portfolio-uuid",
      "symbol": "BTC-PERP",
      "positionSize": 125000.0,
      "side": "long",
      "fundingRate": 0.0001,
      "payment": 12.50,
      "paidAt": 1700000000000
    }
  ]
}
```

### GET /api/trading/portfolios/:id/liquidations

Get liquidation history.

**Response:**
```json
{
  "data": [
    {
      "id": "liquidation-uuid",
      "positionId": "position-uuid",
      "portfolioId": "portfolio-uuid",
      "symbol": "BTC-PERP",
      "quantity": 2.5,
      "liquidationPrice": 45500.0,
      "markPrice": 45450.0,
      "loss": 11250.0,
      "liquidationFee": 568.75,
      "isPartial": false,
      "liquidatedAt": 1700000000000
    }
  ]
}
```

---

## WebSocket Subscriptions

Connect to `ws://localhost:3000/ws` and subscribe to trading updates.

### Subscribe to Portfolio Updates

```json
{
  "type": "subscribe",
  "channel": "trading:portfolio",
  "portfolioId": "portfolio-uuid"
}
```

### Portfolio Update Events

```json
{
  "type": "portfolio_update",
  "portfolioId": "portfolio-uuid",
  "data": {
    "cashBalance": 4500000.0,
    "marginUsed": 250000.0,
    "unrealizedPnl": 15000.0,
    "totalValue": 4515000.0
  }
}
```

### Order Update Events

```json
{
  "type": "order_update",
  "portfolioId": "portfolio-uuid",
  "order": {
    "id": "order-uuid",
    "status": "filled",
    "filledQuantity": 1.5,
    "avgFillPrice": 50000.0
  }
}
```

### Position Update Events

```json
{
  "type": "position_update",
  "portfolioId": "portfolio-uuid",
  "position": {
    "id": "position-uuid",
    "currentPrice": 51000.0,
    "unrealizedPnl": 2500.0
  }
}
```

### Strategy Signal Events

```json
{
  "type": "strategy_signal",
  "portfolioId": "portfolio-uuid",
  "signal": {
    "strategyId": "strategy-uuid",
    "ruleId": "rule-uuid",
    "symbol": "BTC",
    "action": {
      "actionType": "market_buy",
      "sizeType": "portfolio_percent",
      "sizeValue": 5.0
    },
    "strength": 0.85,
    "generatedAt": 1700000000000
  }
}
```

### Margin Warning Events

```json
{
  "type": "margin_warning",
  "portfolioId": "portfolio-uuid",
  "level": "warning90",
  "marginLevel": 108.5,
  "positions": [
    {
      "positionId": "position-uuid",
      "symbol": "BTC-PERP",
      "liquidationPrice": 45500.0
    }
  ]
}
```

### Liquidation Events

```json
{
  "type": "liquidation",
  "portfolioId": "portfolio-uuid",
  "liquidation": {
    "positionId": "position-uuid",
    "symbol": "BTC-PERP",
    "quantity": 2.5,
    "price": 45500.0,
    "loss": 11250.0
  }
}
```

---

## Type Definitions

### AssetClass

```typescript
type AssetClass =
  | "crypto_spot"  // Max 10x leverage
  | "stock"        // Max 4x leverage
  | "etf"          // Max 4x leverage
  | "perp"         // Max 100x leverage
  | "option"       // No leverage (premium-based)
  | "forex"        // Max 50x leverage
```

### OrderSide

```typescript
type OrderSide = "buy" | "sell"
```

### OrderType

```typescript
type OrderType =
  | "market"
  | "limit"
  | "stop_loss"
  | "take_profit"
  | "stop_limit"
  | "trailing_stop"
```

### OrderStatus

```typescript
type OrderStatus =
  | "pending"
  | "open"
  | "partially_filled"
  | "filled"
  | "cancelled"
  | "expired"
  | "rejected"
```

### TimeInForce

```typescript
type TimeInForce =
  | "gtc"  // Good Till Cancelled (default)
  | "gtd"  // Good Till Date
  | "fok"  // Fill or Kill
  | "ioc"  // Immediate or Cancel
```

### PositionSide

```typescript
type PositionSide = "long" | "short"
```

### MarginMode

```typescript
type MarginMode =
  | "isolated"  // Independent margin per position
  | "cross"     // Shared margin across positions
```

### CostBasisMethod

```typescript
type CostBasisMethod =
  | "fifo"     // First In, First Out
  | "lifo"     // Last In, First Out
  | "average"  // Weighted Average Cost
```

### StrategyStatus

```typescript
type StrategyStatus =
  | "active"   // Executing trades
  | "paused"   // Not executing
  | "disabled" // Manual reactivation required
  | "deleted"  // Soft deleted
```

### BacktestStatus

```typescript
type BacktestStatus =
  | "pending"
  | "running"
  | "completed"
  | "failed"
  | "cancelled"
```

### IndicatorType

```typescript
type IndicatorType =
  | "rsi"
  | "macd"
  | "ema"
  | "sma"
  | "bollinger"
  | "atr"
  | "adx"
  | "stochastic"
  | "obv"
  | "vwap"
  | "cci"
  | "mfi"
  | "price"
```

### OptionType

```typescript
type OptionType = "call" | "put"
```

### OptionStyle

```typescript
type OptionStyle = "american" | "european"
```

### Greeks

```typescript
interface Greeks {
  delta: number;   // Price sensitivity (dV/dS)
  gamma: number;   // Rate of delta change (d²V/dS²)
  theta: number;   // Time decay per day (dV/dt)
  vega: number;    // Volatility sensitivity (dV/dσ)
  rho: number;     // Interest rate sensitivity (dV/dr)
}
```

---

## Error Codes

| Code | HTTP Status | Description |
|------|-------------|-------------|
| `PORTFOLIO_NOT_FOUND` | 404 | Portfolio ID does not exist |
| `ORDER_NOT_FOUND` | 404 | Order ID does not exist |
| `POSITION_NOT_FOUND` | 404 | Position ID does not exist |
| `INSUFFICIENT_FUNDS` | 400 | Not enough cash balance |
| `INSUFFICIENT_MARGIN` | 400 | Not enough margin available |
| `POSITION_LIMIT_EXCEEDED` | 400 | Max positions reached |
| `INVALID_ORDER` | 400 | Order parameters invalid |
| `CANNOT_CANCEL_ORDER` | 400 | Order in terminal state |
| `LEVERAGE_EXCEEDED` | 400 | Leverage exceeds tier limit |
| `PORTFOLIO_STOPPED` | 403 | Portfolio hit drawdown limit |
| `DATABASE_ERROR` | 500 | Database operation failed |
| `NO_PRICE_DATA` | 503 | Price data unavailable |

**Error Response Format:**
```json
{
  "error": "Insufficient funds: required $50000.00, available $45000.00",
  "code": "INSUFFICIENT_FUNDS"
}
```

---

## Frontend Implementation Notes

### Real-time Updates

1. Connect to WebSocket and subscribe to portfolio channel immediately after login
2. Listen for `portfolio_update`, `order_update`, `position_update` events
3. Update local state optimistically, then reconcile with WebSocket updates

### Order Flow

1. Validate order parameters client-side before submission
2. Show pending state immediately after POST
3. Update to final state when WebSocket event arrives

### Margin Display

1. Show margin level as a gauge/meter (100% = liquidation)
2. Color code warnings: green (>125%), yellow (111-125%), orange (105-111%), red (<105%)
3. Show liquidation price prominently for leveraged positions

### Backtest Visualization

1. Plot equity curve as line chart
2. Show trades as markers on price chart
3. Display Monte Carlo distribution as histogram
4. Compare strategy vs buy-and-hold as overlaid lines

### Strategy Builder

1. Allow drag-and-drop rule creation
2. Show indicator preview with current values
3. Validate rule logic before saving
4. Provide templates for common strategies (RSI oversold, MACD crossover, etc.)
