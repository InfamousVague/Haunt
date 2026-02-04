# PaperBot Traders - AI Trading Agent System

This document outlines the plan for implementing AI-powered trading bots that compete on the leaderboard, providing users with bots to trade against and follow.

## Overview

Three distinct bot personalities will trade in real-time using the paper trading system:

| Bot | Personality | Strategy Style | Risk Profile |
|-----|-------------|----------------|--------------|
| **Grandma** | Conservative basic trader | Simple moving averages, buy-and-hold tendencies | Low risk, slow and steady |
| **Crypto Bro** | Enthusiastic but not technical | Momentum chasing, FOMO-driven, social sentiment | High risk, volatile |
| **Quant** | Data-driven ML trader | Multi-indicator fusion, online learning | Calculated risk, adaptive |

---

## Research Notes

### Strategy Performance (Documented Win Rates)

| Strategy | Win Rate | Notes |
|----------|----------|-------|
| RSI + MACD Combo | 73-80% | Confirmed oversold + bullish crossover signals |
| MACD Crossover Only | 60-65% | Better with volume confirmation |
| RSI Divergence | 55-60% | Works best in ranging markets |
| Simple MA Crossover | 50-55% | Many false signals, needs filtering |
| Momentum (3-12 month) | 55-65% | Well-documented in academic literature |
| Mean Reversion | 50-60% | Works in stable markets, fails in trends |

### Lightweight ML Approaches for Trading

1. **Online/Incremental Learning** - Update models with each new data point without storing history
   - Stochastic Gradient Descent (SGD) - O(1) memory per update
   - Online Random Forests - Fixed memory budget
   - Streaming K-means for regime detection

2. **Q-Learning for Trading** - Simple state-action-reward framework
   - State: Discretized indicators (RSI buckets, MACD signal, trend direction)
   - Actions: Buy, Sell, Hold
   - Reward: PnL from action
   - Memory: Only Q-table (~10KB for reasonable state space)

3. **Contextual Bandits** - Simpler than full RL, faster convergence
   - LinUCB algorithm for action selection
   - Thompson Sampling for exploration
   - Memory: O(features²) - very compact

### Rust Trading Bot Implementations (Reference)

- **tickgrinder** - High-frequency trading platform in Rust
- **rust_bt** - Backtesting framework with strategy composition
- **ta-rs** - Technical analysis library (we already use similar indicators)

### Memory-Efficient Architecture

Target: <50MB RAM per bot, <100MB storage for learning state

```
┌─────────────────────────────────────────────────────────┐
│                    Bot Runner Service                    │
├─────────────────────────────────────────────────────────┤
│  ┌───────────┐  ┌───────────┐  ┌───────────────────┐   │
│  │  Grandma  │  │Crypto Bro │  │       Quant       │   │
│  │  ~5MB RAM │  │  ~10MB RAM│  │     ~30MB RAM     │   │
│  └───────────┘  └───────────┘  └───────────────────┘   │
├─────────────────────────────────────────────────────────┤
│              Shared Market Data Stream                   │
│         (Price Cache, Signals, Order Book)              │
└─────────────────────────────────────────────────────────┘
```

---

## Implementation Plan

### Phase 1: Bot Framework Core

- [ ] **Create `src/services/paperbot/mod.rs`** - Bot framework module
  - [ ] Define `TradingBot` trait with required methods
  - [ ] Create `BotConfig` struct for configuration
  - [ ] Implement `BotRunner` service to manage all bots

- [ ] **Create `src/services/paperbot/portfolio.rs`** - Bot portfolio management
  - [ ] Each bot gets a dedicated paper trading portfolio
  - [ ] Track performance metrics separately
  - [ ] Implement position sizing logic

- [ ] **Create `src/services/paperbot/decision.rs`** - Decision engine
  - [ ] Define `TradeDecision` enum (Buy, Sell, Hold)
  - [ ] Create `DecisionContext` with market state
  - [ ] Implement confidence scoring

- [ ] **Integrate with existing systems**
  - [ ] Connect to `PriceCache` for real-time prices
  - [ ] Connect to `SignalStore` for indicator values
  - [ ] Connect to `OrderBookService` for liquidity data
  - [ ] Use `TradingService` to execute paper trades

### Phase 2: Grandma Bot (Simple Rule-Based)

Strategy: Conservative, long-term focused, simple indicators

- [ ] **Create `src/services/paperbot/grandma.rs`**
  - [ ] Implement `TradingBot` trait
  - [ ] Use 50/200 SMA crossover (golden/death cross)
  - [ ] Only trade when RSI confirms (not overbought/oversold)
  - [ ] Maximum 1 trade per day per asset
  - [ ] Position size: 5% of portfolio max

