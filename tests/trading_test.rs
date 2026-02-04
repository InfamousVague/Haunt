//! Comprehensive tests for the paper trading system
//!
//! Tests cover:
//! - Portfolio management
//! - Order creation and execution
//! - Position tracking and P&L calculation
//! - Advanced order types
//! - Margin and leverage
//! - Liquidation
//! - Options
//! - Strategies
//! - Backtesting

use haunt::types::*;

// =============================================================================
// Portfolio Tests
// =============================================================================

mod portfolio_tests {
    use super::*;

    #[test]
    fn test_portfolio_creation() {
        let portfolio = Portfolio::new(
            "user-1".to_string(),
            "My Portfolio".to_string(),
        );

        assert!(!portfolio.id.is_empty());
        assert_eq!(portfolio.user_id, "user-1");
        assert_eq!(portfolio.name, "My Portfolio");
        assert_eq!(portfolio.starting_balance, 5_000_000.0);
        assert_eq!(portfolio.cash_balance, 5_000_000.0);
        assert_eq!(portfolio.realized_pnl, 0.0);
        assert_eq!(portfolio.unrealized_pnl, 0.0);
    }

    #[test]
    fn test_portfolio_equity() {
        let mut portfolio = Portfolio::new(
            "user-1".to_string(),
            "Test".to_string(),
        );
        portfolio.unrealized_pnl = 500.0;

        assert_eq!(portfolio.equity(), 5_000_500.0);
    }

    #[test]
    fn test_portfolio_margin_level() {
        let mut portfolio = Portfolio::new(
            "user-1".to_string(),
            "Test".to_string(),
        );
        portfolio.margin_used = 1_000_000.0;

        // Margin level = equity / margin_used * 100
        let margin_level = portfolio.margin_level();
        assert!(margin_level > 0.0);
    }
}

// =============================================================================
// Order Tests
// =============================================================================

mod order_tests {
    use super::*;

    #[test]
    fn test_market_order_creation() {
        let order = Order::market(
            "portfolio-1".to_string(),
            "BTC".to_string(),
            AssetClass::CryptoSpot,
            OrderSide::Buy,
            1.0,
        );

        assert!(!order.id.is_empty());
        assert_eq!(order.portfolio_id, "portfolio-1");
        assert_eq!(order.symbol, "BTC");
        assert_eq!(order.side, OrderSide::Buy);
        assert_eq!(order.order_type, OrderType::Market);
        assert_eq!(order.quantity, 1.0);
        assert_eq!(order.status, OrderStatus::Pending);
    }

    #[test]
    fn test_limit_order_creation() {
        let order = Order::limit(
            "portfolio-1".to_string(),
            "ETH".to_string(),
            AssetClass::CryptoSpot,
            OrderSide::Sell,
            10.0,
            3500.0,
        );

        assert_eq!(order.order_type, OrderType::Limit);
        assert_eq!(order.price, Some(3500.0));
    }

    #[test]
    fn test_order_type_serialization() {
        assert_eq!(serde_json::to_string(&OrderType::Market).unwrap(), "\"market\"");
        assert_eq!(serde_json::to_string(&OrderType::Limit).unwrap(), "\"limit\"");
        assert_eq!(serde_json::to_string(&OrderType::StopLoss).unwrap(), "\"stop_loss\"");
        assert_eq!(serde_json::to_string(&OrderType::TrailingStop).unwrap(), "\"trailing_stop\"");
    }

    #[test]
    fn test_order_status_serialization() {
        assert_eq!(serde_json::to_string(&OrderStatus::Pending).unwrap(), "\"pending\"");
        assert_eq!(serde_json::to_string(&OrderStatus::Open).unwrap(), "\"open\"");
        assert_eq!(serde_json::to_string(&OrderStatus::Filled).unwrap(), "\"filled\"");
        assert_eq!(serde_json::to_string(&OrderStatus::Cancelled).unwrap(), "\"cancelled\"");
    }

    #[test]
    fn test_time_in_force_serialization() {
        assert_eq!(serde_json::to_string(&TimeInForce::Gtc).unwrap(), "\"gtc\"");
        assert_eq!(serde_json::to_string(&TimeInForce::Fok).unwrap(), "\"fok\"");
        assert_eq!(serde_json::to_string(&TimeInForce::Ioc).unwrap(), "\"ioc\"");
        assert_eq!(serde_json::to_string(&TimeInForce::Gtd).unwrap(), "\"gtd\"");
    }

