# Haunt Paper Trading Platform - Implementation Plan

## Executive Summary

This document outlines the comprehensive plan for implementing a full-featured paper trading platform within Haunt. The system will support portfolio management, synthetic trade execution, and multi-asset trading across crypto, stocks, ETFs, perpetual futures, options, and forex.

**Starting Balance:** $5,000,000 USD per portfolio
**Target:** Professional-grade paper trading with realistic execution simulation

---

## Feature Specifications

### 1. Order Types (Full Suite)

| Order Type | Description | Priority |
|------------|-------------|----------|
| **Market Order** | Execute immediately at best available price | P1 |
| **Limit Order** | Execute at specified price or better | P1 |
| **Stop Loss** | Trigger sell when price drops to threshold | P1 |
| **Take Profit** | Trigger sell when price reaches profit target | P1 |
| **Stop Limit** | Stop loss that becomes a limit order when triggered | P1 |
| **Trailing Stop** | Dynamic stop that follows price by fixed amount or % | P2 |
| **OCO (One-Cancels-Other)** | Two linked orders; execution of one cancels the other | P2 |
| **Fill or Kill (FOK)** | Execute entire order immediately or cancel completely | P2 |
| **Immediate or Cancel (IOC)** | Fill available quantity immediately, cancel remainder | P2 |
| **Good Till Cancelled (GTC)** | Order remains active until filled or manually cancelled | P1 |
| **Good Till Date (GTD)** | Order expires at specified date/time | P2 |
| **Bracket Order** | Entry order + take profit + stop loss as single unit | P2 |

**Implementation Notes:**
- Orders stored in SQLite with status tracking
- Real-time order monitoring via background tasks
- Price triggers evaluated against aggregated price feed
- Partial fill support for realistic execution

---

### 2. Asset Classes

| Asset Type | Support Level | Data Sources | Leverage |
|------------|--------------|--------------|----------|
| **Crypto Spot** | Full | Binance, Coinbase, Kraken, CoinGecko, CMC | Up to 10x margin |
| **Stocks** | Full | Alpaca, Finnhub, Yahoo Finance | Up to 4x (Reg-T) |
| **ETFs** | Full | Alpaca, Finnhub, Yahoo Finance | Up to 4x |
| **Perpetual Futures** | Full | Binance, OKX, Hyperliquid | Up to 100x |
| **Options** | Full | Alpaca, Finnhub (chains) | Premium-based |
| **Forex (Majors)** | Full | Alpha Vantage, Finnhub, Twelve Data, FCS API | Up to 50x |

**Forex Pairs (Major):**
- EUR/USD, GBP/USD, USD/JPY, USD/CHF
- AUD/USD, USD/CAD, NZD/USD
- Major crosses: EUR/GBP, EUR/JPY, GBP/JPY

---

### 3. Position Management

#### Long & Short Positions
- Full support for both directions across all asset types
- Short selling simulation with borrow fee calculation
- Position flipping (long to short and vice versa)

#### Leverage & Margin

| Asset Type | Max Leverage | Initial Margin | Maintenance Margin |
|------------|--------------|----------------|-------------------|
| Crypto Perps | 100x | 1% | 0.5% |
| Crypto Spot | 10x | 10% | 5% |
| Stocks | 4x | 25% | 25% |
| ETFs | 4x | 25% | 25% |
| Forex | 50x | 2% | 1% |
| Options | N/A (premium) | 100% premium | N/A |

#### Margin Modes (Perps)
- **Isolated Margin:** Each position has independent margin
- **Cross Margin:** All positions share account margin
- User-selectable per position

#### Position Features
- Partial close support
- Dollar-cost averaging (add to position)
- Position sizing calculator (risk-based)
- Cost basis tracking (FIFO, LIFO, Average Cost - user selectable)

---

### 4. Risk Management

#### Enforced Controls (System-Level)
- Maximum leverage limits per asset type
- Margin requirements and maintenance levels
- Liquidation engine for underwater positions
- Insurance fund deductions on liquidation

#### Advisory Controls (User-Configurable)
| Control | Default | Range |
|---------|---------|-------|
| Max position size | 25% of portfolio | 1-100% |
| Daily loss limit | 10% of portfolio | 1-50% |
| Max open positions | 20 | 1-100 |
| Risk per trade | 2% of portfolio | 0.1-10% |
| Portfolio stop (drawdown) | 25% | 5-50% |
| Correlation warning threshold | 0.7 | 0.5-0.95 |

#### Liquidation System (Gradual)
1. **Warning at 80%** maintenance margin - alert sent
2. **Warning at 90%** maintenance margin - urgent alert
3. **Warning at 95%** maintenance margin - final warning
4. **Partial liquidation** begins at maintenance breach
5. **ADL (Auto-Deleverage)** for counterparty positions if needed
6. **Insurance fund** covers shortfalls

---

### 5. Execution Simulation (Highly Realistic)

#### Order Book Integration
- Use real-time order book data from top exchanges
- Binance, Coinbase, Kraken for crypto
- Alpaca, Finnhub for stocks

#### Slippage Simulation
```
slippage = base_slippage + (order_size / available_liquidity) * impact_factor

Where:
- base_slippage: 0.01% for liquid assets, 0.05% for illiquid
- available_liquidity: sum of order book depth at price levels
- impact_factor: 0.1 for crypto, 0.05 for stocks
```

#### Partial Fills
- Large orders fill incrementally based on order book depth
- Each fill level has different price (price improvement or slippage)
- Fill notifications sent in real-time