- [ ] **Grandma's Rules**
  ```
  BUY when:
    - 50 SMA crosses above 200 SMA (golden cross)
    - RSI < 70 (not overbought)
    - Price above 200 SMA

  SELL when:
    - 50 SMA crosses below 200 SMA (death cross)
    - OR RSI > 80 (take profits)
    - OR 10% stop loss hit

  HOLD otherwise (patience is a virtue)
  ```

- [ ] **Configuration**
  - [ ] Trade frequency: Max 1/day
  - [ ] Assets: BTC, ETH, major stocks only
  - [ ] Risk per trade: 2% of portfolio
  - [ ] Stop loss: 10%
  - [ ] Take profit: 20%

### Phase 3: Crypto Bro Bot (Momentum Chaser)

Strategy: Aggressive momentum, chases trends, high activity

- [ ] **Create `src/services/paperbot/crypto_bro.rs`**
  - [ ] Implement `TradingBot` trait
  - [ ] Use short-term momentum indicators (RSI, MACD)
  - [ ] React quickly to price movements
  - [ ] Higher position sizes, more frequent trades
  - [ ] FOMO logic: buy on breakouts

- [ ] **Crypto Bro's Rules**
  ```
  BUY when:
    - RSI crosses above 50 from below (momentum building)
    - MACD histogram turning positive
    - Price breaks above recent high (FOMO trigger)
    - Volume spike detected

  SELL when:
    - RSI > 75 (maybe take profits... or diamond hands?)
    - MACD bearish crossover
    - 5% stop loss (paper hands on losses)
    - OR randomly hold through dips (diamond hands mode)

  POSITION SIZE: 10-25% of portfolio (YOLO energy)
  ```

- [ ] **Personality Quirks**
  - [ ] 20% chance to "diamond hands" through stop loss
  - [ ] Occasional all-in trades on strong signals
  - [ ] Prefers crypto over stocks
  - [ ] More active during volatile periods

- [ ] **Configuration**
  - [ ] Trade frequency: Multiple per day
  - [ ] Assets: All crypto, meme stocks
  - [ ] Risk per trade: 5-15% of portfolio
  - [ ] Stop loss: 5% (sometimes ignored)
  - [ ] Take profit: Variable (greed factor)

### Phase 4: Quant Bot (ML-Powered)

Strategy: Data-driven, adaptive, multi-indicator fusion

- [ ] **Create `src/services/paperbot/quant/mod.rs`** - Quant module
  - [ ] Implement `TradingBot` trait
  - [ ] Multi-indicator feature extraction
  - [ ] Online learning decision engine
  - [ ] Risk-adjusted position sizing

- [ ] **Create `src/services/paperbot/quant/features.rs`** - Feature engineering
  - [ ] Extract features from all available indicators
  - [ ] Normalize features to [-1, 1] range
  - [ ] Calculate derived features (rate of change, volatility)
  - [ ] Regime detection (trending vs ranging)

- [ ] **Create `src/services/paperbot/quant/learner.rs`** - Online learning
  - [ ] Implement contextual bandit (LinUCB)
  - [ ] Action space: {Strong Buy, Buy, Hold, Sell, Strong Sell}
  - [ ] Update weights after each trade outcome
  - [ ] Exploration vs exploitation balance

- [ ] **Create `src/services/paperbot/quant/state.rs`** - Persistent state
  - [ ] Save/load model weights to SQLite
  - [ ] Track learning history
  - [ ] Store performance by regime/condition

- [ ] **Quant's Algorithm**
  ```
  FEATURES (normalized):
    - RSI (14) → [-1, 1]
    - MACD histogram → [-1, 1]
    - Bollinger Band position → [-1, 1]
    - Volume ratio (vs 20-period avg) → [0, 2]
    - Trend strength (ADX) → [0, 1]
    - Volatility (ATR %) → [0, 1]
    - Order book imbalance → [-1, 1]

  DECISION:
    action = LinUCB.select_action(features)
    confidence = model.confidence(features, action)

    if confidence > threshold:
      execute_trade(action, size=confidence * max_size)
    else:
      HOLD

  LEARNING:
    After trade closes:
      reward = risk_adjusted_return (Sharpe-like)
      model.update(features, action, reward)
  ```

- [ ] **Configuration**
  - [ ] Learning rate: 0.01 (slow adaptation)
  - [ ] Exploration: ε-greedy with decay
  - [ ] Min confidence to trade: 0.6
  - [ ] Position sizing: Kelly criterion capped at 10%
  - [ ] Max drawdown trigger: Reduce size after losses

