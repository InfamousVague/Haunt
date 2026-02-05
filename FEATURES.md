# Haunt Backend - Complete Feature Inventory

> **Purpose:** Comprehensive list of every API endpoint, service, database table, and business rule for testing coverage
> **Last Updated:** 2026-02-04

---

## Table of Contents

1. [Authentication & Profiles](#1-authentication--profiles)
2. [Cryptocurrency Data](#2-cryptocurrency-data)
3. [Market Data & Movers](#3-market-data--movers)
4. [Trading System](#4-trading-system)
5. [Signals & Predictions](#5-signals--predictions)
6. [Order Book](#6-order-book)
7. [Alerts System](#7-alerts-system)
8. [Bots & Strategies](#8-bots--strategies)
9. [Peer Mesh & Sync](#9-peer-mesh--sync)
10. [WebSocket Events](#10-websocket-events)
11. [Database Schema](#11-database-schema)
12. [Health & Status](#12-health--status)
13. [Validation Rules](#13-validation-rules)
14. [Services Architecture](#14-services-architecture)

---

## 1. Authentication & Profiles

### 1.1 Endpoints

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | /api/auth/challenge | Get challenge for signing | No |
| POST | /api/auth/verify | Verify signature, get token | No |
| GET | /api/auth/me | Get current user profile | Yes |
| PUT | /api/auth/profile | Update profile settings | Yes |
| POST | /api/auth/profile/leaderboard | Opt in/out leaderboard | Yes |
| POST | /api/auth/logout | Logout, invalidate session | Yes |

### 1.2 Challenge Flow

| Step | Action | Test Criteria |
|------|--------|---------------|
| 1. Request challenge | GET /api/auth/challenge | Returns random string |
| 2. Sign challenge | Client signs with private key | Valid signature |
| 3. Verify | POST /api/auth/verify | Returns session token |
| 4. Use token | Authorization header | Access granted |

### 1.3 Session Management

| Feature | Description | Test Criteria |
|---------|-------------|---------------|
| Token TTL | 24 hours | Expires correctly |
| Challenge TTL | 5 minutes | Expires correctly |
| Redis storage | Sessions in Redis | Persists/expires |
| Token format | JWT-like structure | Valid format |

### 1.4 Profile Data

| Field | Type | Description | Test Criteria |
|-------|------|-------------|---------------|
| id | string | Unique ID | Generated on create |
| public_key | string | User's public key | Unique constraint |
| username | string | Display name | Editable |
| created_at | timestamp | Account creation | Immutable |
| last_seen | timestamp | Last activity | Updates on request |
| show_on_leaderboard | boolean | Public visibility | Toggle works |
| leaderboard_signature | string | Consent signature | Required for opt-in |
| leaderboard_consent_at | timestamp | Consent time | Within ±5 min |
| settings_json | json | User preferences | Any JSON |

### 1.5 Leaderboard Consent

| Requirement | Description | Test Criteria |
|-------------|-------------|---------------|
| Signature | Signed consent message | Valid signature |
| Timestamp | Within ±5 minutes | Rejects stale |
| Message format | "I consent to leaderboard: {timestamp}" | Exact format |

---

## 2. Cryptocurrency Data

### 2.1 Endpoints

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | /api/crypto/listings | Paginated asset list | No |
| GET | /api/crypto/search | Search by name/symbol | No |
| GET | /api/crypto/:id | Single asset details | No |
| GET | /api/crypto/:id/quotes | Quote data only | No |
| GET | /api/crypto/:id/chart | OHLC chart data | No |
| POST | /api/crypto/seed | Trigger data seeding | No |
| POST | /api/crypto/seed/batch | Batch seed symbols | No |
| GET | /api/crypto/seed/status | Seeding status | No |

### 2.2 Listings Query Parameters

| Parameter | Type | Options | Default | Test Criteria |
|-----------|------|---------|---------|---------------|
| start | number | 1-1000 | 1 | Offset works |
| limit | number | 1-100 | 100 | Clamped correctly |
| sort | string | market_cap, price, change_24h, volume_24h | market_cap | Sorts correctly |
| sort_dir | string | asc, desc | desc | Direction works |
| filter | string | gainers, losers, most_volatile, top_volume, all | all | Filters correctly |
| asset_type | string | all, crypto, stock, etf, forex | all | Types filter |
| min_change | number | -100 to 100 | null | Filters % change |
| max_change | number | -100 to 100 | null | Filters % change |

### 2.3 Asset Data Structure

| Field | Type | Description | Test Criteria |
|-------|------|-------------|---------------|
| id | string | Unique identifier | Matches request |
| name | string | Full name | Present |
| symbol | string | Ticker | Uppercase |
| asset_class | string | crypto/stock/etf/forex | Valid type |
| price | number | Current USD price | > 0 |
| price_change_1h | number | 1h % change | Calculated |
| price_change_24h | number | 24h % change | Calculated |
| price_change_7d | number | 7d % change | Calculated |
| market_cap | number | Market cap USD | >= 0 |
| volume_24h | number | 24h volume | >= 0 |
| circulating_supply | number | Circulating | >= 0 |
| total_supply | number | Total supply | >= circulating |
| max_supply | number | Max supply | null or >= total |
| sparkline_7d | array | 7-day price points | 168 points (hourly) |
| trade_direction | string | up/down/null | Based on recent trades |
| last_updated | timestamp | Data freshness | Recent |

### 2.4 Chart Data

| Parameter | Options | Test Criteria |
|-----------|---------|---------------|
| range | 1h, 4h, 1d, 1w, 1m | Returns correct timeframe |

| Response Field | Description | Test Criteria |
|----------------|-------------|---------------|
| symbol | Asset symbol | Matches request |
| range | Requested range | Matches request |
| data | OHLC array | Correct structure |
| seeding_status | pending/in_progress/complete | Accurate |
| data_completeness | 0-100% | Calculated correctly |
| expected_points | Expected data points | For range |

### 2.5 OHLC Point Structure

| Field | Type | Description | Test Criteria |
|-------|------|-------------|---------------|
| timestamp | number | Unix timestamp ms | Valid |
| open | number | Open price | > 0 |
| high | number | High price | >= open, close |
| low | number | Low price | <= open, close |
| close | number | Close price | > 0 |
| volume | number | Period volume | >= 0 |

### 2.6 Data Sources (14 Total)

| Source | Type | Weight | WebSocket | Test Criteria |
|--------|------|--------|-----------|---------------|
| Coinbase | Exchange | 10/10 | Yes | Prices update |
| Binance | Exchange | 9/10 | Yes | Prices update |
| Kraken | Exchange | 8/10 | Yes | Prices update |
| KuCoin | Exchange | 7/10 | Yes | Prices update |
| OKX | Exchange | 7/10 | Yes | Prices update |
| Huobi | Exchange | 6/10 | Yes | Prices update |
| CoinGecko | Aggregator | 8/10 | No | Prices update |
| CryptoCompare | Aggregator | 7/10 | No | Prices update |
| CoinMarketCap | Aggregator | 9/10 | No | Volume authority |
| Hyperliquid | DEX | 7/10 | Yes | Prices update |
| Finnhub | Stocks | 9/10 | Yes | Stocks update |
| AlphaVantage | Stocks | 8/10 | No | Stocks update |
| Alpaca | Stocks | 8/10 | Yes | Stocks update |
| Tiingo | Stocks | 7/10 | Yes | Stocks update |

### 2.7 Price Aggregation

| Factor | Description | Test Criteria |
|--------|-------------|---------------|
| Source weighting | Higher weight = more influence | Weighted average |
| Volume authority | Only CMC/CoinGecko for volume | Volume source correct |
| Spread calculation | Max - Min / Avg | Calculated |
| Consistency check | Reject outliers > 5% deviation | Outliers filtered |
| Trade direction | Based on last N trades | Accurate |

### 2.8 Confidence Scoring

| Factor | Max Points | Calculation | Test Criteria |
|--------|------------|-------------|---------------|
| Source diversity | 25 | Sources × 5 (max 5) | 0-25 |
| Update frequency | 20 | Updates/min × 4 (max 5/min) | 0-20 |
| Data recency | 22 | 22 - (age_seconds / 10) | 0-22 |
| Price consistency | 18 | 18 - (spread% × 3) | 0-18 |
| Missing data | -15 | Per missing critical field | Deduction |
| Total | 100 | Sum of factors | 0-100 |

---

## 3. Market Data & Movers

### 3.1 Endpoints

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | /api/market/global | Global crypto metrics | No |
| GET | /api/market/fear-greed | Fear & Greed Index | No |
| GET | /api/market/exchanges | Per-exchange stats | No |
| GET | /api/market/stats | Server performance | No |
| GET | /api/market/movers | Top gainers/losers | No |
| GET | /api/market/source-stats/:symbol | Symbol source stats | No |
| GET | /api/market/confidence/:symbol | Symbol confidence | No |

### 3.2 Global Metrics

| Metric | Type | Description | Test Criteria |
|--------|------|-------------|---------------|
| total_market_cap | number | Total crypto market cap | > 0 |
| total_volume_24h | number | 24h trading volume | > 0 |
| btc_dominance | number | BTC % of market | 0-100 |
| eth_dominance | number | ETH % of market | 0-100 |
| active_cryptocurrencies | number | Count of assets | > 0 |
| active_exchanges | number | Count of exchanges | > 0 |
| defi_volume_24h | number | DeFi volume | >= 0 |
| defi_market_cap | number | DeFi market cap | >= 0 |
| stablecoin_volume_24h | number | Stablecoin volume | >= 0 |
| last_updated | timestamp | Data freshness | Recent |

### 3.3 Fear & Greed Index

| Field | Type | Description | Test Criteria |
|-------|------|-------------|---------------|
| value | number | Index value | 0-100 |
| classification | string | Text classification | Matches value |
| timestamp | timestamp | When calculated | Recent |

| Classification | Value Range | Test Criteria |
|----------------|-------------|---------------|
| Extreme Fear | 0-24 | Correct label |
| Fear | 25-44 | Correct label |
| Neutral | 45-55 | Correct label |
| Greed | 56-74 | Correct label |
| Extreme Greed | 75-100 | Correct label |

### 3.4 Exchange Stats

| Field | Type | Description | Test Criteria |
|-------|------|-------------|---------------|
| exchange | string | Exchange name | Valid name |
| update_count | number | Updates received | >= 0 |
| uptime_pct | number | Uptime percentage | 0-100 |
| last_update | timestamp | Last received | Recent |
| status | string | online/offline | Accurate |

### 3.5 Server Stats

| Metric | Type | Description | Test Criteria |
|--------|------|-------------|---------------|
| total_updates | number | All-time updates | >= 0 |
| tps | number | Transactions/second | >= 0 |
| uptime_seconds | number | Server uptime | > 0 |
| active_symbols | number | Tracked symbols | > 0 |
| online_sources | number | Active sources | > 0 |
| per_exchange_stats | array | Per-exchange breakdown | All exchanges |

### 3.6 Movers Query Parameters

| Parameter | Options | Default | Test Criteria |
|-----------|---------|---------|---------------|
| timeframe | 1m, 5m, 15m, 1h, 4h, 24h | 24h | Correct window |
| limit | 1-50 | 10 | Clamped |
| asset_type | all, crypto, stock, etf | all | Filters |

### 3.7 Movers Response

| Field | Type | Description | Test Criteria |
|-------|------|-------------|---------------|
| gainers | array | Top gainers | Sorted desc by % |
| losers | array | Top losers | Sorted asc by % |

| Mover Item | Type | Description | Test Criteria |
|------------|------|-------------|---------------|
| symbol | string | Asset symbol | Valid |
| name | string | Asset name | Valid |
| price | number | Current price | > 0 |
| change_pct | number | % change | Matches timeframe |
| volume_24h | number | Volume | >= 0 |

---

## 4. Trading System

### 4.1 Portfolio Endpoints

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | /api/trading/portfolios | List user portfolios | Yes |
| POST | /api/trading/portfolios | Create portfolio | Yes |
| GET | /api/trading/portfolios/:id | Get portfolio | Yes |
| GET | /api/trading/portfolios/:id/summary | Portfolio summary | Yes |
| GET | /api/trading/portfolios/:id/history | Equity curve | Yes |
| PUT | /api/trading/portfolios/:id | Update settings | Yes |
| POST | /api/trading/portfolios/:id/reset | Reset portfolio | Yes |
| DELETE | /api/trading/portfolios/:id | Delete portfolio | Yes |

### 4.2 Portfolio Data

| Field | Type | Description | Test Criteria |
|-------|------|-------------|---------------|
| id | string | Unique ID | Generated |
| user_id | string | Owner ID | Matches auth |
| name | string | Portfolio name | Editable |
| description | string | Description | Nullable |
| base_currency | string | USD/EUR/etc | Default USD |
| starting_balance | number | Initial capital | > 0 |
| cash_balance | number | Available cash | >= 0 |
| margin_used | number | In positions | >= 0 |
| margin_available | number | Free margin | Calculated |
| unrealized_pnl | number | Open P&L | Any |
| realized_pnl | number | Closed P&L | Any |
| total_value | number | Total worth | Calculated |
| total_trades | number | Trade count | >= 0 |
| winning_trades | number | Profitable trades | <= total_trades |
| created_at | timestamp | Creation time | Immutable |
| updated_at | timestamp | Last update | Updates |

### 4.3 Portfolio Summary

| Metric | Type | Description | Test Criteria |
|--------|------|-------------|---------------|
| total_return | number | Absolute return | Calculated |
| total_return_pct | number | % return | Calculated |
| win_rate | number | Win % | winning/total |
| sharpe_ratio | number | Risk-adjusted return | Calculated |
| max_drawdown | number | Max peak-trough | Calculated |
| current_positions | number | Open position count | Accurate |
| open_orders | number | Pending order count | Accurate |
| margin_ratio | number | Used/Available | Calculated |

### 4.4 Order Endpoints

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | /api/trading/orders | List orders | Yes |
| POST | /api/trading/orders | Place order | Yes |
| GET | /api/trading/orders/:id | Get order | Yes |
| PUT | /api/trading/orders/:id | Modify order | Yes |
| DELETE | /api/trading/orders/:id | Cancel order | Yes |
| DELETE | /api/trading/orders | Cancel all | Yes |

### 4.5 Order Query Parameters

| Parameter | Options | Default | Test Criteria |
|-----------|---------|---------|---------------|
| portfolio_id | UUID | required | Filters |
| status | open, filled, cancelled, all | all | Filters |
| symbol | string | null | Filters |
| limit | 1-500 | 100 | Clamped |
| offset | number | 0 | Pagination |

### 4.6 Order Types

| Type | Description | Required Fields | Test Criteria |
|------|-------------|-----------------|---------------|
| market | Immediate execution | symbol, side, quantity | Fills at market |
| limit | Execute at price | symbol, side, quantity, price | Waits for price |
| stop_loss | Trigger on drop | symbol, side, quantity, stop_price | Triggers correctly |
| take_profit | Trigger on rise | symbol, side, quantity, stop_price | Triggers correctly |
| stop_limit | Stop becomes limit | symbol, side, quantity, stop_price, price | Both prices |
| trailing_stop | Follow price | symbol, side, quantity, trail_amount OR trail_percent | Trails |

### 4.7 Order Data

| Field | Type | Description | Test Criteria |
|-------|------|-------------|---------------|
| id | string | Unique ID | Generated |
| portfolio_id | string | Parent portfolio | Valid |
| symbol | string | Trading pair | Valid asset |
| asset_class | string | crypto/stock/etc | Valid |
| side | string | buy/sell | Valid |
| order_type | string | See order types | Valid type |
| quantity | number | Order size | > 0 |
| filled_quantity | number | Filled amount | <= quantity |
| price | number | Limit price | For limit orders |
| stop_price | number | Trigger price | For stop orders |
| trail_amount | number | $ trail amount | XOR with percent |
| trail_percent | number | % trail amount | XOR with amount |
| time_in_force | string | GTC/GTD | Valid |
| status | string | See statuses | Valid |
| leverage | number | Position leverage | 1-100 |
| avg_fill_price | number | Average fill | After fills |
| total_fees | number | Trading fees | Calculated |
| created_at | timestamp | Order time | Immutable |
| updated_at | timestamp | Last update | Updates |
| expires_at | timestamp | For GTD | Nullable |

### 4.8 Order Statuses

| Status | Description | Test Criteria |
|--------|-------------|---------------|
| pending | Waiting to submit | Initial state |
| open | In order book | After submission |
| partially_filled | Some fills | 0 < filled < quantity |
| filled | Complete | filled == quantity |
| cancelled | User cancelled | Manual action |
| expired | Time expired | GTD orders |
| rejected | Invalid/blocked | Validation failed |

### 4.9 Execution Simulation

| Factor | Value | Test Criteria |
|--------|-------|---------------|
| Base slippage (liquid) | 0.01% | Applied to fills |
| Base slippage (illiquid) | 0.05% | Applied to fills |
| Impact factor | 0.1x | Size impact |
| Trading fee | 0.1% | Per fill |
| Min order value | $1 | Rejects smaller |

### 4.10 Position Endpoints

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | /api/trading/positions | List positions | Yes |
| GET | /api/trading/positions/:id | Get position | Yes |
| PUT | /api/trading/positions/:id | Modify SL/TP | Yes |
| DELETE | /api/trading/positions/:id | Close position | Yes |
| POST | /api/trading/positions/:id/margin | Add margin | Yes |

### 4.11 Position Data

| Field | Type | Description | Test Criteria |
|-------|------|-------------|---------------|
| id | string | Unique ID | Generated |
| portfolio_id | string | Parent portfolio | Valid |
| symbol | string | Trading pair | Valid |
| asset_class | string | crypto/stock/etc | Valid |
| side | string | long/short | Valid |
| quantity | number | Position size | > 0 |
| entry_price | number | Average entry | > 0 |
| current_price | number | Mark price | Updates |
| unrealized_pnl | number | Open P&L | Calculated |
| unrealized_pnl_pct | number | P&L % | Calculated |
| realized_pnl | number | Closed portion | >= 0 |
| margin_used | number | Position margin | > 0 |
| leverage | number | Leverage ratio | 1-100 |
| margin_mode | string | isolated/cross | Valid |
| liquidation_price | number | Liq price | Calculated |
| stop_loss | number | SL price | Nullable |
| take_profit | number | TP price | Nullable |
| funding_payments | number | Accumulated funding | Any |
| created_at | timestamp | Open time | Immutable |
| updated_at | timestamp | Last update | Updates |

### 4.12 Liquidation Rules

| Asset Class | Initial Margin | Maint. Margin | Max Leverage | Test Criteria |
|-------------|----------------|---------------|--------------|---------------|
| Crypto Spot | 10% | 5% | 10x | Liq at maint |
| Crypto Perps | 1% | 0.5% | 100x | Liq at maint |
| Stocks | 25% | 20% | 4x | Liq at maint |
| Options | 100% | 100% | 1x | Premium only |
| Forex | 2% | 1% | 50x | Liq at maint |

### 4.13 Margin Warnings

| Level | Margin Used | Action | Test Criteria |
|-------|-------------|--------|---------------|
| Warning | 80% | Toast notification | Notified once |
| Critical | 90% | Prominent alert | Notified once |
| Danger | 95% | Block new trades | Blocked |
| Liquidation | 100% | Force close | Auto-liquidated |

### 4.14 Trade History Endpoint

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | /api/trading/trades | List trade executions | Yes |

### 4.15 Trade Data

| Field | Type | Description | Test Criteria |
|-------|------|-------------|---------------|
| id | string | Unique ID | Generated |
| portfolio_id | string | Parent portfolio | Valid |
| order_id | string | Parent order | Valid |
| position_id | string | Linked position | Nullable |
| symbol | string | Trading pair | Valid |
| asset_class | string | Type | Valid |
| side | string | buy/sell | Valid |
| quantity | number | Executed size | > 0 |
| entry_price | number | Entry/exec price | > 0 |
| exit_price | number | Exit price | For closes |
| realized_pnl | number | Trade P&L | Any |
| fees | number | Trading fees | >= 0 |
| slippage | number | Price slippage | Any |
| created_at | timestamp | Execution time | Immutable |
| exited_at | timestamp | Close time | For closes |

### 4.16 Leaderboard Endpoint

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | /api/trading/leaderboard | Top portfolios | No |

### 4.17 Leaderboard Query Parameters

| Parameter | Type | Default | Test Criteria |
|-----------|------|---------|---------------|
| limit | number | 100 | 1-500 |
| timeframe | string | all | 24h, 7d, 30d, all |

### 4.18 Leaderboard Entry

| Field | Type | Description | Test Criteria |
|-------|------|-------------|---------------|
| rank | number | Position | 1-indexed |
| portfolio_id | string | Portfolio | Valid |
| user_id | string | User | Valid |
| username | string | Display name | Present |
| total_value | number | Portfolio value | > 0 |
| total_return_pct | number | % return | Calculated |
| pnl | number | Absolute P&L | Any |
| win_rate | number | Win % | 0-100 |
| trade_count | number | Total trades | >= 0 |
| badges | array | Achievements | Valid badges |

---

## 5. Signals & Predictions

### 5.1 Endpoints

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | /api/signals/:symbol | Get signals | No |
| POST | /api/signals/:symbol/generate | Force generate | No |
| GET | /api/signals/:symbol/recommendation | Buy/sell rec | No |
| GET | /api/signals/:symbol/accuracy | Accuracy stats | No |
| GET | /api/signals/:symbol/predictions | Prediction history | No |
| GET | /api/signals/accuracy/:indicator | Indicator accuracy | No |

### 5.2 Signal Query Parameters

| Parameter | Options | Default | Test Criteria |
|-----------|---------|---------|---------------|
| timeframe | scalping, day_trading, swing_trading, position_trading | day_trading | Changes weights |

### 5.3 Signal Response

| Field | Type | Description | Test Criteria |
|-------|------|-------------|---------------|
| symbol | string | Asset | Matches request |
| timeframe | string | Analysis timeframe | Matches request |
| composite_score | number | Overall signal | -100 to 100 |
| direction | string | Signal direction | See directions |
| trend_score | number | Trend category | -100 to 100 |
| momentum_score | number | Momentum category | -100 to 100 |
| volatility_score | number | Volatility category | -100 to 100 |
| volume_score | number | Volume category | -100 to 100 |
| signals | array | Individual indicators | All indicators |
| generated_at | timestamp | Generation time | Recent |

### 5.4 Signal Directions

| Direction | Score Range | Test Criteria |
|-----------|-------------|---------------|
| StrongBuy | >= 60 | Correct label |
| Buy | >= 20 | Correct label |
| Neutral | > -20 and < 20 | Correct label |
| Sell | <= -20 | Correct label |
| StrongSell | < -60 | Correct label |

### 5.5 Individual Indicator Signal

| Field | Type | Description | Test Criteria |
|-------|------|-------------|---------------|
| name | string | Indicator name | Valid indicator |
| value | number | Calculated value | Valid range |
| signal | number | Bullish/bearish | -100 to 100 |
| category | string | Indicator category | Valid category |

### 5.6 Technical Indicators (13 Total)

| Indicator | Category | Parameters | Signal Logic | Test Criteria |
|-----------|----------|------------|--------------|---------------|
| SMA | Trend | 20, 50, 200 period | Price vs MA | Crosses detected |
| EMA | Trend | 12, 26 period | Price vs EMA | Crosses detected |
| ADX | Trend | 14 period | Trend strength | > 25 trending |
| VWAP | Trend | Session | Price vs VWAP | Above/below |
| RSI | Momentum | 14 period | Overbought/sold | 70/30 levels |
| MACD | Momentum | 12, 26, 9 | Line vs signal | Crossovers |
| Stochastic | Momentum | 14, 3, 3 | %K vs %D | 80/20 levels |
| CCI | Momentum | 20 period | Channel extremes | ±100 levels |
| OBV | Volume | Cumulative | Volume trend | Direction |
| Bollinger | Volatility | 20, 2 std | Band position | Width, position |
| ATR | Volatility | 14 period | Range measure | Value shown |
| MFI | Volume | 14 period | Money flow | 80/20 levels |

### 5.7 Timeframe Weights

| Category | Scalping | Day Trading | Swing | Position | Test Criteria |
|----------|----------|-------------|-------|----------|---------------|
| Trend | 20% | 35% | 40% | 50% | Weights sum 100% |
| Momentum | 50% | 35% | 30% | 20% | Weights sum 100% |
| Volatility | 20% | 15% | 15% | 15% | Weights sum 100% |
| Volume | 10% | 15% | 15% | 15% | Weights sum 100% |

### 5.8 Prediction Data

| Field | Type | Description | Test Criteria |
|-------|------|-------------|---------------|
| id | string | Unique ID | Generated |
| symbol | string | Asset | Valid |
| indicator | string | Indicator name | Valid |
| direction | string | Predicted direction | buy/sell/neutral |
| score | number | Confidence | -100 to 100 |
| price_at_prediction | number | Entry price | > 0 |
| timestamp | timestamp | Prediction time | Immutable |
| price_after_5m | number | +5m price | Nullable |
| price_after_1h | number | +1h price | Nullable |
| price_after_4h | number | +4h price | Nullable |
| price_after_24h | number | +24h price | Nullable |
| outcome_5m | string | 5m result | correct/incorrect/pending |
| outcome_1h | string | 1h result | correct/incorrect/pending |
| outcome_4h | string | 4h result | correct/incorrect/pending |
| outcome_24h | string | 24h result | correct/incorrect/pending |

### 5.9 Accuracy Response

| Field | Type | Description | Test Criteria |
|-------|------|-------------|---------------|
| symbol | string | Asset | Matches request |
| overall_accuracy | number | Win rate % | 0-100 |
| sample_size | number | Prediction count | >= 0 |
| per_indicator | object | By indicator | All indicators |
| per_direction | object | By direction | buy/sell stats |
| per_timeframe | object | By validation | 5m/1h/4h/24h stats |

---

## 6. Order Book

### 6.1 Endpoint

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | /api/orderbook/:symbol | Aggregated order book | No |

### 6.2 Query Parameters

| Parameter | Type | Default | Test Criteria |
|-----------|------|---------|---------------|
| depth | number | 20 | 1-100 |

### 6.3 Order Book Response

| Field | Type | Description | Test Criteria |
|-------|------|-------------|---------------|
| symbol | string | Asset | Matches request |
| bids | array | Buy orders | Sorted desc |
| asks | array | Sell orders | Sorted asc |
| spread | number | Bid-ask spread | asks[0] - bids[0] |
| spread_pct | number | Spread % | Calculated |
| last_updated | timestamp | Freshness | Recent |

### 6.4 Order Book Level

| Field | Type | Description | Test Criteria |
|-------|------|-------------|---------------|
| price | number | Price level | > 0 |
| quantity | number | Total size | > 0 |
| exchange_count | number | Contributing exchanges | >= 1 |

---

## 7. Alerts System

### 7.1 Endpoints

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | /api/alerts | List user alerts | Yes |
| POST | /api/alerts | Create alert | Yes |
| GET | /api/alerts/:id | Get alert | Yes |
| DELETE | /api/alerts/:id | Delete alert | Yes |

### 7.2 Alert Data

| Field | Type | Description | Test Criteria |
|-------|------|-------------|---------------|
| id | string | Unique ID | Generated |
| user_id | string | Owner | Matches auth |
| symbol | string | Asset to watch | Valid |
| condition | string | above/below/crosses | Valid |
| target_price | number | Trigger price | > 0 |
| triggered | boolean | Has fired | Initially false |
| triggered_at | timestamp | Fire time | When triggered |
| notification_type | string | push/email/webhook | Valid |
| created_at | timestamp | Creation | Immutable |

### 7.3 Alert Conditions

| Condition | Description | Test Criteria |
|-----------|-------------|---------------|
| above | Price rises above target | Triggers on cross up |
| below | Price drops below target | Triggers on cross down |
| crosses | Price crosses either way | Triggers on any cross |

---

## 8. Bots & Strategies

### 8.1 Endpoints

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | /api/bots | List all bots | No |
| GET | /api/bots/:id | Get bot details | No |
| GET | /api/bots/:id/performance | Bot performance | No |
| GET | /api/bots/:id/trades | Bot trade history | No |
| POST | /api/bots/:id/follow | Follow bot | Yes |
| POST | /api/bots/:id/unfollow | Unfollow bot | Yes |

### 8.2 Bot Personalities

| Bot | Strategy | Risk Level | Asset Focus | Test Criteria |
|-----|----------|------------|-------------|---------------|
| Scalper | High-frequency momentum | Medium | Crypto spot | Quick trades |
| Grandma | Buy-and-hold dividends | Very Low | Dividend stocks | Long holds |
| Quant | Statistical arbitrage | Moderate | All assets | Model-based |
| CryptoBro | Trend chasing | High | Alt coins | Leveraged |

### 8.3 Bot Data

| Field | Type | Description | Test Criteria |
|-------|------|-------------|---------------|
| id | string | Unique ID | Generated |
| name | string | Display name | Present |
| personality | string | Bot type | Valid type |
| portfolio_id | string | Bot's portfolio | Valid |
| status | string | active/paused/stopped | Valid |
| supported_assets | array | Tradeable assets | Non-empty |
| description | string | Strategy description | Present |
| created_at | timestamp | Creation | Immutable |

### 8.4 Bot Performance

| Metric | Type | Description | Test Criteria |
|--------|------|-------------|---------------|
| total_return | number | % return | Calculated |
| win_rate | number | Win % | 0-100 |
| sharpe_ratio | number | Risk-adjusted | Calculated |
| max_drawdown | number | Max loss | Calculated |
| total_trades | number | Trade count | >= 0 |
| avg_trade_duration | number | Seconds | > 0 |

### 8.5 TradingBot Trait Methods

| Method | Description | Test Criteria |
|--------|-------------|---------------|
| name() | Bot name | Returns string |
| personality() | Bot type | Valid enum |
| config() | Bot settings | Valid config |
| supported_asset_classes() | Tradeable types | Non-empty |
| analyze(context) | Generate decision | Returns decision |
| on_trade_executed() | Post-trade hook | Called on fill |
| tick() | Periodic update | Called regularly |
| get_state() | Serialize state | Returns JSON |
| restore_state() | Restore state | Accepts JSON |

### 8.6 Strategy Table

| Field | Type | Description | Test Criteria |
|-------|------|-------------|---------------|
| id | string | Unique ID | Generated |
| portfolio_id | string | Parent portfolio | Valid |
| name | string | Strategy name | Present |
| type | string | Strategy type | Valid |
| enabled | boolean | Active | true/false |
| parameters_json | json | Config | Valid JSON |
| status | string | Status | Valid |
| created_at | timestamp | Creation | Immutable |
| updated_at | timestamp | Last update | Updates |

---

## 9. Peer Mesh & Sync

### 9.1 Peer Mesh Endpoints

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | /api/mesh | Mesh discovery | No |
| GET | /api/peers | Peer status | No |

### 9.2 Sync Endpoints

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | /api/sync/health | Local sync state | No |
| GET | /api/sync/mesh-status | All nodes sync health | No |
| GET | /api/sync/metrics | Sync performance | No |
| GET | /api/sync/queue | Pending sync items | No |

### 9.3 Peer Status Response

| Field | Type | Description | Test Criteria |
|-------|------|-------------|---------------|
| server_id | string | This node ID | Matches config |
| server_region | string | This node region | Matches config |
| peers | array | Connected peers | >= 0 |
| connected_count | number | Online peers | Accurate |
| total_peers | number | Known peers | >= connected |

### 9.4 Peer Info

| Field | Type | Description | Test Criteria |
|-------|------|-------------|---------------|
| id | string | Peer node ID | Valid |
| region | string | Peer region | Valid |
| status | string | Connection status | Valid |
| latency_ms | number | Round-trip time | > 0 |
| uptime_pct | number | Uptime % | 0-100 |
| last_ping | timestamp | Last ping | Recent |
| ping_count | number | Total pings | >= 0 |
| failed_pings | number | Failed pings | < ping_count |

### 9.5 Peer Connection States

| State | Description | Test Criteria |
|-------|-------------|---------------|
| Connected | Active connection | WebSocket open |
| Connecting | Establishing | In progress |
| Disconnected | Not connected | Clean disconnect |
| Failed | Connection error | After retries |

### 9.6 Sync Health Response

| Field | Type | Description | Test Criteria |
|-------|------|-------------|---------------|
| node_id | string | This node | Matches |
| is_primary | bool | Primary node (Osaka) | Correct |
| sync_enabled | bool | Sync active | true |
| last_full_sync_at | timestamp | Last full sync | Past |
| last_incremental_at | timestamp | Last incremental | Recent |
| pending_sync_count | number | Queue size | >= 0 |
| failed_sync_count | number | Failures | >= 0 |
| database_size_mb | number | DB size | > 0 |
| database_row_count | number | Total rows | > 0 |

### 9.7 Sync Entity Types

| Entity | Priority | Consistency | Conflict Strategy | Test Criteria |
|--------|----------|-------------|-------------------|---------------|
| Profile | Medium | Eventual | LastWriteWins | Syncs |
| Portfolio | Critical | Strong | PrimaryWins | Syncs |
| Order | Critical | Strong | PrimaryWins | Syncs |
| Position | Critical | Strong | PrimaryWins | Syncs |
| Trade | High | Eventual | Merge (append) | Syncs |
| OptionsPosition | High | Strong | PrimaryWins | Syncs |
| Strategy | Medium | Eventual | LastWriteWins | Syncs |
| FundingPayment | Medium | Eventual | Merge | Syncs |
| Liquidation | High | Eventual | Merge | Syncs |
| MarginHistory | Medium | Eventual | Merge | Syncs |
| PortfolioSnapshot | Low | Eventual | Merge | Syncs |
| InsuranceFund | High | Strong | PrimaryWins (Osaka) | Syncs |
| PredictionHistory | Low | Eventual | Merge | Syncs |

### 9.8 Sync Queue Item

| Field | Type | Description | Test Criteria |
|-------|------|-------------|---------------|
| id | string | Queue item ID | Generated |
| entity_type | string | Entity type | Valid |
| entity_id | string | Entity ID | Valid |
| operation | string | insert/update/delete | Valid |
| priority | number | 0-10 (0 highest) | Valid |
| target_nodes | array | Specific nodes | Nullable = all |
| retry_count | number | Retry attempts | >= 0 |
| created_at | timestamp | Queued time | Immutable |
| scheduled_at | timestamp | Next attempt | Future |
| attempted_at | timestamp | Last attempt | Nullable |
| completed_at | timestamp | Success time | Nullable |
| error | string | Last error | Nullable |

### 9.9 Sync Message Format

| Field | Type | Description | Test Criteria |
|-------|------|-------------|---------------|
| id | string | Message ID | Unique |
| source_node | string | Originating node | Valid |
| timestamp | number | Unix timestamp | Recent |
| entity_type | string | Entity type | Valid |
| entity_id | string | Entity ID | Valid |
| operation | string | Operation | Valid |
| data | bytes | Serialized entity | Valid JSON |
| version | number | Entity version | > 0 |
| checksum | string | SHA256 | Verifiable |
| compressed | bool | Gzip compressed | Decompresses |

### 9.10 Peer Messages

| Message | Direction | Purpose | Test Criteria |
|---------|-----------|---------|---------------|
| Auth | Client→Peer | Authenticate | HMAC verified |
| Ping | Bidirectional | Latency check | Responded |
| Pong | Bidirectional | Ping response | Correct timing |
| Announce | Peer→Peer | Introduce self | Contains URLs |
| SharePeers | Peer→Peer | Peer list | Array of peers |
| RequestPeers | Peer→Peer | Ask for peers | Triggers share |
| SyncData | Peer→Peer | Data sync | Processed |

---

## 10. WebSocket Events

### 10.1 Client → Server Messages

| Message | Fields | Purpose | Test Criteria |
|---------|--------|---------|---------------|
| Subscribe | assets: string[] | Subscribe to prices | Confirmation |
| Unsubscribe | assets: string[] | Unsubscribe | Confirmation |
| SetThrottle | throttle_ms: number | Rate limit | Applied |
| SubscribePeers | - | Subscribe to mesh | Confirmation |
| UnsubscribePeers | - | Unsubscribe mesh | Confirmation |
| SubscribeTrading | portfolio_id: string | Trading updates | Requires auth |
| UnsubscribeTrading | portfolio_id: string | Unsubscribe trading | Confirmation |
| Ping | from_id, from_region, timestamp | Heartbeat | Pong returned |
| Auth | id, region, timestamp, signature | Peer auth | AuthResponse |
| Identify | id, region, version | Self-identify | Logged |

### 10.2 Server → Client Messages

| Message | Fields | Purpose | Test Criteria |
|---------|--------|---------|---------------|
| PriceUpdate | symbol, price, change24h, etc | Price data | Received |
| MarketUpdate | totalMarketCap, volume, etc | Market data | Received |
| SeedingProgress | symbol, completion%, status | Seed progress | Updates |
| SignalUpdate | symbol, score, direction, etc | Signal data | Received |
| PeerUpdate | peers: PeerInfo[] | Mesh status | Received |
| Subscribed | assets: string[] | Confirmation | Matches |
| Unsubscribed | assets: string[] | Confirmation | Matches |
| ThrottleSet | throttle_ms | Confirmation | Matches |
| PeersSubscribed | - | Confirmation | Received |
| PeersUnsubscribed | - | Confirmation | Received |
| TradingSubscribed | portfolio_id | Confirmation | Received |
| TradingUnsubscribed | portfolio_id | Confirmation | Received |
| OrderUpdate | id, status, filled, etc | Order change | Received |
| PositionUpdate | id, pnl, price, etc | Position change | Received |
| PortfolioUpdate | balance, margin, pnl | Portfolio change | Received |
| TradeExecution | trade details | Trade happened | Received |
| MarginWarning | level, margin_used | Warning | Received |
| LiquidationAlert | position_id, details | Liquidation | Received |
| Error | error: string | Error message | Received |
| Pong | from_id, from_region, original_timestamp | Heartbeat response | Received |
| AuthResponse | success: bool, error?: string | Auth result | Handled |

### 10.3 Price Update Data

| Field | Type | Description | Test Criteria |
|-------|------|-------------|---------------|
| symbol | string | Asset | Valid |
| price | number | Current price | > 0 |
| price_change_24h | number | 24h change % | Any |
| volume_24h | number | Volume | >= 0 |
| trade_direction | string | up/down | Valid |
| confidence | number | Data quality | 0-100 |
| source_count | number | Data sources | > 0 |
| timestamp | number | Update time | Recent |

### 10.4 Trading Event Data

| Event | Key Fields | Test Criteria |
|-------|------------|---------------|
| OrderUpdate | id, status, filled_qty, avg_price, fee | Accurate |
| PositionUpdate | id, current_price, unrealized_pnl, event | Accurate |
| PortfolioUpdate | balance, margin_used, unrealized_pnl | Accurate |
| TradeExecution | order_id, symbol, side, qty, price, fee | Accurate |
| MarginWarning | level (80/90/95), margin_used_pct | Triggers once per level |
| LiquidationAlert | position_id, symbol, loss_amount | Immediate |

---

## 11. Database Schema

### 11.1 Core Tables

| Table | Purpose | Key Columns | Test Criteria |
|-------|---------|-------------|---------------|
| profiles | User accounts | id, public_key, username | Unique public_key |
| portfolios | Trading portfolios | id, user_id, balance | User ownership |
| orders | Order history | id, portfolio_id, status | FK to portfolio |
| positions | Open/closed positions | id, portfolio_id, symbol | FK to portfolio |
| trades | Trade executions | id, order_id, symbol | FK to order |
| prediction_history | Signal predictions | id, symbol, indicator | Indexed by symbol |

### 11.2 Advanced Trading Tables

| Table | Purpose | Key Columns | Test Criteria |
|-------|---------|-------------|---------------|
| funding_payments | Perp funding | id, position_id, amount | FK to position |
| liquidations | Forced closes | id, position_id, loss | FK to position |
| margin_history | Margin changes | id, portfolio_id, timestamp | Time-series |
| portfolio_snapshots | Equity curve | id, portfolio_id, value | Time-series |
| insurance_fund | Liq insurance | id, balance | Single row |
| options_positions | Options | id, portfolio_id, details | FK to portfolio |
| strategies | Bot strategies | id, portfolio_id, config | FK to portfolio |
| adl_entries | Auto-deleverage | id, position_id | FK to position |

### 11.3 Sync/Mesh Tables

| Table | Purpose | Key Columns | Test Criteria |
|-------|---------|-------------|---------------|
| sync_versions | Version tracking | entity_type, entity_id, version | Per-entity |
| sync_state | Sync progress | id (singleton), cursors | Single row |
| sync_queue | Pending syncs | id, entity_type, priority | Processed FIFO |
| sync_conflicts | Conflict log | id, entity_type, resolution | Logged |
| node_metrics | Node stats | id, node_id, timestamp | Time-series |

### 11.4 Key Indexes

| Table | Index | Columns | Purpose |
|-------|-------|---------|---------|
| profiles | idx_profiles_public_key | public_key | Unique lookup |
| portfolios | idx_portfolios_user_id | user_id | User's portfolios |
| orders | idx_orders_portfolio_id | portfolio_id | Portfolio's orders |
| orders | idx_orders_status | status | Open orders |
| orders | idx_orders_symbol | symbol | Symbol's orders |
| positions | idx_positions_portfolio_id | portfolio_id | Portfolio positions |
| positions | idx_positions_symbol | symbol | Symbol positions |
| predictions | idx_predictions_symbol | symbol | Symbol history |
| predictions | idx_predictions_timestamp | timestamp | Time-based |
| sync_queue | idx_sync_queue_scheduled | scheduled_at | Processing |
| sync_queue | idx_sync_queue_priority | priority, scheduled_at | Priority order |

---

## 12. Health & Status

### 12.1 Endpoint

| Method | Endpoint | Description | Auth |
|--------|----------|-------------|------|
| GET | /health | Basic health check | No |

### 12.2 Response

| Field | Type | Description | Test Criteria |
|-------|------|-------------|---------------|
| status | string | "ok" | Always ok if running |
| version | string | Server version | Present |
| uptime_seconds | number | Server uptime | > 0 |

---

## 13. Validation Rules

### 13.1 Authentication

| Rule | Description | Test Criteria |
|------|-------------|---------------|
| Challenge expiry | 5 minutes | Rejects expired |
| Session expiry | 24 hours | Rejects expired |
| Signature validity | HMAC-SHA256 | Rejects invalid |
| Consent timestamp | ±5 minutes | Rejects stale |

### 13.2 Trading

| Rule | Value | Test Criteria |
|------|-------|---------------|
| Min order value | $1 | Rejects smaller |
| Max leverage (crypto) | 100x | Clamps |
| Max leverage (stocks) | 4x | Clamps |
| Position limit | Per portfolio config | Enforces |
| Stop price (SL) | < entry (long) | Validates |
| Stop price (TP) | > entry (long) | Validates |
| Trail stop | $ XOR % | One only |
| GTD expires_at | Future timestamp | Validates |
| Fill qty | <= order qty | Enforces |

### 13.3 Market Data

| Rule | Range | Test Criteria |
|------|-------|---------------|
| Listings start | 1-1000 | Clamps |
| Listings limit | 1-100 | Clamps |
| Chart range | 1h/4h/1d/1w/1m | Validates |
| Movers timeframe | 1m-24h | Validates |
| Search limit | 1-100 | Clamps |

### 13.4 Signals

| Rule | Options | Test Criteria |
|------|---------|---------------|
| Timeframe | scalping/day_trading/swing_trading/position_trading | Validates |
| Score range | -100 to 100 | Clamps |

---

## 14. Services Architecture

### 14.1 Core Services

| Service | Purpose | Key Methods |
|---------|---------|-------------|
| AuthService | Sessions, signatures | verify_signature, create_session |
| TradingService | Portfolio & orders | place_order, execute_order, close_position |
| PriceCache | Price aggregation | get_price, get_aggregate, update_price |
| ChartStore | OHLC data | get_chart, append_candle |
| HistoricalDataService | Data seeding | seed_symbol, get_seed_status |
| SignalStore | Indicator calculations | calculate_signals, get_recommendation |
| PredictionStore | Prediction tracking | record_prediction, validate_outcomes |

### 14.2 Trading Services

| Service | Purpose | Key Methods |
|---------|---------|-------------|
| LiquidationEngine | Margin monitoring | check_margin_levels, liquidate_position |
| OrderBookService | Aggregated book | get_orderbook, update_level |
| LiquiditySimulator | Execution simulation | simulate_fill, calculate_slippage |
| BacktestRunner | Strategy testing | run_backtest, calculate_metrics |

### 14.3 Distributed Services

| Service | Purpose | Key Methods |
|---------|---------|-------------|
| PeerMesh | Node connectivity | connect_peer, broadcast, ping |
| SyncService | Data synchronization | queue_sync, process_queue, handle_message |
| BotRunner | Bot execution | run_bots, execute_decision |

---

## Testing Checklist Summary

### Critical Paths (Must Test)

1. [ ] Auth: Challenge → Sign → Verify → Session → Protected endpoint
2. [ ] Trading: Create portfolio → Place order → Fill → Position created
3. [ ] Order lifecycle: Pending → Open → Partial → Filled
4. [ ] Order lifecycle: Pending → Cancelled
5. [ ] Position: Open → Modify SL/TP → Close → Trade recorded
6. [ ] Liquidation: Position → Margin warning → Liquidation
7. [ ] Sync: Create on Node A → Verify on Node B (once fixed)
8. [ ] WebSocket: Subscribe → Receive updates → Unsubscribe

### API Coverage Targets

| Category | Endpoints | Priority |
|----------|-----------|----------|
| Auth | 6 | Critical |
| Trading | 18 | Critical |
| Market Data | 7 | High |
| Crypto Data | 8 | High |
| Signals | 6 | Medium |
| Alerts | 4 | Medium |
| Bots | 6 | Medium |
| Sync/Mesh | 6 | Critical (for multi-node) |
| WebSocket | 25+ events | High |

---

*Total Endpoints: 50+*
*Total WebSocket Events: 25+*
*Total Database Tables: 19*
*Total Validation Rules: 25+*