#### Spread Simulation
- Use real bid-ask spread from aggregated order book
- Market buys execute at ask, sells at bid
- Limit orders must cross spread to fill

#### Latency Modeling
- Configurable simulated latency (default: 50-200ms random)
- Affects time between order submission and execution
- Can be disabled for instant fills

---

### 6. Perpetual Futures (Full Derivatives)

#### Funding Rate System
- 8-hour funding intervals (00:00, 08:00, 16:00 UTC)
- Rate calculated from premium index
- Longs pay shorts when positive, shorts pay longs when negative
- Historical funding rate tracking

#### Mark Price
- Used for liquidation calculations (not last price)
- Calculated from index price + funding basis
- Prevents manipulation-based liquidations

#### Margin Modes
- **Isolated:** Margin isolated per position, max loss = position margin
- **Cross:** Shared margin across all positions, higher capital efficiency

#### Position Limits
| Tier | Position Size | Max Leverage |
|------|--------------|--------------|
| 1 | < $50,000 | 100x |
| 2 | < $250,000 | 50x |
| 3 | < $1,000,000 | 20x |
| 4 | < $5,000,000 | 10x |
| 5 | > $5,000,000 | 5x |

#### ADL (Auto-Deleverage) System
- Priority queue based on profit and leverage
- Highest profit + highest leverage = first to be deleveraged
- Triggered when insurance fund insufficient

#### Insurance Fund
- Virtual fund that absorbs liquidation losses
- Replenished by liquidation fees (0.5% of position)
- Tracks fund balance over time

---

### 7. Options Trading (Full)

#### Option Types
- **American Style:** Exercise any time before expiration
- **European Style:** Exercise only at expiration
- Auto-detection based on underlying asset

#### Greeks Calculation
| Greek | Description | Update Frequency |
|-------|-------------|------------------|
| Delta | Price sensitivity | Real-time |
| Gamma | Delta sensitivity | Real-time |
| Theta | Time decay | Hourly |
| Vega | Volatility sensitivity | Real-time |
| Rho | Interest rate sensitivity | Daily |

#### Options Features
- Options chains with strikes and expirations
- Implied volatility calculation
- Premium pricing (Black-Scholes for European, Binomial for American)

#### Multi-Leg Strategies
- Spreads: Bull/Bear Call/Put Spread
- Straddles and Strangles
- Iron Condor, Iron Butterfly
- Calendar Spreads
- Custom multi-leg combinations

#### Expiration Handling
- Automatic exercise of ITM options at expiration
- OTM options expire worthless
- Early exercise simulation for American options
- Assignment simulation for short options

---

### 8. Portfolio Analytics (Comprehensive Suite)

#### Core Metrics
| Metric | Calculation | Update Frequency |
|--------|-------------|------------------|
| Unrealized P&L | (current_price - entry_price) * quantity | Real-time |
| Realized P&L | Sum of closed position profits/losses | On close |
| Total P&L | Unrealized + Realized | Real-time |
| Return % | (current_value - initial_value) / initial_value | Real-time |

#### Time-Based Returns
- Daily, Weekly, Monthly, Quarterly, YTD, All-Time
- Rolling periods (last 7d, 30d, 90d, 365d)
- Comparison to benchmarks (BTC, SPY, etc.)

#### Risk-Adjusted Metrics
| Metric | Formula |
|--------|---------|
| Sharpe Ratio | (Return - Risk_Free) / Std_Dev |
| Sortino Ratio | (Return - Risk_Free) / Downside_Dev |
| Calmar Ratio | Annual_Return / Max_Drawdown |
| Information Ratio | (Return - Benchmark) / Tracking_Error |

#### Drawdown Analysis
- Current drawdown from peak
- Maximum drawdown (all-time)
- Drawdown duration (days underwater)
- Recovery time from drawdowns

#### Trade Statistics
| Metric | Description |
|--------|-------------|
| Win Rate | % of profitable trades |
| Loss Rate | % of losing trades |
| Avg Win | Average profit on winning trades |
| Avg Loss | Average loss on losing trades |
| Risk/Reward | Avg Win / Avg Loss |
| Profit Factor | Gross Profit / Gross Loss |
| Expectancy | (Win% * Avg Win) - (Loss% * Avg Loss) |

#### Position Attribution
- P&L breakdown by asset
- P&L breakdown by asset class
- P&L breakdown by strategy/tag
- Contribution to portfolio return

#### Beta & Alpha
- Portfolio beta vs benchmark (configurable)
- Jensen's Alpha calculation
- R-squared (correlation to benchmark)

---

### 9. Account Management

#### Multi-Portfolio Support
- Unlimited portfolios per user
- Each portfolio has independent:
  - Balance and positions
  - Risk settings
  - Performance history
  - Strategy tags/labels

#### Portfolio Operations
| Operation | Description |
|-----------|-------------|
| Create | New portfolio with $5,000,000 starting balance |
| Reset | Wipe all positions and reset to starting balance |
| Top-up | Add virtual funds (tracked separately) |
| Archive | Soft-delete, preserve history |
| Clone | Copy settings to new portfolio |

#### Portfolio Settings
- Name and description
- Base currency (USD, EUR, BTC, etc.)
- Risk parameters (overrides defaults)
- Cost basis method (FIFO, LIFO, Average)
- Default leverage per asset type

---

### 10. Competition Platform

