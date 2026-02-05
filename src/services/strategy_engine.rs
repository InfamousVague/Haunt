//! Strategy Engine
//!
//! Evaluates trading strategies against real-time indicator values and generates
//! signals for trade execution.
//!
//! Features:
//! - Rule condition evaluation
//! - Support for all 13 indicators
//! - Cooldown management
//! - Position limit enforcement
//! - Integration with trading service

#![allow(dead_code)]

use crate::services::SqliteStore;
use crate::types::{
    ComparisonOperator, IndicatorType, LogicalOperator, PlaceOrderRequest, PositionSizeType,
    RuleAction, RuleActionType, RuleCondition, StrategySignal, StrategyStatus, TradingStrategy,
    AssetClass, OrderSide, OrderType,
};
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, info, warn};

/// Strategy engine errors.
#[derive(Debug, Error)]
pub enum StrategyError {
    #[error("Strategy not found: {0}")]
    StrategyNotFound(String),
    #[error("Strategy is not active: {0}")]
    StrategyNotActive(String),
    #[error("Strategy is in cooldown: {0}")]
    StrategyInCooldown(String),
    #[error("Position limit reached: {0}")]
    PositionLimitReached(String),
    #[error("Indicator value not available: {0}")]
    IndicatorNotAvailable(String),
    #[error("Database error: {0}")]
    DatabaseError(String),
}

/// Indicator values for a symbol at a point in time.
#[derive(Debug, Clone, Default)]
pub struct IndicatorSnapshot {
    /// Current price
    pub price: f64,
    /// RSI value (0-100)
    pub rsi: Option<f64>,
    /// MACD line value
    pub macd: Option<f64>,
    /// MACD signal line value
    pub macd_signal: Option<f64>,
    /// MACD histogram
    pub macd_histogram: Option<f64>,
    /// EMA values by period
    pub ema: HashMap<u32, f64>,
    /// SMA values by period
    pub sma: HashMap<u32, f64>,
    /// Bollinger upper band
    pub bollinger_upper: Option<f64>,
    /// Bollinger middle band
    pub bollinger_middle: Option<f64>,
    /// Bollinger lower band
    pub bollinger_lower: Option<f64>,
    /// ATR value
    pub atr: Option<f64>,
    /// ADX value
    pub adx: Option<f64>,
    /// Stochastic K value
    pub stochastic_k: Option<f64>,
    /// Stochastic D value
    pub stochastic_d: Option<f64>,
    /// OBV value
    pub obv: Option<f64>,
    /// VWAP value
    pub vwap: Option<f64>,
    /// CCI value
    pub cci: Option<f64>,
    /// MFI value
    pub mfi: Option<f64>,
    /// Previous values for cross detection
    pub previous: Option<Box<IndicatorSnapshot>>,
}

impl IndicatorSnapshot {
    /// Create a new snapshot with just price.
    pub fn new(price: f64) -> Self {
        Self {
            price,
            ..Default::default()
        }
    }

    /// Get indicator value by type and optional period.
    pub fn get_value(&self, indicator: IndicatorType, period: Option<u32>) -> Option<f64> {
        match indicator {
            IndicatorType::Price => Some(self.price),
            IndicatorType::Rsi => self.rsi,
            IndicatorType::Macd => self.macd,
            IndicatorType::Ema => period.and_then(|p| self.ema.get(&p).copied()),
            IndicatorType::Sma => period.and_then(|p| self.sma.get(&p).copied()),
            IndicatorType::Bollinger => self.bollinger_middle,
            IndicatorType::Atr => self.atr,
            IndicatorType::Adx => self.adx,
            IndicatorType::Stochastic => self.stochastic_k,
            IndicatorType::Obv => self.obv,
            IndicatorType::Vwap => self.vwap,
            IndicatorType::Cci => self.cci,
            IndicatorType::Mfi => self.mfi,
        }
    }

    /// Get previous indicator value for cross detection.
    pub fn get_previous_value(&self, indicator: IndicatorType, period: Option<u32>) -> Option<f64> {
        self.previous.as_ref().and_then(|prev| prev.get_value(indicator, period))
    }
}

/// Strategy engine for evaluating and executing trading strategies.
pub struct StrategyEngine {
    store: Arc<SqliteStore>,
    /// Cache of indicator snapshots by symbol
    snapshots: DashMap<String, IndicatorSnapshot>,
    /// Cache of open position counts by (portfolio_id, symbol)
    position_counts: DashMap<(String, String), u32>,
}

impl StrategyEngine {
    /// Create a new strategy engine.
    pub fn new(store: Arc<SqliteStore>) -> Self {
        Self {
            store,
            snapshots: DashMap::new(),
            position_counts: DashMap::new(),
        }
    }