    #[test]
    fn test_order_remaining_quantity() {
        let mut order = Order::market(
            "portfolio-1".to_string(),
            "BTC".to_string(),
            AssetClass::CryptoSpot,
            OrderSide::Buy,
            10.0,
        );

        assert_eq!(order.remaining_quantity(), 10.0);

        order.filled_quantity = 3.0;
        assert_eq!(order.remaining_quantity(), 7.0);

        order.filled_quantity = 10.0;
        assert_eq!(order.remaining_quantity(), 0.0);
    }
}

// =============================================================================
// Position Tests
// =============================================================================

mod position_tests {
    use super::*;

    fn create_test_position() -> Position {
        Position::new(
            "portfolio-1".to_string(),
            "BTC".to_string(),
            AssetClass::CryptoSpot,
            PositionSide::Long,
            1.0,
            50000.0,
            1.0, // leverage
        )
    }

    #[test]
    fn test_position_creation() {
        let position = create_test_position();

        assert!(!position.id.is_empty());
        assert_eq!(position.portfolio_id, "portfolio-1");
        assert_eq!(position.symbol, "BTC");
        assert_eq!(position.side, PositionSide::Long);
        assert_eq!(position.quantity, 1.0);
        assert_eq!(position.entry_price, 50000.0);
        assert_eq!(position.current_price, 50000.0);
    }

    #[test]
    fn test_position_notional_value() {
        let position = create_test_position();
        assert_eq!(position.notional_value(), 50000.0);
    }