#### Competition Types
| Type | Duration | Reset |
|------|----------|-------|
| Daily | 24 hours | Daily at 00:00 UTC |
| Weekly | 7 days | Monday 00:00 UTC |
| Monthly | Calendar month | 1st of month |
| Custom | Admin-defined | Custom |

#### Leaderboard Rankings
- **By Return %** - Raw performance
- **By Sharpe Ratio** - Risk-adjusted (featured)
- **By Profit Factor** - Consistency
- **By Max Drawdown** - Risk management

#### Competition Features
- Separate competition portfolios (isolated from main)
- Entry requirements (minimum trades, etc.)
- **Prizes:** Leaderboard placement only (no monetary rewards)
- Historical competition results

#### Social Features
- Public/private portfolio visibility toggle
- Follow traders (get notifications on their trades)
- Copy trading (mirror positions automatically)
  - **Copy trading fees:** Copiers pay a configurable fee to the trader they follow
  - **Default fee:** 5% of profits (configurable 0-25% by copied trader)
  - Fee only charged on profitable copied trades
- Trader profiles with statistics

#### Achievements & Badges
| Badge | Criteria |
|-------|----------|
| First Blood | Complete first trade |
| Centurion | 100 trades completed |
| Sharp Shooter | 70%+ win rate (min 50 trades) |
| Diamond Hands | Hold position 30+ days profitably |
| Risk Manager | Never exceed 5% daily loss (30 days) |
| Top 10 | Finish in top 10 of competition |
| Champion | Win a competition |

---

### 11. Signal Integration & Auto-Trading

#### Signal-to-Trade Rules
```yaml
rule:
  name: "RSI Oversold Buy"
  conditions:
    - indicator: RSI
      operator: "<"
      value: 30
    - indicator: MACD
      signal: "bullish_cross"
  action:
    type: "market_buy"
    size: "2% of portfolio"
    stop_loss: "3%"
    take_profit: "6%"
  cooldown: "4h"
  max_positions: 3
```

#### Rule Builder Features
- Visual rule builder UI
- Condition combinations (AND, OR, NOT)
- All 13 existing indicators supported
- Price-based conditions
- Time-based conditions (market hours, etc.)
- Position-aware rules (only if no existing position)

#### Strategy Templates
- Pre-built strategies users can clone
- Community-shared strategies
- Backtested performance shown

#### Backtesting Engine
- Test strategies against historical data
- Use existing chart store (up to 90 days)
- Fetch additional historical data on-demand
- Cache fetched data for reuse

#### Backtest Reports
- Equity curve visualization
- Trade-by-trade breakdown
- Performance metrics (same as live)
- Comparison to buy-and-hold
- Monte Carlo simulation for robustness

---

### 12. Trade History & Reporting

#### Order History
- Complete audit trail of all orders
- Status tracking: Pending → Filled/Cancelled/Expired
- Partial fill tracking
- Order modification history

#### Position History
- Entry and exit details
- P&L per position
- Holding duration
- Tags and notes

#### Export Formats
- CSV (Excel-compatible)
- JSON (programmatic access)
- PDF reports (formatted summaries)

#### Tax Reporting
- Realized gains/losses by tax year
- Cost basis method applied (FIFO/LIFO/Average)
- Short-term vs long-term classification
- Export for tax software (TurboTax, etc.)

#### Trade Journal
- Add notes to any trade
- Tag trades with strategies
- Screenshot attachment support
- Searchable and filterable

#### Retention Policy
- Unlimited retention for all history
- Archived portfolios preserved
- Compliance-ready audit trail

---

### 13. Alerts & Notifications

#### Alert Types
| Alert | Trigger | Channels |
|-------|---------|----------|
| Order Filled | Order execution | WebSocket, Webhook |
| Order Cancelled | Order cancelled/expired | WebSocket, Webhook |
| Stop Loss Hit | Stop loss triggered | WebSocket, Webhook |
| Take Profit Hit | Take profit triggered | WebSocket, Webhook |
| Margin Warning | 80%/90%/95% maintenance | WebSocket, Webhook |
| Liquidation | Position liquidated | WebSocket, Webhook |
| Price Alert | Price crosses threshold | WebSocket, Webhook |
| Signal Alert | Trading signal fired | WebSocket, Webhook |
| Portfolio Alert | Drawdown threshold | WebSocket, Webhook |
| Competition | Rank change, competition end | WebSocket, Webhook |

#### Notification Channels
- **WebSocket:** Real-time in-app (existing infrastructure)
- **Webhooks:** HTTP POST to user-configured URLs
- **Future:** Email, Push notifications (mobile)

#### Integration with Existing Signals
- Extend current WebSocket SignalUpdate messages
- Add portfolio context to signal alerts
- Trigger auto-trade rules from signal alerts

---

### 14. API & Integration

#### REST API Endpoints

**Portfolio Management**
```
GET    /api/portfolio                    # List user portfolios
POST   /api/portfolio                    # Create portfolio
GET    /api/portfolio/:id                # Get portfolio details
PUT    /api/portfolio/:id                # Update portfolio settings
DELETE /api/portfolio/:id                # Archive portfolio
POST   /api/portfolio/:id/reset          # Reset portfolio
POST   /api/portfolio/:id/topup          # Add virtual funds
```

**Orders**
```
GET    /api/orders                       # List orders (filterable)
POST   /api/orders                       # Place order
GET    /api/orders/:id                   # Get order details
PUT    /api/orders/:id                   # Modify order
DELETE /api/orders/:id                   # Cancel order
```