### Phase 5: Bot Runner Service

- [ ] **Create `src/services/paperbot/runner.rs`** - Main runner
  - [ ] Spawn background task for each bot
  - [ ] Configurable tick interval (default: 1 minute)
  - [ ] Health monitoring and auto-restart
  - [ ] Performance metrics collection

- [ ] **Create `src/api/bots.rs`** - Bot API endpoints
  - [ ] `GET /api/bots` - List all bots with status
  - [ ] `GET /api/bots/:name` - Get bot details
  - [ ] `GET /api/bots/:name/trades` - Bot's trade history
  - [ ] `GET /api/bots/:name/performance` - Performance metrics
  - [ ] `POST /api/bots/:name/follow` - Follow a bot's trades

- [ ] **Bot Following Feature**
  - [ ] Users can "follow" a bot
  - [ ] Get notifications when bot trades
  - [ ] Optional: Auto-copy trades to user's portfolio
  - [ ] Leaderboard integration

### Phase 6: Learning & Adaptation

- [ ] **Implement feedback loop for Quant bot**
  - [ ] Track trade outcomes (win/loss, profit factor)
  - [ ] Update model weights based on results
  - [ ] Periodic model evaluation
  - [ ] Regime-specific model adjustments

- [ ] **Performance tracking**
  - [ ] Calculate Sharpe ratio per bot
  - [ ] Track max drawdown
  - [ ] Win rate by market condition
  - [ ] Compare bot performance over time

- [ ] **Model persistence**
  - [ ] Save Quant model state to SQLite
  - [ ] Checkpoint every 100 trades
  - [ ] Recovery from crashes
  - [ ] Model versioning

### Phase 7: Leaderboard Integration

- [ ] **Update leaderboard to include bots**
  - [ ] Bots appear with special badges
  - [ ] Users can see bot strategies
  - [ ] Filter: Users only, Bots only, All

- [ ] **Bot performance dashboard**
  - [ ] Real-time PnL charts
  - [ ] Trade frequency visualization
  - [ ] Learning progress for Quant

---

## Technical Specifications

### Memory Budget

| Component | Grandma | Crypto Bro | Quant |
|-----------|---------|------------|-------|
| Base runtime | 2 MB | 2 MB | 2 MB |
| Strategy state | 1 MB | 3 MB | 5 MB |
| Indicator cache | 2 MB | 5 MB | 10 MB |
| ML model | - | - | 10 MB |
| Trade history | 1 MB | 2 MB | 3 MB |
| **Total** | **~6 MB** | **~12 MB** | **~30 MB** |

### Decision Frequency

| Bot | Check Interval | Avg Trades/Day |
|-----|----------------|----------------|
| Grandma | 15 minutes | 0-2 |
| Crypto Bro | 1 minute | 5-20 |
| Quant | 5 minutes | 3-10 |

### Data Dependencies

All bots require:
- Real-time price data (PriceCache)
- Technical indicators (SignalStore)
- Order book data (OrderBookService)

Quant additionally requires:
- Historical indicator values (for feature engineering)
- Trade outcome history (for learning)

---

## File Structure

```
src/services/paperbot/
├── mod.rs           # Module exports, TradingBot trait
├── config.rs        # Bot configuration types
├── decision.rs      # TradeDecision, DecisionContext
├── portfolio.rs     # Bot portfolio management
├── runner.rs        # BotRunner service
├── grandma.rs       # Grandma bot implementation
├── crypto_bro.rs    # Crypto Bro bot implementation
└── quant/
    ├── mod.rs       # Quant bot main implementation
    ├── features.rs  # Feature extraction
    ├── learner.rs   # Online learning (LinUCB)
    └── state.rs     # Persistent state management
```

---

## Success Metrics

### Phase 1 Complete When:
- [ ] All three bots running and placing trades
- [ ] Bots appear on leaderboard
- [ ] API endpoints functional

### Phase 2 Complete When:
- [ ] Quant bot shows learning (improving Sharpe ratio)
- [ ] Bot following feature works
- [ ] Users can compare their performance to bots

### Long-term Goals:
- Quant bot achieves Sharpe ratio > 1.0
- Bots collectively generate interesting leaderboard dynamics
- Users learn from bot strategies

---

## References

- "Advances in Financial Machine Learning" - López de Prado
- "Machine Learning for Algorithmic Trading" - Jansen
- LinUCB: "A Contextual-Bandit Approach to Personalized News Article Recommendation"
- Online Learning: "Online Learning and Online Convex Optimization" - Shalev-Shwartz