    /// Update indicator snapshot for a symbol.
    pub fn update_snapshot(&self, symbol: &str, snapshot: IndicatorSnapshot) {
        let symbol_lower = symbol.to_lowercase();

        // Preserve previous snapshot for cross detection
        if let Some(existing) = self.snapshots.get(&symbol_lower) {
            let mut new_snapshot = snapshot;
            new_snapshot.previous = Some(Box::new(existing.clone()));
            self.snapshots.insert(symbol_lower, new_snapshot);
        } else {
            self.snapshots.insert(symbol_lower, snapshot);
        }
    }

    /// Get current snapshot for a symbol.
    pub fn get_snapshot(&self, symbol: &str) -> Option<IndicatorSnapshot> {
        self.snapshots.get(&symbol.to_lowercase()).map(|s| s.clone())
    }

    /// Update position count for a portfolio/symbol combination.
    pub fn update_position_count(&self, portfolio_id: &str, symbol: &str, count: u32) {
        self.position_counts.insert(
            (portfolio_id.to_string(), symbol.to_lowercase()),
            count,
        );
    }

    /// Get position count for a portfolio/symbol combination.
    pub fn get_position_count(&self, portfolio_id: &str, symbol: &str) -> u32 {
        self.position_counts
            .get(&(portfolio_id.to_string(), symbol.to_lowercase()))
            .map(|c| *c)
            .unwrap_or(0)
    }

    /// Evaluate all active strategies for a portfolio against current indicator values.
    /// Returns a list of signals that should be executed.
    pub fn evaluate_portfolio_strategies(
        &self,
        portfolio_id: &str,
    ) -> Vec<StrategySignal> {
        let strategies = self.store.get_active_strategies(portfolio_id);
        let mut signals = Vec::new();

        for strategy in strategies {
            if let Ok(strategy_signals) = self.evaluate_strategy(&strategy) {
                signals.extend(strategy_signals);
            }
        }

        signals
    }

    /// Evaluate a single strategy against current indicator values.
    pub fn evaluate_strategy(
        &self,
        strategy: &TradingStrategy,
    ) -> Result<Vec<StrategySignal>, StrategyError> {
        // Check if strategy is active
        if strategy.status != StrategyStatus::Active {
            return Err(StrategyError::StrategyNotActive(strategy.id.clone()));
        }

        // Check cooldown
        if strategy.is_in_cooldown() {
            debug!("Strategy {} is in cooldown", strategy.id);
            return Ok(Vec::new());
        }

        let mut signals = Vec::new();

        // Evaluate rules for each symbol
        for symbol in &strategy.symbols {
            // Get current snapshot for this symbol
            let snapshot = match self.get_snapshot(symbol) {
                Some(s) => s,
                None => {
                    debug!("No snapshot available for symbol {}", symbol);
                    continue;
                }
            };

            // Check position limits
            let current_positions = self.get_position_count(&strategy.portfolio_id, symbol);
            if current_positions >= strategy.max_positions {
                debug!(
                    "Position limit reached for strategy {} on {}",
                    strategy.id, symbol
                );
                continue;
            }

            // Evaluate each rule
            for rule in &strategy.rules {
                if !rule.enabled {
                    continue;
                }

                if self.evaluate_rule_conditions(&rule.conditions, rule.condition_operator, &snapshot) {
                    info!(
                        "Rule '{}' triggered for strategy '{}' on {}",
                        rule.name, strategy.name, symbol
                    );

                    let signal = StrategySignal::new(
                        strategy.id.clone(),
                        rule.id.clone(),
                        symbol.clone(),
                        rule.action.clone(),
                    );
                    signals.push(signal);

                    // Only trigger one rule per symbol per evaluation
                    break;
                }
            }
        }

        Ok(signals)
    }

    /// Evaluate a set of rule conditions.
    fn evaluate_rule_conditions(
        &self,
        conditions: &[RuleCondition],
        operator: LogicalOperator,
        snapshot: &IndicatorSnapshot,
    ) -> bool {
        if conditions.is_empty() {
            return false;
        }

        let results: Vec<bool> = conditions
            .iter()
            .map(|c| self.evaluate_condition(c, snapshot))
            .collect();

        match operator {
            LogicalOperator::And => results.iter().all(|&r| r),
            LogicalOperator::Or => results.iter().any(|&r| r),
        }
    }