**Positions**
```
GET    /api/positions                    # List open positions
GET    /api/positions/:id                # Get position details
PUT    /api/positions/:id                # Modify position (SL/TP)
DELETE /api/positions/:id                # Close position
POST   /api/positions/:id/close-partial  # Partial close
```

**Analytics**
```
GET    /api/analytics/performance        # Portfolio performance
GET    /api/analytics/metrics            # Risk metrics
GET    /api/analytics/attribution        # P&L attribution
GET    /api/analytics/trades             # Trade statistics
```

**History**
```
GET    /api/history/orders               # Order history
GET    /api/history/positions            # Closed positions
GET    /api/history/trades               # Trade log
GET    /api/history/export               # Export data
```

**Strategies**
```
GET    /api/strategies                   # List strategies
POST   /api/strategies                   # Create strategy
GET    /api/strategies/:id               # Get strategy
PUT    /api/strategies/:id               # Update strategy
DELETE /api/strategies/:id               # Delete strategy
POST   /api/strategies/:id/backtest      # Run backtest
POST   /api/strategies/:id/activate      # Activate auto-trading
```

**Competitions**
```
GET    /api/competitions                 # List competitions
GET    /api/competitions/:id             # Competition details
POST   /api/competitions/:id/join        # Join competition
GET    /api/competitions/:id/leaderboard # Get leaderboard
```

#### WebSocket Events

**Outbound (Server → Client)**
```
OrderUpdate        # Order status change
PositionUpdate     # Position P&L update
PortfolioUpdate    # Portfolio balance/metrics
MarginUpdate       # Margin level changes
LiquidationWarning # Approaching liquidation
TradeExecution     # Trade filled
CompetitionUpdate  # Rank/score changes
```

**Inbound (Client → Server)**
```
SubscribePortfolio   # Subscribe to portfolio updates
UnsubscribePortfolio # Unsubscribe
SubscribePositions   # Subscribe to position updates
SubscribeOrders      # Subscribe to order updates
```

#### Webhooks
- User-configurable webhook URLs
- Event filtering (select which events)
- Retry logic with exponential backoff
- Webhook signature verification (HMAC)

---

## Technical Architecture

### Data Models

#### Portfolio
```rust
struct Portfolio {
    id: Uuid,
    user_id: Uuid,
    name: String,
    description: Option<String>,
    base_currency: String,           // USD, EUR, BTC
    starting_balance: Decimal,       // 5,000,000
    current_balance: Decimal,
    margin_used: Decimal,
    margin_available: Decimal,
    unrealized_pnl: Decimal,
    realized_pnl: Decimal,
    cost_basis_method: CostBasisMethod,
    risk_settings: RiskSettings,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    is_competition: bool,
    competition_id: Option<Uuid>,
}
```

#### Order
```rust
struct Order {
    id: Uuid,
    portfolio_id: Uuid,
    symbol: String,
    asset_type: AssetType,
    side: OrderSide,                 // Buy, Sell
    order_type: OrderType,
    quantity: Decimal,
    filled_quantity: Decimal,
    price: Option<Decimal>,          // For limit orders
    stop_price: Option<Decimal>,     // For stop orders
    trail_amount: Option<Decimal>,   // For trailing stops
    trail_percent: Option<Decimal>,
    time_in_force: TimeInForce,
    status: OrderStatus,
    linked_order_id: Option<Uuid>,   // For OCO
    bracket_id: Option<Uuid>,        // For bracket orders
    fills: Vec<Fill>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
}
```

#### Position
```rust
struct Position {
    id: Uuid,
    portfolio_id: Uuid,
    symbol: String,
    asset_type: AssetType,
    side: PositionSide,              // Long, Short
    quantity: Decimal,
    entry_price: Decimal,
    current_price: Decimal,
    unrealized_pnl: Decimal,
    realized_pnl: Decimal,
    margin_used: Decimal,
    leverage: Decimal,
    margin_mode: MarginMode,         // Isolated, Cross
    liquidation_price: Option<Decimal>,
    stop_loss: Option<Decimal>,
    take_profit: Option<Decimal>,
    cost_basis: Vec<CostBasisEntry>,
    funding_payments: Decimal,       // For perps
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}
```

#### Trade
```rust
struct Trade {
    id: Uuid,
    order_id: Uuid,
    portfolio_id: Uuid,
    symbol: String,
    side: OrderSide,
    quantity: Decimal,
    price: Decimal,
    fee: Decimal,
    slippage: Decimal,
    executed_at: DateTime<Utc>,
}
```

### Storage Architecture

#### SQLite (Persistent)
- Portfolios
- Orders (all)
- Positions (open and closed)
- Trades
- Strategies
- Competition results
- User achievements
- Tax records

#### Redis (Hot Data)
- Open orders (for fast matching)
- Real-time P&L calculations
- Margin levels
- Position summaries
- Leaderboard rankings
- Session data

#### In-Memory (DashMap)
- Active price triggers
- Order book cache
- WebSocket subscriptions
- Rate limiting

---

## Implementation Phases

### Phase 1: Core Foundation (Weeks 1-4)

**Deliverables:**
1. Portfolio data models and storage
2. Basic order types (Market, Limit, Stop Loss, Take Profit)
3. Spot trading (Crypto, Stocks, ETFs)
4. Position tracking and P&L calculation
5. Basic REST API endpoints
6. WebSocket position/order updates

**Database Schema:**
- portfolios table
- orders table
- positions table
- trades table