    #[test]
    fn test_position_update_price_long() {
        let mut position = create_test_position();
        position.update_price(55000.0);

        assert_eq!(position.current_price, 55000.0);
        assert_eq!(position.unrealized_pnl, 5000.0);
        assert!((position.unrealized_pnl_pct - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_position_update_price_short() {
        let mut position = Position::new(
            "portfolio-1".to_string(),
            "ETH".to_string(),
            AssetClass::CryptoSpot,
            PositionSide::Short,
            10.0,
            3000.0,
            1.0,
        );

        position.update_price(2700.0);

        // Short position profits when price goes down
        assert_eq!(position.unrealized_pnl, 3000.0); // 10 * (3000 - 2700)
        assert!((position.unrealized_pnl_pct - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_position_with_leverage() {
        let position = Position::new(
            "portfolio-1".to_string(),
            "BTC".to_string(),
            AssetClass::CryptoSpot,
            PositionSide::Long,
            1.0,
            50000.0,
            10.0, // 10x leverage
        );

        // Margin = notional / leverage = 50000 / 10 = 5000
        assert_eq!(position.margin_used, 5000.0);
    }

    #[test]
    fn test_position_margin_level() {
        let mut position = Position::new(
            "portfolio-1".to_string(),
            "BTC".to_string(),
            AssetClass::CryptoSpot,
            PositionSide::Long,
            1.0,
            50000.0,
            10.0,
        );

        position.update_price(55000.0); // 10% profit
        let margin_level = position.margin_level();
        // With profit, margin level should be > 100%
        assert!(margin_level > 100.0);
    }

    #[test]
    fn test_position_side_serialization() {
        assert_eq!(serde_json::to_string(&PositionSide::Long).unwrap(), "\"long\"");
        assert_eq!(serde_json::to_string(&PositionSide::Short).unwrap(), "\"short\"");
    }
}

// =============================================================================
// Trade Tests
// =============================================================================

mod trade_tests {
    use super::*;

    #[test]
    fn test_trade_creation() {
        let trade = Trade::new(
            "order-1".to_string(),
            "portfolio-1".to_string(),
            "BTC".to_string(),
            AssetClass::CryptoSpot,
            OrderSide::Buy,
            1.0,
            50000.0,
            50.0,
            0.0, // slippage
        );

        assert!(!trade.id.is_empty());
        assert_eq!(trade.portfolio_id, "portfolio-1");
        assert_eq!(trade.order_id, "order-1");
        assert_eq!(trade.symbol, "BTC");
        assert_eq!(trade.side, OrderSide::Buy);
        assert_eq!(trade.quantity, 1.0);
        assert_eq!(trade.price, 50000.0);
        assert_eq!(trade.fee, 50.0);
        assert_eq!(trade.total_cost(), 50050.0);
    }

    #[test]
    fn test_trade_total_cost_buy() {
        let trade = Trade::new(
            "order-1".to_string(),
            "portfolio-1".to_string(),
            "ETH".to_string(),
            AssetClass::CryptoSpot,
            OrderSide::Buy,
            10.0,
            3000.0,
            30.0,
            0.0,
        );

        // Total cost for buy = quantity * price + fee
        assert_eq!(trade.total_cost(), 30030.0);
    }

    #[test]
    fn test_trade_total_cost_sell() {
        let trade = Trade::new(
            "order-1".to_string(),
            "portfolio-1".to_string(),
            "ETH".to_string(),
            AssetClass::CryptoSpot,
            OrderSide::Sell,
            10.0,
            3000.0,
            30.0,
            0.0,
        );

        // Total cost = quantity * price + fee (same for buy and sell)
        assert_eq!(trade.total_cost(), 30030.0);
    }
}

// =============================================================================
// Asset Class Tests
// =============================================================================

mod asset_class_tests {
    use super::*;

    #[test]
    fn test_asset_class_max_leverage() {
        assert_eq!(AssetClass::CryptoSpot.max_leverage(), 10.0);
        assert_eq!(AssetClass::Stock.max_leverage(), 4.0);
        assert_eq!(AssetClass::Etf.max_leverage(), 4.0);
        assert_eq!(AssetClass::Perp.max_leverage(), 100.0);
        assert_eq!(AssetClass::Option.max_leverage(), 1.0);
        assert_eq!(AssetClass::Forex.max_leverage(), 50.0);
    }

    #[test]
    fn test_asset_class_initial_margin() {
        assert_eq!(AssetClass::CryptoSpot.initial_margin(), 0.10);
        assert_eq!(AssetClass::Stock.initial_margin(), 0.25);
        assert_eq!(AssetClass::Perp.initial_margin(), 0.01);
    }

    #[test]
    fn test_asset_class_maintenance_margin() {
        assert_eq!(AssetClass::CryptoSpot.maintenance_margin(), 0.05);
        assert_eq!(AssetClass::Stock.maintenance_margin(), 0.25); // 25%
        assert_eq!(AssetClass::Perp.maintenance_margin(), 0.005);
    }

    #[test]
    fn test_asset_class_serialization() {
        assert_eq!(serde_json::to_string(&AssetClass::CryptoSpot).unwrap(), "\"crypto_spot\"");
        assert_eq!(serde_json::to_string(&AssetClass::Stock).unwrap(), "\"stock\"");
        assert_eq!(serde_json::to_string(&AssetClass::Perp).unwrap(), "\"perp\"");
        assert_eq!(serde_json::to_string(&AssetClass::Option).unwrap(), "\"option\"");

        let parsed: AssetClass = serde_json::from_str("\"forex\"").unwrap();
        assert_eq!(parsed, AssetClass::Forex);
    }
}

// =============================================================================
// Margin Tests
// =============================================================================

mod margin_tests {
    use super::*;

    #[test]
    fn test_margin_mode_serialization() {
        assert_eq!(serde_json::to_string(&MarginMode::Isolated).unwrap(), "\"isolated\"");
        assert_eq!(serde_json::to_string(&MarginMode::Cross).unwrap(), "\"cross\"");
    }

    #[test]
    fn test_margin_change_type_serialization() {
        assert_eq!(
            serde_json::to_string(&MarginChangeType::PositionOpened).unwrap(),
            "\"position_opened\""
        );
        assert_eq!(
            serde_json::to_string(&MarginChangeType::Liquidation).unwrap(),
            "\"liquidation\""
        );
    }

    #[test]
    fn test_liquidation_warning_levels() {
        assert_eq!(LiquidationWarningLevel::Warning80.margin_level_threshold(), 125.0);
        assert_eq!(LiquidationWarningLevel::Warning90.margin_level_threshold(), 111.0);
        assert_eq!(LiquidationWarningLevel::Warning95.margin_level_threshold(), 105.0);
        assert_eq!(LiquidationWarningLevel::Liquidation.margin_level_threshold(), 100.0);
    }

    #[test]
    fn test_margin_history_creation() {
        let history = MarginHistory::new(
            "portfolio-1".to_string(),
            Some("position-1".to_string()),
            MarginChangeType::PositionOpened,
            0.0,
            150.0,
            0.0,
            5000.0,
            Some("Opened 1 BTC long".to_string()),
        );

        assert!(!history.id.is_empty());
        assert_eq!(history.portfolio_id, "portfolio-1");
        assert_eq!(history.position_id, Some("position-1".to_string()));
        assert_eq!(history.previous_margin_level, 0.0);
        assert_eq!(history.new_margin_level, 150.0);
    }
}

// =============================================================================
// Liquidation Tests
// =============================================================================

mod liquidation_tests {
    use super::*;

    #[test]
    fn test_liquidation_creation() {
        let liquidation = Liquidation::new(
            "position-1".to_string(),
            "portfolio-1".to_string(),
            "BTC".to_string(),
            1.0,
            45000.0,
            45100.0,
            50000.0,
            PositionSide::Long,
            false,
            None,
        );

        assert!(!liquidation.id.is_empty());
        assert_eq!(liquidation.position_id, "position-1");
        assert_eq!(liquidation.symbol, "BTC");
        assert_eq!(liquidation.quantity, 1.0);
        assert_eq!(liquidation.liquidation_price, 45000.0);
        assert!(!liquidation.is_partial);
    }

    #[test]
    fn test_liquidation_loss_calculation() {
        let liquidation = Liquidation::new(
            "position-1".to_string(),
            "portfolio-1".to_string(),
            "BTC".to_string(),
            1.0,
            45000.0,
            45000.0,
            50000.0,
            PositionSide::Long,
            false,
            None,
        );

        // Loss = (entry - liquidation) * quantity for long
        // 50000 - 45000 = 5000
        assert_eq!(liquidation.loss, 5000.0);
    }

    #[test]
    fn test_insurance_fund() {
        let mut fund = InsuranceFund::default();
        assert_eq!(fund.balance, 0.0);

        fund.add_contribution(1000.0);
        assert_eq!(fund.balance, 1000.0);
        assert_eq!(fund.total_contributions, 1000.0);

        let covered = fund.cover_loss(300.0);
        assert_eq!(covered, 300.0);
        assert_eq!(fund.balance, 700.0);
        assert_eq!(fund.total_payouts, 300.0);
        assert_eq!(fund.liquidations_covered, 1);
    }

    #[test]
    fn test_insurance_fund_insufficient() {
        let mut fund = InsuranceFund::default();
        fund.add_contribution(500.0);

        let covered = fund.cover_loss(800.0);
        assert_eq!(covered, 500.0); // Only covers what's available
        assert_eq!(fund.balance, 0.0);
    }
}

// =============================================================================
// Options Tests
// =============================================================================

mod options_tests {
    use super::*;

    #[test]
    fn test_option_type_serialization() {
        assert_eq!(serde_json::to_string(&OptionType::Call).unwrap(), "\"call\"");
        assert_eq!(serde_json::to_string(&OptionType::Put).unwrap(), "\"put\"");
    }

    #[test]
    fn test_option_style_serialization() {
        assert_eq!(serde_json::to_string(&OptionStyle::European).unwrap(), "\"european\"");
        assert_eq!(serde_json::to_string(&OptionStyle::American).unwrap(), "\"american\"");
    }

    #[test]
    fn test_option_contract_creation() {
        let contract = OptionContract::new(
            "BTC".to_string(),
            OptionType::Call,
            50000.0,
            chrono::Utc::now().timestamp_millis() + 86400000, // 1 day
            OptionStyle::European,
        );

        assert!(!contract.contract_symbol.is_empty());
        assert_eq!(contract.underlying_symbol, "BTC");
        assert_eq!(contract.option_type, OptionType::Call);
        assert_eq!(contract.strike, 50000.0);
        assert_eq!(contract.style, OptionStyle::European);
    }

    #[test]
    fn test_option_contract_expiration() {
        let future_expiry = chrono::Utc::now().timestamp_millis() + 86400000;
        let contract = OptionContract::new(
            "ETH".to_string(),
            OptionType::Put,
            3000.0,
            future_expiry,
            OptionStyle::American,
        );

        assert!(!contract.is_expired());

        // Create expired contract
        let past_expiry = chrono::Utc::now().timestamp_millis() - 86400000;
        let expired_contract = OptionContract::new(
            "ETH".to_string(),
            OptionType::Put,
            3000.0,
            past_expiry,
            OptionStyle::American,
        );

        assert!(expired_contract.is_expired());
    }

    #[test]
    fn test_option_contract_itm() {
        let call = OptionContract::new(
            "BTC".to_string(),
            OptionType::Call,
            50000.0,
            chrono::Utc::now().timestamp_millis() + 86400000,
            OptionStyle::European,
        );

        assert!(call.is_itm(55000.0)); // Price above strike = call ITM
        assert!(!call.is_itm(45000.0)); // Price below strike = call OTM

        let put = OptionContract::new(
            "BTC".to_string(),
            OptionType::Put,
            50000.0,
            chrono::Utc::now().timestamp_millis() + 86400000,
            OptionStyle::European,
        );

        assert!(put.is_itm(45000.0)); // Price below strike = put ITM
        assert!(!put.is_itm(55000.0)); // Price above strike = put OTM
    }

    #[test]
    fn test_greeks_default() {
        let greeks = Greeks::default();
        assert_eq!(greeks.delta, 0.0);
        assert_eq!(greeks.gamma, 0.0);
        assert_eq!(greeks.theta, 0.0);
        assert_eq!(greeks.vega, 0.0);
        assert_eq!(greeks.rho, 0.0);
    }

    #[test]
    fn test_option_position_creation() {
        let contract = OptionContract::new(
            "BTC".to_string(),
            OptionType::Call,
            50000.0,
            chrono::Utc::now().timestamp_millis() + 2592000000, // 30 days
            OptionStyle::European,
        );

        let position = OptionPosition::new(
            "portfolio-1".to_string(),
            &contract,
            10,
            1500.0,
        );

        assert!(!position.id.is_empty());
        assert_eq!(position.contracts, 10);
        assert_eq!(position.multiplier, 100);
        assert_eq!(position.entry_premium, 1500.0);
        assert_eq!(position.current_premium, 1500.0);
    }

    #[test]
    fn test_option_position_long_short() {
        let contract = OptionContract::new(
            "ETH".to_string(),
            OptionType::Put,
            3000.0,
            chrono::Utc::now().timestamp_millis() + 2592000000,
            OptionStyle::European,
        );

        let long_position = OptionPosition::new(
            "portfolio-1".to_string(),
            &contract,
            5,
            200.0,
        );
        assert!(long_position.is_long());
        assert!(!long_position.is_short());

        let short_position = OptionPosition::new(
            "portfolio-1".to_string(),
            &contract,
            -5,
            200.0,
        );
        assert!(!short_position.is_long());
        assert!(short_position.is_short());
    }
}

// =============================================================================
// Strategy Tests
// =============================================================================

mod strategy_tests {
    use super::*;

    #[test]
    fn test_strategy_status_serialization() {
        assert_eq!(serde_json::to_string(&StrategyStatus::Active).unwrap(), "\"active\"");
        assert_eq!(serde_json::to_string(&StrategyStatus::Paused).unwrap(), "\"paused\"");
        assert_eq!(serde_json::to_string(&StrategyStatus::Disabled).unwrap(), "\"disabled\"");
        assert_eq!(serde_json::to_string(&StrategyStatus::Deleted).unwrap(), "\"deleted\"");
    }

    #[test]
    fn test_indicator_type_serialization() {
        assert_eq!(serde_json::to_string(&IndicatorType::Rsi).unwrap(), "\"rsi\"");
        assert_eq!(serde_json::to_string(&IndicatorType::Macd).unwrap(), "\"macd\"");
        assert_eq!(serde_json::to_string(&IndicatorType::Ema).unwrap(), "\"ema\"");
        assert_eq!(serde_json::to_string(&IndicatorType::Bollinger).unwrap(), "\"bollinger\"");
    }

    #[test]
    fn test_comparison_operator_serialization() {
        assert_eq!(serde_json::to_string(&ComparisonOperator::LessThan).unwrap(), "\"less_than\"");
        assert_eq!(serde_json::to_string(&ComparisonOperator::GreaterThan).unwrap(), "\"greater_than\"");
        assert_eq!(serde_json::to_string(&ComparisonOperator::CrossesAbove).unwrap(), "\"crosses_above\"");
        assert_eq!(serde_json::to_string(&ComparisonOperator::CrossesBelow).unwrap(), "\"crosses_below\"");
    }

    #[test]
    fn test_rule_action_type_serialization() {
        assert_eq!(serde_json::to_string(&RuleActionType::MarketBuy).unwrap(), "\"market_buy\"");
        assert_eq!(serde_json::to_string(&RuleActionType::MarketSell).unwrap(), "\"market_sell\"");
        assert_eq!(serde_json::to_string(&RuleActionType::ClosePosition).unwrap(), "\"close_position\"");
    }

    #[test]
    fn test_position_size_type_serialization() {
        assert_eq!(serde_json::to_string(&PositionSizeType::FixedAmount).unwrap(), "\"fixed_amount\"");
        assert_eq!(serde_json::to_string(&PositionSizeType::PortfolioPercent).unwrap(), "\"portfolio_percent\"");
        assert_eq!(serde_json::to_string(&PositionSizeType::RiskPercent).unwrap(), "\"risk_percent\"");
        assert_eq!(serde_json::to_string(&PositionSizeType::FixedUnits).unwrap(), "\"fixed_units\"");
    }

    #[test]
    fn test_rule_condition_creation() {
        let condition = RuleCondition {
            id: "cond-1".to_string(),
            indicator: IndicatorType::Rsi,
            period: Some(14),
            operator: ComparisonOperator::LessThan,
            value: 30.0,
            compare_indicator: None,
            compare_period: None,
        };

        assert_eq!(condition.indicator, IndicatorType::Rsi);
        assert_eq!(condition.period, Some(14));
        assert_eq!(condition.operator, ComparisonOperator::LessThan);
        assert!((condition.value - 30.0).abs() < 0.01);
    }

    #[test]
    fn test_trading_strategy_creation() {
        let strategy = TradingStrategy::new(
            "portfolio-1".to_string(),
            "RSI Oversold".to_string(),
            vec!["BTC".to_string(), "ETH".to_string()],
        );

        assert!(!strategy.id.is_empty());
        assert_eq!(strategy.portfolio_id, "portfolio-1");
        assert_eq!(strategy.name, "RSI Oversold");
        assert_eq!(strategy.symbols.len(), 2);
        assert_eq!(strategy.status, StrategyStatus::Paused);
        assert_eq!(strategy.total_trades, 0);
    }

    #[test]
    fn test_trading_strategy_win_rate() {
        let mut strategy = TradingStrategy::new(
            "portfolio-1".to_string(),
            "Test".to_string(),
            vec!["BTC".to_string()],
        );

        assert_eq!(strategy.win_rate(), 0.0);

        strategy.total_trades = 10;
        strategy.winning_trades = 6;
        strategy.losing_trades = 4;

        assert!((strategy.win_rate() - 0.6).abs() < 0.01);
    }

    #[test]
    fn test_trading_strategy_record_trade() {
        let mut strategy = TradingStrategy::new(
            "portfolio-1".to_string(),
            "Test".to_string(),
            vec!["BTC".to_string()],
        );

        strategy.record_trade(true, 100.0);
        assert_eq!(strategy.total_trades, 1);
        assert_eq!(strategy.winning_trades, 1);
        assert_eq!(strategy.realized_pnl, 100.0);

        strategy.record_trade(false, -50.0);
        assert_eq!(strategy.total_trades, 2);
        assert_eq!(strategy.losing_trades, 1);
        assert_eq!(strategy.realized_pnl, 50.0);
    }

    #[test]
    fn test_trading_strategy_cooldown() {
        let mut strategy = TradingStrategy::new(
            "portfolio-1".to_string(),
            "Test".to_string(),
            vec!["BTC".to_string()],
        );

        strategy.cooldown_seconds = 60;
        assert!(!strategy.is_in_cooldown());

        strategy.last_trade_at = Some(chrono::Utc::now().timestamp_millis());
        assert!(strategy.is_in_cooldown());
    }

    #[test]
    fn test_trading_strategy_activate() {
        let mut strategy = TradingStrategy::new(
            "portfolio-1".to_string(),
            "Test".to_string(),
            vec!["BTC".to_string()],
        );

        assert_eq!(strategy.status, StrategyStatus::Paused);

        strategy.activate();
        assert_eq!(strategy.status, StrategyStatus::Active);

        strategy.pause();
        assert_eq!(strategy.status, StrategyStatus::Paused);
    }
}

// =============================================================================
// Backtest Tests
// =============================================================================

mod backtest_tests {
    use super::*;

    #[test]
    fn test_backtest_status_serialization() {
        assert_eq!(serde_json::to_string(&BacktestStatus::Pending).unwrap(), "\"pending\"");
        assert_eq!(serde_json::to_string(&BacktestStatus::Running).unwrap(), "\"running\"");
        assert_eq!(serde_json::to_string(&BacktestStatus::Completed).unwrap(), "\"completed\"");
        assert_eq!(serde_json::to_string(&BacktestStatus::Failed).unwrap(), "\"failed\"");
    }

    #[test]
    fn test_backtest_config_creation() {
        let start = 1704067200000; // 2024-01-01
        let end = 1706745600000;   // 2024-02-01

        let config = BacktestConfig::new("strategy-1".to_string(), start, end);

        assert_eq!(config.strategy_id, "strategy-1");
        assert_eq!(config.start_time, start);
        assert_eq!(config.end_time, end);
        assert_eq!(config.initial_balance, 10_000.0);
        assert_eq!(config.commission_rate, 0.001);
        assert!(config.use_ohlc);
    }

    #[test]
    fn test_backtest_config_duration() {
        let start = 1704067200000;
        let end = 1706745600000;
        let config = BacktestConfig::new("strategy-1".to_string(), start, end);

        let duration_days = config.duration_days();
        assert!(duration_days > 30.0);
        assert!(duration_days < 32.0);
    }

    #[test]
    fn test_backtest_trade_open() {
        let trade = BacktestTrade::open(
            "BTC".to_string(),
            OrderSide::Buy,
            50000.0,
            0.1,
            1704067200000,
            Some("rule-1".to_string()),
            5.0,
        );

        assert!(trade.is_open());
        assert_eq!(trade.symbol, "BTC");
        assert_eq!(trade.entry_price, 50000.0);
        assert_eq!(trade.quantity, 0.1);
        assert_eq!(trade.pnl, -5.0); // Just commission
        assert!(!trade.is_winner);
    }

    #[test]
    fn test_backtest_trade_close() {
        let mut trade = BacktestTrade::open(
            "BTC".to_string(),
            OrderSide::Buy,
            50000.0,
            0.1,
            1704067200000,
            None,
            5.0,
        );

        trade.close(52000.0, 1704153600000, Some("rule-2".to_string()), 5.2);

        assert!(!trade.is_open());
        assert_eq!(trade.exit_price, Some(52000.0));
        // Gross P&L: (52000 - 50000) * 0.1 = 200
        // Net P&L: 200 - 5.0 - 5.2 = 189.8
        assert!((trade.pnl - 189.8).abs() < 0.01);
        assert!(trade.is_winner);
    }

    #[test]
    fn test_backtest_trade_duration() {
        let mut trade = BacktestTrade::open(
            "ETH".to_string(),
            OrderSide::Buy,
            3000.0,
            1.0,
            1000000,
            None,
            0.0,
        );

        assert!(trade.duration_ms().is_none());

        trade.close(3100.0, 2000000, None, 0.0);
        assert_eq!(trade.duration_ms(), Some(1000000));
    }

    #[test]
    fn test_backtest_trade_excursion() {
        let mut trade = BacktestTrade::open(
            "BTC".to_string(),
            OrderSide::Buy,
            50000.0,
            1.0,
            0,
            None,
            0.0,
        );

        trade.update_excursion(52000.0); // 2000 profit
        assert_eq!(trade.max_favorable_excursion, 2000.0);

        trade.update_excursion(49000.0); // 1000 loss
        assert_eq!(trade.max_adverse_excursion, -1000.0);

        trade.update_excursion(53000.0); // 3000 profit
        assert_eq!(trade.max_favorable_excursion, 3000.0);
        assert_eq!(trade.max_adverse_excursion, -1000.0); // Still the worst
    }

    #[test]
    fn test_equity_point() {
        let point = EquityPoint {
            timestamp: 1704067200000,
            equity: 10500.0,
            cash: 5000.0,
            positions_value: 5500.0,
            realized_pnl: 200.0,
            unrealized_pnl: 300.0,
            drawdown_pct: 2.5,
        };

        assert_eq!(point.equity, 10500.0);
        assert_eq!(point.cash + point.positions_value, 10500.0);
    }

    #[test]
    fn test_backtest_metrics_default() {
        let metrics = BacktestMetrics::default();

        assert_eq!(metrics.total_return_pct, 0.0);
        assert_eq!(metrics.total_trades, 0);
        assert_eq!(metrics.winning_trades, 0);
        assert_eq!(metrics.sharpe_ratio, 0.0);
        assert_eq!(metrics.max_drawdown_pct, 0.0);
    }

    #[test]
    fn test_backtest_result_lifecycle() {
        let config = BacktestConfig::new(
            "strategy-1".to_string(),
            1704067200000,
            1706745600000,
        );

        let mut result = BacktestResult::new("strategy-1".to_string(), config);

        assert_eq!(result.status, BacktestStatus::Pending);
        assert!(result.started_at.is_none());
        assert!(result.completed_at.is_none());

        result.start();
        assert_eq!(result.status, BacktestStatus::Running);
        assert!(result.started_at.is_some());

        result.complete(11000.0);
        assert_eq!(result.status, BacktestStatus::Completed);
        assert_eq!(result.final_balance, 11000.0);
        assert!(result.completed_at.is_some());
        assert!(result.execution_time_ms.is_some());
    }

    #[test]
    fn test_backtest_result_failure() {
        let config = BacktestConfig::new(
            "strategy-1".to_string(),
            1704067200000,
            1706745600000,
        );

        let mut result = BacktestResult::new("strategy-1".to_string(), config);
        result.start();
        result.fail("No historical data available".to_string());

        assert_eq!(result.status, BacktestStatus::Failed);
        assert_eq!(result.error_message, Some("No historical data available".to_string()));
    }

    #[test]
    fn test_buy_and_hold_comparison() {
        let comparison = BuyAndHoldComparison {
            bnh_return_pct: 15.0,
            outperformance_pct: 10.0,
            strategy_max_dd: 8.0,
            bnh_max_dd: 12.0,
            strategy_sharpe: 1.5,
            bnh_sharpe: 0.8,
        };

        assert_eq!(comparison.bnh_return_pct, 15.0);
        assert_eq!(comparison.outperformance_pct, 10.0);
    }

    #[test]
    fn test_monte_carlo_results() {
        let mc = MonteCarloResults {
            num_runs: 1000,
            return_p5: -5.0,
            return_p25: 2.0,
            return_p50: 8.0,
            return_p75: 15.0,
            return_p95: 25.0,
            max_dd_p5: 3.0,
            max_dd_p50: 10.0,
            max_dd_p95: 25.0,
            probability_of_profit: 75.0,
            probability_of_ruin: 2.0,
        };

        assert_eq!(mc.num_runs, 1000);
        assert!(mc.return_p50 > mc.return_p25);
        assert!(mc.probability_of_profit > mc.probability_of_ruin);
    }
}

// =============================================================================
// Chart Candle Tests
// =============================================================================

mod chart_candle_tests {
    use super::*;

    #[test]
    fn test_chart_candle_creation() {
        let candle = ChartCandle::new(
            1704067200000,
            50000.0,
            51000.0,
            49500.0,
            50500.0,
            1000000.0,
        );

        assert_eq!(candle.timestamp, 1704067200000);
        assert_eq!(candle.open, 50000.0);
        assert_eq!(candle.high, 51000.0);
        assert_eq!(candle.low, 49500.0);
        assert_eq!(candle.close, 50500.0);
        assert_eq!(candle.volume, 1000000.0);
    }

    #[test]
    fn test_chart_candle_from_ohlc() {
        let ohlc = OhlcPoint {
            time: 1704067200000,
            open: 50000.0,
            high: 51000.0,
            low: 49500.0,
            close: 50500.0,
            volume: Some(1000000.0),
        };

        let candle = ChartCandle::from_ohlc(&ohlc);

        assert_eq!(candle.timestamp, ohlc.time);
        assert_eq!(candle.open, ohlc.open);
        assert_eq!(candle.close, ohlc.close);
        assert_eq!(candle.volume, 1000000.0);
    }

    #[test]
    fn test_chart_candle_to_ohlc() {
        let candle = ChartCandle::new(
            1704067200000,
            50000.0,
            51000.0,
            49500.0,
            50500.0,
            1000000.0,
        );

        let ohlc = candle.to_ohlc();

        assert_eq!(ohlc.time, candle.timestamp);
        assert_eq!(ohlc.open, candle.open);
        assert_eq!(ohlc.high, candle.high);
        assert_eq!(ohlc.low, candle.low);
        assert_eq!(ohlc.close, candle.close);
        assert_eq!(ohlc.volume, Some(candle.volume));
    }
}

// =============================================================================
// Cost Basis Tests
// =============================================================================

mod cost_basis_tests {
    use super::*;

    #[test]
    fn test_cost_basis_method_serialization() {
        assert_eq!(serde_json::to_string(&CostBasisMethod::Fifo).unwrap(), "\"fifo\"");
        assert_eq!(serde_json::to_string(&CostBasisMethod::Lifo).unwrap(), "\"lifo\"");
        assert_eq!(serde_json::to_string(&CostBasisMethod::Average).unwrap(), "\"average\"");
    }
}

// =============================================================================
// Bracket Order Tests
// =============================================================================

mod bracket_order_tests {
    use super::*;

    #[test]
    fn test_bracket_role_serialization() {
        assert_eq!(serde_json::to_string(&BracketRole::Entry).unwrap(), "\"entry\"");
        assert_eq!(serde_json::to_string(&BracketRole::StopLoss).unwrap(), "\"stop_loss\"");
        assert_eq!(serde_json::to_string(&BracketRole::TakeProfit).unwrap(), "\"take_profit\"");
    }

    #[test]
    fn test_stop_loss_order_creation() {
        let order = Order::stop_loss(
            "portfolio-1".to_string(),
            "BTC".to_string(),
            AssetClass::CryptoSpot,
            OrderSide::Sell,
            1.0,
            48000.0,
        );

        assert_eq!(order.order_type, OrderType::StopLoss);
        assert_eq!(order.stop_price, Some(48000.0));
    }

    #[test]
    fn test_take_profit_order_creation() {
        let order = Order::take_profit(
            "portfolio-1".to_string(),
            "BTC".to_string(),
            AssetClass::CryptoSpot,
            OrderSide::Sell,
            1.0,
            55000.0,
        );

        assert_eq!(order.order_type, OrderType::TakeProfit);
        assert_eq!(order.stop_price, Some(55000.0));
    }

    #[test]
    fn test_trailing_stop_order_creation() {
        let order = Order::trailing_stop(
            "portfolio-1".to_string(),
            "BTC".to_string(),
            AssetClass::CryptoSpot,
            OrderSide::Sell,
            1.0,
            Some(1000.0),  // trail amount
            None,          // trail percent
            50000.0,       // initial price
        );

        assert_eq!(order.order_type, OrderType::TrailingStop);
        assert_eq!(order.trail_amount, Some(1000.0));
    }
}