    /// Evaluate a single condition.
    fn evaluate_condition(&self, condition: &RuleCondition, snapshot: &IndicatorSnapshot) -> bool {
        // Get current value
        let current_value = match snapshot.get_value(condition.indicator, condition.period) {
            Some(v) => v,
            None => {
                debug!("Indicator {:?} not available", condition.indicator);
                return false;
            }
        };

        // For cross comparisons, we need previous values
        match condition.operator {
            ComparisonOperator::CrossesAbove => {
                let prev_value = match snapshot.get_previous_value(condition.indicator, condition.period) {
                    Some(v) => v,
                    None => return false,
                };

                // Get the comparison value (either a fixed value or another indicator)
                let compare_value = if let Some(compare_ind) = condition.compare_indicator {
                    snapshot.get_value(compare_ind, condition.compare_period).unwrap_or(condition.value)
                } else {
                    condition.value
                };

                let prev_compare = if let Some(compare_ind) = condition.compare_indicator {
                    snapshot.get_previous_value(compare_ind, condition.compare_period).unwrap_or(condition.value)
                } else {
                    condition.value
                };

                // Crosses above: was below, now above
                prev_value <= prev_compare && current_value > compare_value
            }
            ComparisonOperator::CrossesBelow => {
                let prev_value = match snapshot.get_previous_value(condition.indicator, condition.period) {
                    Some(v) => v,
                    None => return false,
                };

                let compare_value = if let Some(compare_ind) = condition.compare_indicator {
                    snapshot.get_value(compare_ind, condition.compare_period).unwrap_or(condition.value)
                } else {
                    condition.value
                };

                let prev_compare = if let Some(compare_ind) = condition.compare_indicator {
                    snapshot.get_previous_value(compare_ind, condition.compare_period).unwrap_or(condition.value)
                } else {
                    condition.value
                };

                // Crosses below: was above, now below
                prev_value >= prev_compare && current_value < compare_value
            }
            ComparisonOperator::LessThan => current_value < condition.value,
            ComparisonOperator::LessThanOrEqual => current_value <= condition.value,
            ComparisonOperator::GreaterThan => current_value > condition.value,
            ComparisonOperator::GreaterThanOrEqual => current_value >= condition.value,
            ComparisonOperator::Equal => (current_value - condition.value).abs() < f64::EPSILON,
            ComparisonOperator::NotEqual => (current_value - condition.value).abs() >= f64::EPSILON,
        }
    }

    /// Convert a strategy signal into a place order request.
    pub fn signal_to_order_request(
        &self,
        signal: &StrategySignal,
        portfolio_value: f64,
        current_price: f64,
    ) -> PlaceOrderRequest {
        let action = &signal.action;

        // Calculate quantity based on size type
        let quantity = match action.size_type {
            PositionSizeType::FixedAmount => action.size_value / current_price,
            PositionSizeType::PortfolioPercent => {
                (portfolio_value * action.size_value / 100.0) / current_price
            }
            PositionSizeType::RiskPercent => {
                // Risk-based sizing requires stop loss
                if let Some(sl_pct) = action.stop_loss_pct {
                    let risk_amount = portfolio_value * action.size_value / 100.0;
                    let price_risk = current_price * sl_pct / 100.0;
                    risk_amount / price_risk
                } else {
                    // Default to portfolio percent if no stop loss
                    (portfolio_value * action.size_value / 100.0) / current_price
                }
            }
            PositionSizeType::FixedUnits => action.size_value,
        };

        // Determine order side
        let side = match action.action_type {
            RuleActionType::MarketBuy | RuleActionType::LimitBuy => OrderSide::Buy,
            RuleActionType::MarketSell | RuleActionType::LimitSell |
            RuleActionType::ClosePosition | RuleActionType::ClosePartial => OrderSide::Sell,
        };

        // Determine order type
        let order_type = match action.action_type {
            RuleActionType::MarketBuy | RuleActionType::MarketSell |
            RuleActionType::ClosePosition | RuleActionType::ClosePartial => OrderType::Market,
            RuleActionType::LimitBuy | RuleActionType::LimitSell => OrderType::Limit,
        };

        // Create order request
        PlaceOrderRequest {
            portfolio_id: String::new(), // Will be set by caller
            symbol: signal.symbol.clone(),
            asset_class: AssetClass::CryptoSpot, // Will be determined by caller
            side,
            order_type,
            quantity,
            price: if order_type == OrderType::Limit { Some(current_price) } else { None },
            stop_price: None,
            trail_amount: None,
            trail_percent: None,
            stop_loss: action.stop_loss_pct.map(|pct| current_price * (1.0 - pct / 100.0)),
            take_profit: action.take_profit_pct.map(|pct| current_price * (1.0 + pct / 100.0)),
            leverage: Some(action.leverage),
            time_in_force: None,
            client_order_id: Some(format!("strategy-{}-{}", signal.strategy_id, signal.rule_id)),
            bypass_drawdown: false,
            reduce_only: false,
            post_only: false,
            margin_mode: None,
        }
    }