**API Endpoints:**
- Portfolio CRUD
- Order placement and management
- Position queries

**Testing:**
- Unit tests for order matching
- Integration tests for order flow
- API endpoint tests

---

### Phase 2: Advanced Orders & Perps (Weeks 5-8)

**Deliverables:**
1. Advanced order types (Trailing Stop, OCO, FOK, IOC, GTD, Bracket)
2. Perpetual futures support
3. Leverage and margin system
4. Liquidation engine
5. Funding rate calculation
6. Cross/Isolated margin modes

**New Tables:**
- funding_payments
- liquidations
- margin_history

**New Services:**
- MarginService
- LiquidationEngine
- FundingRateService

**Testing:**
- Margin calculation tests
- Liquidation scenario tests
- Funding rate tests

---

### Phase 3: Options & Auto-Trading (Weeks 9-12)

**Deliverables:**
1. Options trading support
2. Greeks calculation
3. Options chains integration
4. Multi-leg strategies
5. Signal-to-trade rule engine
6. Strategy builder
7. Backtesting engine

**New Tables:**
- options_positions
- strategies
- strategy_rules
- backtest_results

**New Services:**
- OptionsService
- GreeksCalculator
- StrategyEngine
- BacktestRunner

**Testing:**
- Options pricing tests
- Greeks accuracy tests
- Strategy execution tests
- Backtest validation

---

### Phase 4: Social & Analytics (Weeks 13-16)

**Deliverables:**
1. Competition platform
2. Leaderboards
3. Social features (follow, copy trading)
4. Achievements system
5. Comprehensive analytics
6. Tax reporting
7. Forex support

**New Tables:**
- competitions
- competition_entries
- achievements
- user_follows
- copy_trades

**New Services:**
- CompetitionService
- LeaderboardService
- CopyTradingService
- AnalyticsService
- TaxReportService

**Testing:**
- Competition logic tests
- Leaderboard ranking tests
- Copy trading tests
- Analytics accuracy tests

---

## API Documentation Requirements

Each endpoint must include:
1. **Description:** What the endpoint does
2. **Authentication:** Required auth level
3. **Request:** Parameters, body schema
4. **Response:** Success and error responses
5. **Examples:** curl examples and response samples
6. **Rate Limits:** Applicable limits

Documentation format: OpenAPI 3.0 specification
Documentation hosting: Extend existing /docs with Docsify

---

## Testing Strategy

### Unit Tests
- Order matching logic
- P&L calculations
- Margin calculations
- Greeks calculations
- Risk limit enforcement

### Integration Tests
- Full order lifecycle
- Position management
- Liquidation scenarios
- Funding payments
- Competition scoring

### End-to-End Tests
- User signup → portfolio creation → trading → analytics
- Competition participation flow
- Auto-trading activation

### Performance Tests
- Order throughput (target: 1000 orders/second)
- WebSocket update latency (target: <100ms)
- Backtest execution time

---

## Success Metrics

| Metric | Target |
|--------|--------|
| Order execution latency | < 100ms |
| P&L calculation accuracy | 99.99% |
| System uptime | 99.9% |
| API response time (p95) | < 200ms |
| WebSocket message latency | < 50ms |

---

## Dependencies

### Existing (Leverage)
- Authentication system
- Price aggregation pipeline
- Order book data
- Trading signals (13 indicators)
- WebSocket infrastructure
- SQLite + Redis storage

### New Requirements
- Options data source (Alpaca options API)
- Historical data for backtesting (extend CryptoCompare, Alpha Vantage)

### Forex Data Sources (Multi-Provider, Free-First)

| Provider | Free Tier | Priority | Notes |
|----------|-----------|----------|-------|
| **Alpha Vantage** | 25 req/day | Primary | Already integrated, extend for forex |
| **Finnhub** | 60 req/min | Primary | Already integrated, good forex coverage |
| **Twelve Data** | 800 req/day | Secondary | WebSocket support, 140+ currencies |
| **FCS API** | 500 req/day | Tertiary | 2000+ forex pairs, real-time |
| **Fixer.io** | 100 req/mo | Fallback | 170 currencies, rate-only fallback |

**Implementation Strategy:**
1. Extend existing Alpha Vantage source (`src/sources/alphavantage.rs`) for forex pairs
2. Extend existing Finnhub source (`src/sources/finnhub.rs`) for forex data
3. Add new Twelve Data source for WebSocket streaming and higher limits
4. Add FCS API as additional fallback
5. Use same multi-source aggregation pattern as crypto/stocks

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Complex margin calculation bugs | Medium | High | Extensive unit tests, shadow mode testing |
| Performance issues at scale | Medium | Medium | Load testing, Redis caching, query optimization |
| Data source reliability | Low | High | Multiple fallback sources, circuit breakers |
| Options pricing accuracy | Medium | Medium | Validate against real exchange prices |

---

## Resolved Decisions

| Question | Decision |
|----------|----------|
| Forex data source | Multi-provider: Alpha Vantage + Finnhub (existing) + Twelve Data + FCS API (free-first) |
| Competition prizes | Leaderboard placement only - no monetary rewards |
| Copy trading fees | 5% of profits default (configurable 0-25%), only on profitable trades |
| Options data | Alpaca options API for proof of concept, evaluate expansion later |
| Frontend/Mobile | Out of scope - backend API only |

## Open Questions

None - all major decisions resolved. Ready for implementation.

---

## Appendix

### A. Order Type Specifications

#### Market Order
- Executes immediately at best available price
- Uses order book for price discovery
- Slippage applied based on order size vs liquidity

#### Limit Order
- Placed in virtual order book
- Fills when market price crosses limit
- May partially fill

#### Stop Loss
- Triggers when last price ≤ stop price (for longs)
- Becomes market order when triggered
- Slippage applied on execution

#### Take Profit
- Triggers when last price ≥ target price (for longs)
- Becomes market order when triggered

#### Stop Limit
- Triggers when last price crosses stop price
- Becomes limit order (not market) when triggered
- May not fill if price moves past limit

#### Trailing Stop
- Stop price trails market by fixed amount or %
- Only moves in favorable direction
- Triggers like normal stop when hit

#### OCO (One-Cancels-Other)
- Two orders linked together
- When one fills or triggers, other is cancelled
- Common: Stop Loss + Take Profit

#### Fill or Kill
- Must fill entire quantity immediately
- If not possible, entire order cancelled
- No partial fills

#### Immediate or Cancel
- Fill whatever quantity available immediately
- Cancel any unfilled remainder
- Partial fills allowed

#### Bracket Order
- Entry order + Stop Loss + Take Profit as package
- SL and TP activate when entry fills
- SL and TP are OCO linked

### B. Margin Calculation Formulas

```
Initial Margin = Position Size / Leverage
Maintenance Margin = Position Size * Maintenance Rate

Margin Level = (Equity / Used Margin) * 100%
Equity = Balance + Unrealized P&L

Liquidation Price (Long) = Entry Price * (1 - Initial Margin + Maintenance Margin)
Liquidation Price (Short) = Entry Price * (1 + Initial Margin - Maintenance Margin)
```

### C. Greeks Formulas (Black-Scholes)

```
Delta (Call) = N(d1)
Delta (Put) = N(d1) - 1

Gamma = N'(d1) / (S * σ * √T)

Theta (Call) = -(S * N'(d1) * σ) / (2√T) - r * K * e^(-rT) * N(d2)
Theta (Put) = -(S * N'(d1) * σ) / (2√T) + r * K * e^(-rT) * N(-d2)

Vega = S * √T * N'(d1)

Where:
d1 = (ln(S/K) + (r + σ²/2)T) / (σ√T)
d2 = d1 - σ√T
N(x) = Standard normal CDF
N'(x) = Standard normal PDF
S = Spot price
K = Strike price
T = Time to expiration
r = Risk-free rate
σ = Volatility
```

---

## Implementation Progress Checklist

### Phase 1: Core Foundation

#### Data Models (`src/types/trading.rs`)
- [x] Portfolio struct with balance tracking
- [x] Order struct with all order types
- [x] Position struct with P&L tracking
- [x] Trade struct for execution records
- [x] RiskSettings struct
- [x] PortfolioSummary struct
- [x] AssetClass enum (CryptoSpot, Stock, Etf, Perp, Option, Forex)
- [x] OrderType enum (Market, Limit, StopLoss, TakeProfit, StopLimit, TrailingStop)
- [x] OrderStatus enum with full lifecycle
- [x] OrderSide and PositionSide enums
- [x] TimeInForce enum (GTC, IOC, FOK, GTD)
- [x] CostBasisMethod enum (FIFO, LIFO, Average)
- [x] Fill struct for partial fills
- [x] CostBasisEntry for tax tracking
- [x] Unit tests for all types (40 tests)

#### Database Schema (`src/services/sqlite_store.rs`)
- [x] portfolios table
- [x] orders table
- [x] positions table
- [x] trades table
- [x] Portfolio CRUD operations
- [x] Order CRUD operations
- [x] Position CRUD operations
- [x] Trade creation and queries
- [x] Database parsing for trading enums
- [x] Unit tests for database operations (9 tests)

#### Trading Service (`src/services/trading.rs`)
- [x] TradingService struct with DashMap caching
- [x] Portfolio management (create, get, update, delete, reset)
- [x] Order placement with validation
- [x] Market order execution with slippage simulation
- [x] Position creation and updates
- [x] P&L calculation (realized and unrealized)
- [x] Cost basis tracking (FIFO, LIFO, Average)
- [x] Leverage validation per asset class
- [x] Position limit enforcement
- [x] Stop loss and take profit triggers
- [x] Liquidation detection
- [x] ExecutionConfig for slippage/fee configuration
- [x] Unit tests for trading service (9 tests)

#### REST API (`src/api/trading.rs`)
- [x] `GET /api/trading/portfolios` - List portfolios
- [x] `POST /api/trading/portfolios` - Create portfolio
- [x] `GET /api/trading/portfolios/:id` - Get portfolio
- [x] `GET /api/trading/portfolios/:id/summary` - Get portfolio summary
- [x] `PUT /api/trading/portfolios/:id` - Update settings
- [x] `POST /api/trading/portfolios/:id/reset` - Reset portfolio
- [x] `DELETE /api/trading/portfolios/:id` - Delete portfolio
- [x] `GET /api/trading/orders` - List orders
- [x] `POST /api/trading/orders` - Place order
- [x] `GET /api/trading/orders/:id` - Get order
- [x] `DELETE /api/trading/orders/:id` - Cancel order
- [x] `GET /api/trading/positions` - List positions
- [x] `GET /api/trading/positions/:id` - Get position
- [x] `PUT /api/trading/positions/:id` - Modify position (SL/TP)
- [x] `DELETE /api/trading/positions/:id` - Close position
- [x] `GET /api/trading/trades` - Trade history
- [x] Error response handling with proper HTTP codes
- [x] Unit tests for serialization