    /// Record a trade execution for a strategy.
    pub fn record_trade_execution(
        &self,
        strategy_id: &str,
        is_profitable: bool,
        realized_pnl: f64,
    ) -> Result<(), StrategyError> {
        let mut strategy = self.store.get_strategy(strategy_id)
            .ok_or_else(|| StrategyError::StrategyNotFound(strategy_id.to_string()))?;

        strategy.record_trade(is_profitable, realized_pnl);

        self.store.update_strategy(&strategy)
            .map_err(|e| StrategyError::DatabaseError(e.to_string()))?;

        info!(
            "Recorded trade for strategy {}: profitable={}, pnl={}",
            strategy_id, is_profitable, realized_pnl
        );

        Ok(())
    }

    /// Get all strategies that are interested in a particular symbol.
    pub fn get_strategies_for_symbol(&self, symbol: &str) -> Vec<TradingStrategy> {
        // This would need to query all active strategies and filter by symbol
        // For now, we'll iterate through all portfolios - in production you'd want an index
        Vec::new() // Placeholder - would need portfolio list access
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{TradingRule, RuleCondition, RuleAction, PositionSizeType};

    fn setup_test_engine() -> StrategyEngine {
        let store = Arc::new(SqliteStore::new_in_memory().unwrap());
        StrategyEngine::new(store)
    }

    #[test]
    fn test_engine_creation() {
        let engine = setup_test_engine();
        assert!(engine.snapshots.is_empty());
    }

    #[test]
    fn test_snapshot_update() {
        let engine = setup_test_engine();

        let mut snapshot = IndicatorSnapshot::new(50000.0);
        snapshot.rsi = Some(25.0);
        engine.update_snapshot("BTC", snapshot);

        let retrieved = engine.get_snapshot("btc").unwrap();
        assert_eq!(retrieved.price, 50000.0);
        assert_eq!(retrieved.rsi, Some(25.0));
    }

    #[test]
    fn test_snapshot_previous_preservation() {
        let engine = setup_test_engine();

        // First snapshot
        let mut snapshot1 = IndicatorSnapshot::new(50000.0);
        snapshot1.rsi = Some(25.0);
        engine.update_snapshot("BTC", snapshot1);

        // Second snapshot
        let mut snapshot2 = IndicatorSnapshot::new(51000.0);
        snapshot2.rsi = Some(35.0);
        engine.update_snapshot("BTC", snapshot2);

        let retrieved = engine.get_snapshot("btc").unwrap();
        assert_eq!(retrieved.price, 51000.0);
        assert_eq!(retrieved.rsi, Some(35.0));
        assert!(retrieved.previous.is_some());
        assert_eq!(retrieved.previous.as_ref().unwrap().price, 50000.0);
        assert_eq!(retrieved.previous.as_ref().unwrap().rsi, Some(25.0));
    }

    #[test]
    fn test_condition_less_than() {
        let engine = setup_test_engine();

        let condition = RuleCondition::new(
            IndicatorType::Rsi,
            ComparisonOperator::LessThan,
            30.0,
        );

        // RSI = 25, should trigger (25 < 30)
        let mut snapshot = IndicatorSnapshot::new(50000.0);
        snapshot.rsi = Some(25.0);
        assert!(engine.evaluate_condition(&condition, &snapshot));

        // RSI = 35, should not trigger (35 < 30 is false)
        snapshot.rsi = Some(35.0);
        assert!(!engine.evaluate_condition(&condition, &snapshot));
    }

    #[test]
    fn test_condition_greater_than() {
        let engine = setup_test_engine();

        let condition = RuleCondition::new(
            IndicatorType::Rsi,
            ComparisonOperator::GreaterThan,
            70.0,
        );

        // RSI = 75, should trigger (75 > 70)
        let mut snapshot = IndicatorSnapshot::new(50000.0);
        snapshot.rsi = Some(75.0);
        assert!(engine.evaluate_condition(&condition, &snapshot));

        // RSI = 65, should not trigger (65 > 70 is false)
        snapshot.rsi = Some(65.0);
        assert!(!engine.evaluate_condition(&condition, &snapshot));
    }

    #[test]
    fn test_condition_crosses_above() {
        let engine = setup_test_engine();

        let condition = RuleCondition::new(
            IndicatorType::Rsi,
            ComparisonOperator::CrossesAbove,
            30.0,
        );

        // Create snapshot with previous below 30 and current above 30
        let mut prev = IndicatorSnapshot::new(49000.0);
        prev.rsi = Some(28.0);

        let mut snapshot = IndicatorSnapshot::new(50000.0);
        snapshot.rsi = Some(32.0);
        snapshot.previous = Some(Box::new(prev));

        assert!(engine.evaluate_condition(&condition, &snapshot));

        // Test no cross (both above)
        let mut prev2 = IndicatorSnapshot::new(49000.0);
        prev2.rsi = Some(35.0);

        let mut snapshot2 = IndicatorSnapshot::new(50000.0);
        snapshot2.rsi = Some(40.0);
        snapshot2.previous = Some(Box::new(prev2));

        assert!(!engine.evaluate_condition(&condition, &snapshot2));
    }

    #[test]
    fn test_rule_conditions_and() {
        let engine = setup_test_engine();

        let conditions = vec![
            RuleCondition::new(IndicatorType::Rsi, ComparisonOperator::LessThan, 30.0),
            RuleCondition::new(IndicatorType::Price, ComparisonOperator::GreaterThan, 40000.0),
        ];

        // Both conditions met
        let mut snapshot = IndicatorSnapshot::new(50000.0);
        snapshot.rsi = Some(25.0);
        assert!(engine.evaluate_rule_conditions(&conditions, LogicalOperator::And, &snapshot));

        // Only RSI condition met
        snapshot.rsi = Some(25.0);
        let snapshot2 = IndicatorSnapshot::new(30000.0); // Price not > 40000
        let mut snapshot2 = snapshot2;
        snapshot2.rsi = Some(25.0);
        assert!(!engine.evaluate_rule_conditions(&conditions, LogicalOperator::And, &snapshot2));
    }

    #[test]
    fn test_rule_conditions_or() {
        let engine = setup_test_engine();

        let conditions = vec![
            RuleCondition::new(IndicatorType::Rsi, ComparisonOperator::LessThan, 30.0),
            RuleCondition::new(IndicatorType::Price, ComparisonOperator::GreaterThan, 60000.0),
        ];

        // Only RSI condition met, OR should still pass
        let mut snapshot = IndicatorSnapshot::new(50000.0);
        snapshot.rsi = Some(25.0);
        assert!(engine.evaluate_rule_conditions(&conditions, LogicalOperator::Or, &snapshot));

        // Neither condition met
        let mut snapshot2 = IndicatorSnapshot::new(50000.0);
        snapshot2.rsi = Some(50.0);
        assert!(!engine.evaluate_rule_conditions(&conditions, LogicalOperator::Or, &snapshot2));
    }

    #[test]
    fn test_signal_to_order_portfolio_percent() {
        let engine = setup_test_engine();

        let action = RuleAction::market_buy(PositionSizeType::PortfolioPercent, 5.0)
            .with_stop_loss(3.0)
            .with_take_profit(6.0);

        let signal = StrategySignal::new(
            "strat1".to_string(),
            "rule1".to_string(),
            "BTC".to_string(),
            action,
        );

        let order = engine.signal_to_order_request(&signal, 100000.0, 50000.0);

        // 5% of 100000 = 5000, at 50000 price = 0.1 BTC
        assert!((order.quantity - 0.1).abs() < 0.0001);
        assert_eq!(order.side, OrderSide::Buy);
        assert_eq!(order.order_type, OrderType::Market);
        // Stop loss at 3% below: 50000 * 0.97 = 48500
        assert!((order.stop_loss.unwrap() - 48500.0).abs() < 0.01);
        // Take profit at 6% above: 50000 * 1.06 = 53000
        assert!((order.take_profit.unwrap() - 53000.0).abs() < 0.01);
    }

    #[test]
    fn test_signal_to_order_fixed_amount() {
        let engine = setup_test_engine();

        let action = RuleAction::market_buy(PositionSizeType::FixedAmount, 10000.0);

        let signal = StrategySignal::new(
            "strat1".to_string(),
            "rule1".to_string(),
            "ETH".to_string(),
            action,
        );

        let order = engine.signal_to_order_request(&signal, 100000.0, 2500.0);

        // $10000 at $2500 per ETH = 4 ETH
        assert!((order.quantity - 4.0).abs() < 0.0001);
    }

    #[test]
    fn test_position_count_tracking() {
        let engine = setup_test_engine();

        engine.update_position_count("port1", "BTC", 2);
        assert_eq!(engine.get_position_count("port1", "BTC"), 2);
        assert_eq!(engine.get_position_count("port1", "ETH"), 0);

        engine.update_position_count("port1", "BTC", 3);
        assert_eq!(engine.get_position_count("port1", "BTC"), 3);
    }
}