#### WebSocket Updates (`src/types/ws.rs`, `src/websocket/`)
- [x] `SubscribeTrading` client message
- [x] `UnsubscribeTrading` client message
- [x] `TradingSubscribed` / `TradingUnsubscribed` confirmations
- [x] `OrderUpdate` server message
- [x] `PositionUpdate` server message
- [x] `PortfolioUpdate` server message
- [x] `TradeExecution` server message
- [x] `MarginWarning` server message
- [x] `LiquidationAlert` server message
- [x] OrderUpdateType enum (Created, PartialFill, Filled, Cancelled, etc.)
- [x] PositionUpdateType enum (Opened, Closed, Liquidated, etc.)
- [x] PortfolioUpdateType enum (BalanceChanged, Reset, etc.)
- [x] RoomManager trading subscription support
- [x] TradingService broadcast integration
- [x] WebSocket handler trading message processing
- [x] Unit tests for WebSocket types (12 tests)

#### Integration
- [x] AppState includes TradingService
- [x] TradingService integrated with RoomManager
- [x] API router includes trading routes
- [x] All tests passing (648 total)

---

### Phase 2: Advanced Orders & Perps

#### Advanced Order Types (`src/services/trading.rs`)
- [x] Trailing Stop execution logic (`update_trailing_stops`)
- [x] OCO (One-Cancels-Other) linked orders (`cancel_linked_order`, `place_oco_order`)
- [x] Fill or Kill (FOK) validation (`validate_fok_order`)
- [x] Immediate or Cancel (IOC) partial fills (`execute_ioc_order`)
- [x] Good Till Date (GTD) expiration (`expire_gtd_orders`)
- [x] Bracket Order (Entry + SL + TP package) (`place_bracket_order`, `activate_bracket_orders`)
- [ ] Order modification API

#### Perpetual Futures Types (`src/types/trading.rs`)
- [x] FundingRate struct with payment calculation
- [x] FundingPayment struct for tracking payments
- [x] LeverageTier struct with position-based limits
- [x] Position perp methods (apply_funding, leverage_tier, margin_level, warning_level)
- [x] Position leverage validation (`validate_leverage`)
- [x] Mark price support in Position
- [x] Isolated/Cross margin mode enum
- [x] funding_payments table (`src/services/sqlite_store.rs`)

#### Liquidation Engine Types (`src/types/trading.rs`)
- [x] LiquidationWarningLevel enum (80%, 90%, 95%, liquidation)
- [x] Liquidation struct with loss/fee tracking
- [x] MarginHistory struct for audit trail
- [x] MarginChangeType enum
- [x] InsuranceFund struct with contribution/payout tracking
- [x] AdlEntry struct with score calculation
- [x] liquidations table (`src/services/sqlite_store.rs`)
- [x] margin_history table (`src/services/sqlite_store.rs`)
- [x] insurance_fund table (`src/services/sqlite_store.rs`)

#### Services (`src/services/`)
- [x] LiquidationEngine (`src/services/liquidation.rs`)
  - [x] Margin level monitoring
  - [x] Gradual warning system (80%, 90%, 95%)
  - [x] Full and partial liquidation execution
  - [x] Insurance fund management
  - [x] Funding rate tracking and application
  - [x] ADL priority queue (placeholder)
  - [x] WebSocket alerts for warnings and liquidations
- [ ] FundingRateService (periodic funding rate fetching from exchanges)
- [ ] MarginService (cross margin management)

---

### Phase 3: Options & Auto-Trading

#### Options Trading Types (`src/types/trading.rs`)
- [x] OptionType enum (Call, Put)
- [x] OptionStyle enum (American, European)
- [x] Greeks struct (delta, gamma, theta, vega, rho)
- [x] OptionContract struct with pricing fields
- [x] OptionsChain struct for chain data
- [x] OptionPosition struct with Greeks tracking
- [x] OptionStrategyType enum (single, spreads, condors, etc.)
- [x] OptionStrategy struct for multi-leg strategies
- [x] Contract symbol generation

#### Options Service (`src/services/options.rs`)
- [x] Black-Scholes pricing for European options
- [x] Binomial tree pricing for American options
- [x] Greeks calculation (Delta, Gamma, Theta, Vega, Rho)
- [x] Implied volatility calculation (Newton-Raphson with bisection fallback)
- [x] Cumulative normal distribution functions
- [x] Position update with real-time Greeks

#### Options Database (`src/services/sqlite_store.rs`)
- [x] options_positions table with full schema
- [x] CRUD operations for option positions
- [x] Query by underlying symbol
- [x] Query expiring positions
- [x] Greeks JSON serialization
- [x] Unit tests for options position CRUD (4 tests)

#### Options Integration (Remaining)
- [ ] Options chains integration (Alpaca API)
- [ ] Multi-leg strategy execution
- [ ] Expiration handling
- [ ] Early exercise simulation
- [ ] Assignment simulation

#### Auto-Trading Types (`src/types/trading.rs`)
- [x] StrategyStatus enum (Active, Paused, Disabled, Deleted)
- [x] IndicatorType enum (RSI, MACD, EMA, SMA, Bollinger, ATR, ADX, etc.)
- [x] ComparisonOperator enum (LessThan, GreaterThan, CrossesAbove, etc.)
- [x] LogicalOperator enum (And, Or)
- [x] RuleActionType enum (MarketBuy, MarketSell, ClosePosition, etc.)
- [x] PositionSizeType enum (FixedAmount, PortfolioPercent, RiskPercent)
- [x] RuleCondition struct with indicator conditions
- [x] RuleAction struct with stop loss/take profit
- [x] TradingRule struct with multiple conditions
- [x] TradingStrategy struct with rules, cooldown, position limits
- [x] StrategySignal struct for rule triggers
- [x] Unit tests for all strategy types (12 tests)

#### Auto-Trading Database (`src/services/sqlite_store.rs`)
- [x] strategies table with rules JSON storage
- [x] CRUD operations for strategies
- [x] Query active strategies
- [x] Soft delete support
- [x] Unit tests for strategy CRUD (2 tests)

#### Auto-Trading Service (Completed)
- [x] StrategyEngine service for evaluating rules (`src/services/strategy_engine.rs`)
  - IndicatorSnapshot for tracking all 13 indicators
  - Cross detection (CrossesAbove/CrossesBelow) with previous value comparison
  - Rule condition evaluation with AND/OR operators
  - Signal-to-order conversion
  - Position count tracking per portfolio/symbol
  - Unit tests for condition evaluation (11 tests)
- [x] Signal-to-trade execution (signal_to_order_request method)
- [x] Real-time indicator value integration (IndicatorSnapshot)

#### Backtesting (Completed)
- [x] BacktestRunner service (`src/services/backtester.rs`)
  - Historical price simulation with configurable intervals
  - Commission and slippage simulation
  - Position sizing (FixedAmount, PortfolioPercent, RiskPercent, FixedUnits)
  - Unit tests (10 tests)
- [x] Historical data fetching (get_chart_data, synthetic data generation)
- [x] Equity curve generation (EquityPoint with sampling)
- [x] Trade-by-trade breakdown (BacktestTrade with excursion tracking)
- [x] Comparison to buy-and-hold (BuyAndHoldComparison)
- [x] Monte Carlo simulation (shuffled trade sequences, percentile analysis)
- [x] backtest_results table in SQLite

#### Backtest Types (`src/types/trading.rs`)
- [x] BacktestStatus enum (Pending, Running, Completed, Failed, Cancelled)
- [x] BacktestConfig with all configuration options
- [x] BacktestTrade for individual trade records
- [x] EquityPoint for equity curve
- [x] BacktestMetrics (30+ performance metrics)
- [x] BuyAndHoldComparison
- [x] MonteCarloResults
- [x] BacktestResult comprehensive result struct

#### Services (Status)
- [x] OptionsService (`src/services/options.rs`)
- [x] GreeksCalculator (Black-Scholes, binomial)
- [x] StrategyEngine (`src/services/strategy_engine.rs`)
- [x] BacktestRunner (`src/services/backtester.rs`)

---

### Phase 4: Social & Analytics

#### Competition Platform
- [ ] Competition struct and storage
- [ ] Competition types (Daily, Weekly, Monthly, Custom)
- [ ] Separate competition portfolios
- [ ] Entry requirements
- [ ] Leaderboard service
- [ ] competitions table
- [ ] competition_entries table

#### Social Features
- [ ] Follow traders
- [ ] Copy trading service
- [ ] Copy trading fee calculation (5% default)
- [ ] Public/private portfolio toggle
- [ ] Trader profiles
- [ ] user_follows table
- [ ] copy_trades table

#### Achievements
- [ ] Achievement definitions
- [ ] Badge awarding logic
- [ ] Achievement display
- [ ] achievements table

#### Analytics
- [ ] Comprehensive AnalyticsService
- [ ] Time-based returns (daily, weekly, monthly, etc.)
- [ ] Risk-adjusted metrics (Sharpe, Sortino, Calmar)
- [ ] Drawdown analysis
- [ ] Trade statistics (win rate, profit factor, etc.)
- [ ] Position attribution
- [ ] Beta & Alpha calculation

#### Forex Support
- [ ] Extend Alpha Vantage for forex
- [ ] Extend Finnhub for forex
- [ ] Add Twelve Data source
- [ ] Add FCS API source
- [ ] Forex pair validation
- [ ] Forex-specific leverage (50x)

#### Tax Reporting
- [ ] TaxReportService
- [ ] Realized gains/losses by year
- [ ] Cost basis method application
- [ ] Short-term vs long-term classification
- [ ] Export for tax software

#### Export & Reporting
- [ ] CSV export
- [ ] JSON export
- [ ] PDF report generation
- [ ] Trade journal with notes
- [ ] Screenshot attachments

---

### Testing Status

| Test Suite | Tests | Status |
|------------|-------|--------|
| Trading Types | 78 | ✅ Passing |
| SQLite Trading | 15 | ✅ Passing |
| Trading Service | 9 | ✅ Passing |
| WebSocket Trading | 12 | ✅ Passing |
| API Trading | 2 | ✅ Passing |
| Liquidation Engine | 3 | ✅ Passing |
| Options Service | 6 | ✅ Passing |
| Options Position DB | 4 | ✅ Passing |
| Strategy Types | 12 | ✅ Passing |
| Strategy DB | 2 | ✅ Passing |
| **Total Binary Tests** | **705** | ✅ **All Passing** |
| **Total Library Tests** | **262** | ✅ **All Passing** |

---

### Build Status

```
✅ Build successful with only expected unused code warnings
✅ No compilation errors
✅ All tests passing (967 total)
```

**Last Updated:** 2026-02-03
