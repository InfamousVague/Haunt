//! Trading Service
//!
//! Handles paper trading operations including:
//! - Portfolio management (create, update, delete)
//! - Order management (place, cancel, execute)
//! - Position management (open, close, update)
//! - Trade execution simulation with realistic slippage
//!
//! Uses SQLite for persistence and DashMap for real-time caching.

use crate::services::liquidity_sim::{LiquiditySimulator, LiquiditySimConfig};
use crate::services::SqliteStore;
use crate::types::{
    AggregatedOrderBook, AssetClass, BracketOrder, BracketRole, CostBasisEntry, CostBasisMethod,
    Fill, OcoOrder, Order, OrderSide, OrderStatus, OrderType, PlaceOrderRequest, Portfolio,
    Position, PositionSide, PortfolioSummary, RiskSettings, TimeInForce, Trade,
};
use crate::types::{
    LiquidationAlertData, MarginWarningData, OrderUpdateData, OrderUpdateType,
    PortfolioUpdateData, PortfolioUpdateType, PositionUpdateData, PositionUpdateType,
    ServerMessage, TradeExecutionData,
};
use crate::websocket::RoomManager;
use dashmap::DashMap;
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, info, warn};

/// Trading service errors.
#[derive(Debug, Error)]
pub enum TradingError {
    #[error("Portfolio not found: {0}")]
    PortfolioNotFound(String),

    #[error("Order not found: {0}")]
    OrderNotFound(String),

    #[error("Position not found: {0}")]
    PositionNotFound(String),

    #[error("Insufficient funds: need {needed}, have {available}")]
    InsufficientFunds { needed: f64, available: f64 },

    #[error("Insufficient margin: need {needed}, have {available}")]
    InsufficientMargin { needed: f64, available: f64 },

    #[error("Position limit exceeded: max {max} positions")]
    PositionLimitExceeded { max: u32 },

    #[error("Invalid order: {0}")]
    InvalidOrder(String),

    #[error("Order cannot be cancelled: status is {0}")]
    CannotCancelOrder(String),

    #[error("Leverage exceeds maximum: {requested} > {max}")]
    LeverageExceeded { requested: f64, max: f64 },

    #[error("Portfolio is stopped due to drawdown")]
    PortfolioStopped,

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("No price data available for {0}")]
    NoPriceData(String),
}

impl From<rusqlite::Error> for TradingError {
    fn from(e: rusqlite::Error) -> Self {
        TradingError::DatabaseError(e.to_string())
    }
}

/// Configuration for trade execution simulation.
#[derive(Debug, Clone)]
pub struct ExecutionConfig {
    /// Base slippage percentage for liquid assets
    pub base_slippage_pct: f64,
    /// Base slippage percentage for illiquid assets
    pub illiquid_slippage_pct: f64,
    /// Impact factor for large orders
    pub impact_factor: f64,
    /// Base fee percentage
    pub fee_pct: f64,
    /// Minimum order value
    pub min_order_value: f64,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            base_slippage_pct: 0.0001,      // 0.01%
            illiquid_slippage_pct: 0.0005,  // 0.05%
            impact_factor: 0.1,
            fee_pct: 0.001,                 // 0.1%
            min_order_value: 1.0,
        }
    }
}

/// Paper trading service.
#[derive(Clone)]
pub struct TradingService {
    /// Portfolios cache (portfolio_id -> Portfolio)
    portfolios: Arc<DashMap<String, Portfolio>>,
    /// Open orders cache (order_id -> Order)
    orders: Arc<DashMap<String, Order>>,
    /// Open positions cache (position_id -> Position)
    positions: Arc<DashMap<String, Position>>,
    /// SQLite store for persistence
    sqlite: Arc<SqliteStore>,
    /// Execution configuration
    config: ExecutionConfig,
    /// Liquidity simulator for realistic order book-based execution
    liquidity_sim: Arc<LiquiditySimulator>,
    /// Room manager for WebSocket broadcasts (optional for testing)
    room_manager: Option<Arc<RoomManager>>,
}

impl TradingService {
    /// Create a new trading service.
    pub fn new(sqlite: Arc<SqliteStore>) -> Self {
        Self {
            portfolios: Arc::new(DashMap::new()),
            orders: Arc::new(DashMap::new()),
            positions: Arc::new(DashMap::new()),
            sqlite,
            config: ExecutionConfig::default(),
            liquidity_sim: Arc::new(LiquiditySimulator::default()),
            room_manager: None,
        }
    }

    /// Create a new trading service with custom execution config.
    pub fn with_config(sqlite: Arc<SqliteStore>, config: ExecutionConfig) -> Self {
        Self {
            portfolios: Arc::new(DashMap::new()),
            orders: Arc::new(DashMap::new()),
            positions: Arc::new(DashMap::new()),
            sqlite,
            config,
            liquidity_sim: Arc::new(LiquiditySimulator::default()),
            room_manager: None,
        }
    }

    /// Create a new trading service with custom liquidity simulation.
    pub fn with_liquidity_config(sqlite: Arc<SqliteStore>, liquidity_config: LiquiditySimConfig) -> Self {
        Self {
            portfolios: Arc::new(DashMap::new()),
            orders: Arc::new(DashMap::new()),
            positions: Arc::new(DashMap::new()),
            sqlite,
            config: ExecutionConfig::default(),
            liquidity_sim: Arc::new(LiquiditySimulator::new(liquidity_config)),
            room_manager: None,
        }
    }

    /// Create a new trading service with room manager for WebSocket broadcasts.
    pub fn with_room_manager(sqlite: Arc<SqliteStore>, room_manager: Arc<RoomManager>) -> Self {
        Self {
            portfolios: Arc::new(DashMap::new()),
            orders: Arc::new(DashMap::new()),
            positions: Arc::new(DashMap::new()),
            sqlite,
            config: ExecutionConfig::default(),
            liquidity_sim: Arc::new(LiquiditySimulator::default()),
            room_manager: Some(room_manager),
        }
    }

    /// Set room manager for WebSocket broadcasts.
    pub fn set_room_manager(&mut self, room_manager: Arc<RoomManager>) {
        self.room_manager = Some(room_manager);
    }

    // ==========================================================================
    // WebSocket Broadcast Helpers
    // ==========================================================================

    /// Broadcast an order update to subscribers.
    fn broadcast_order_update(&self, order: &Order, update_type: OrderUpdateType) {
        if let Some(ref room_manager) = self.room_manager {
            let data = OrderUpdateData {
                order: order.clone(),
                update_type,
                timestamp: chrono::Utc::now().timestamp_millis(),
            };
            let msg = ServerMessage::OrderUpdate { data };
            if let Ok(json) = serde_json::to_string(&msg) {
                room_manager.broadcast_trading(&order.portfolio_id, &json);
            }
        }
    }

    /// Broadcast a position update to subscribers.
    fn broadcast_position_update(&self, position: &Position, update_type: PositionUpdateType) {
        if let Some(ref room_manager) = self.room_manager {
            let data = PositionUpdateData {
                position: position.clone(),
                update_type,
                timestamp: chrono::Utc::now().timestamp_millis(),
            };
            let msg = ServerMessage::PositionUpdate { data };
            if let Ok(json) = serde_json::to_string(&msg) {
                room_manager.broadcast_trading(&position.portfolio_id, &json);
            }
        }
    }

    /// Broadcast a portfolio update to subscribers.
    fn broadcast_portfolio_update(&self, portfolio: &Portfolio, update_type: PortfolioUpdateType) {
        if let Some(ref room_manager) = self.room_manager {
            let data = PortfolioUpdateData {
                portfolio: portfolio.clone(),
                update_type,
                timestamp: chrono::Utc::now().timestamp_millis(),
            };
            let msg = ServerMessage::PortfolioUpdate { data };
            if let Ok(json) = serde_json::to_string(&msg) {
                room_manager.broadcast_trading(&portfolio.id, &json);
            }
        }
    }

    /// Broadcast a trade execution to subscribers.
    fn broadcast_trade_execution(&self, trade: &Trade, position_id: Option<String>) {
        if let Some(ref room_manager) = self.room_manager {
            let data = TradeExecutionData {
                trade: trade.clone(),
                order_id: trade.order_id.clone(),
                position_id,
                timestamp: chrono::Utc::now().timestamp_millis(),
            };
            let msg = ServerMessage::TradeExecution { data };
            if let Ok(json) = serde_json::to_string(&msg) {
                room_manager.broadcast_trading(&trade.portfolio_id, &json);
            }
        }
    }

    /// Broadcast a margin warning to subscribers.
    fn broadcast_margin_warning(
        &self,
        portfolio_id: &str,
        margin_level: f64,
        warning_level: f64,
        at_risk_positions: Vec<String>,
    ) {
        if let Some(ref room_manager) = self.room_manager {
            let data = MarginWarningData {
                portfolio_id: portfolio_id.to_string(),
                margin_level,
                warning_level,
                at_risk_positions,
                timestamp: chrono::Utc::now().timestamp_millis(),
            };
            let msg = ServerMessage::MarginWarning { data };
            if let Ok(json) = serde_json::to_string(&msg) {
                room_manager.broadcast_trading(portfolio_id, &json);
            }
        }
    }

    /// Broadcast a liquidation alert to subscribers.
    fn broadcast_liquidation_alert(
        &self,
        portfolio_id: &str,
        position_id: &str,
        symbol: &str,
        liquidation_price: f64,
        loss_amount: f64,
    ) {
        if let Some(ref room_manager) = self.room_manager {
            let data = LiquidationAlertData {
                portfolio_id: portfolio_id.to_string(),
                position_id: position_id.to_string(),
                symbol: symbol.to_string(),
                liquidation_price,
                loss_amount,
                timestamp: chrono::Utc::now().timestamp_millis(),
            };
            let msg = ServerMessage::LiquidationAlert { data };
            if let Ok(json) = serde_json::to_string(&msg) {
                room_manager.broadcast_trading(portfolio_id, &json);
            }
        }
    }

    // ==========================================================================
    // Portfolio Management
    // ==========================================================================

    /// Create a new portfolio for a user.
    pub fn create_portfolio(
        &self,
        user_id: &str,
        name: &str,
        description: Option<String>,
        risk_settings: Option<RiskSettings>,
    ) -> Result<Portfolio, TradingError> {
        let mut portfolio = Portfolio::new(user_id.to_string(), name.to_string());

        if let Some(desc) = description {
            portfolio.description = Some(desc);
        }

        if let Some(settings) = risk_settings {
            portfolio.risk_settings = settings;
        }

        // Persist to database
        self.sqlite.create_portfolio(&portfolio)?;

        // Cache in memory
        self.portfolios.insert(portfolio.id.clone(), portfolio.clone());

        info!("Created portfolio {} for user {}", portfolio.id, user_id);
        Ok(portfolio)
    }

    /// Get a portfolio by ID.
    pub fn get_portfolio(&self, id: &str) -> Option<Portfolio> {
        // Check cache first
        if let Some(portfolio) = self.portfolios.get(id) {
            return Some(portfolio.clone());
        }

        // Load from database
        if let Some(portfolio) = self.sqlite.get_portfolio(id) {
            self.portfolios.insert(portfolio.id.clone(), portfolio.clone());
            return Some(portfolio);
        }

        None
    }

    /// Get all portfolios for a user.
    pub fn get_user_portfolios(&self, user_id: &str) -> Vec<Portfolio> {
        let portfolios = self.sqlite.get_user_portfolios(user_id);

        // Update cache
        for portfolio in &portfolios {
            self.portfolios.insert(portfolio.id.clone(), portfolio.clone());
        }

        portfolios
    }

    /// Get portfolio summary with current metrics.
    pub fn get_portfolio_summary(&self, portfolio_id: &str) -> Result<PortfolioSummary, TradingError> {
        let portfolio = self
            .get_portfolio(portfolio_id)
            .ok_or_else(|| TradingError::PortfolioNotFound(portfolio_id.to_string()))?;

        let open_positions = self.sqlite.position_count(portfolio_id) as u32;
        let open_orders = self.sqlite.open_order_count(portfolio_id) as u32;
        let total_return_pct = portfolio.total_return_pct();
        let margin_level = portfolio.margin_level();

        Ok(PortfolioSummary {
            portfolio_id: portfolio.id,
            total_value: portfolio.total_value,
            cash_balance: portfolio.cash_balance,
            unrealized_pnl: portfolio.unrealized_pnl,
            realized_pnl: portfolio.realized_pnl,
            total_return_pct,
            margin_used: portfolio.margin_used,
            margin_available: portfolio.margin_available,
            margin_level,
            open_positions,
            open_orders,
        })
    }

    /// Update portfolio risk settings.
    pub fn update_portfolio_settings(
        &self,
        portfolio_id: &str,
        risk_settings: RiskSettings,
    ) -> Result<Portfolio, TradingError> {
        let mut portfolio = self
            .get_portfolio(portfolio_id)
            .ok_or_else(|| TradingError::PortfolioNotFound(portfolio_id.to_string()))?;

        portfolio.risk_settings = risk_settings;
        portfolio.updated_at = chrono::Utc::now().timestamp_millis();

        self.sqlite.update_portfolio(&portfolio)?;
        self.portfolios.insert(portfolio.id.clone(), portfolio.clone());

        // Broadcast portfolio settings change
        self.broadcast_portfolio_update(&portfolio, PortfolioUpdateType::SettingsChanged);

        Ok(portfolio)
    }

    /// Reset a portfolio to starting balance.
    pub fn reset_portfolio(&self, portfolio_id: &str) -> Result<Portfolio, TradingError> {
        let mut portfolio = self
            .get_portfolio(portfolio_id)
            .ok_or_else(|| TradingError::PortfolioNotFound(portfolio_id.to_string()))?;

        // Close all positions
        let positions = self.sqlite.get_portfolio_positions(portfolio_id);
        for position in positions {
            self.sqlite.close_position(&position.id)?;
            self.positions.remove(&position.id);
        }

        // Cancel all open orders
        let orders = self.sqlite.get_open_orders(portfolio_id);
        for mut order in orders {
            order.status = OrderStatus::Cancelled;
            order.updated_at = chrono::Utc::now().timestamp_millis();
            self.sqlite.update_order(&order)?;
            self.orders.remove(&order.id);
        }

        // Reset portfolio balances
        portfolio.cash_balance = portfolio.starting_balance;
        portfolio.margin_used = 0.0;
        portfolio.margin_available = portfolio.starting_balance;
        portfolio.unrealized_pnl = 0.0;
        portfolio.realized_pnl = 0.0;
        portfolio.total_value = portfolio.starting_balance;
        portfolio.updated_at = chrono::Utc::now().timestamp_millis();

        self.sqlite.update_portfolio(&portfolio)?;
        self.portfolios.insert(portfolio.id.clone(), portfolio.clone());

        // Broadcast portfolio reset
        self.broadcast_portfolio_update(&portfolio, PortfolioUpdateType::Reset);

        info!("Reset portfolio {}", portfolio_id);
        Ok(portfolio)
    }

    /// Delete a portfolio and all associated data.
    pub fn delete_portfolio(&self, portfolio_id: &str) -> Result<(), TradingError> {
        self.sqlite.delete_portfolio(portfolio_id)?;
        self.portfolios.remove(portfolio_id);

        // Clean up cached orders and positions
        self.orders.retain(|_, o| o.portfolio_id != portfolio_id);
        self.positions.retain(|_, p| p.portfolio_id != portfolio_id);

        info!("Deleted portfolio {}", portfolio_id);
        Ok(())
    }

    // ==========================================================================
    // Order Management
    // ==========================================================================

    /// Place a new order.
    pub fn place_order(&self, request: PlaceOrderRequest) -> Result<Order, TradingError> {
        // Validate portfolio exists and is not stopped
        let portfolio = self
            .get_portfolio(&request.portfolio_id)
            .ok_or_else(|| TradingError::PortfolioNotFound(request.portfolio_id.clone()))?;

        if portfolio.is_stopped() {
            return Err(TradingError::PortfolioStopped);
        }

        // Validate leverage
        let leverage = request.leverage.unwrap_or(1.0);
        let max_leverage = request.asset_class.max_leverage();
        if leverage > max_leverage {
            return Err(TradingError::LeverageExceeded {
                requested: leverage,
                max: max_leverage,
            });
        }

        // Check position limits
        let current_positions = self.sqlite.position_count(&request.portfolio_id) as u32;
        if current_positions >= portfolio.risk_settings.max_open_positions {
            return Err(TradingError::PositionLimitExceeded {
                max: portfolio.risk_settings.max_open_positions,
            });
        }

        // Create order
        let mut order = Order {
            id: uuid::Uuid::new_v4().to_string(),
            portfolio_id: request.portfolio_id,
            symbol: request.symbol,
            asset_class: request.asset_class,
            side: request.side,
            order_type: request.order_type,
            quantity: request.quantity,
            filled_quantity: 0.0,
            price: request.price,
            stop_price: request.stop_price,
            trail_amount: request.trail_amount,
            trail_percent: request.trail_percent,
            time_in_force: request.time_in_force.unwrap_or_default(),
            status: OrderStatus::Pending,
            linked_order_id: None,
            bracket_id: None,
            leverage,
            fills: Vec::new(),
            avg_fill_price: None,
            total_fees: 0.0,
            client_order_id: request.client_order_id,
            created_at: chrono::Utc::now().timestamp_millis(),
            updated_at: chrono::Utc::now().timestamp_millis(),
            expires_at: None,
            trail_high_price: None,
            trail_low_price: None,
            bracket_role: None,
        };

        // Initialize trailing stop tracking if applicable
        if order.order_type == OrderType::TrailingStop {
            // For trailing stops, we need an initial reference price
            // This would typically come from the current market price
            // For now, set initial tracking values to trigger price
            if let Some(stop) = order.stop_price {
                match order.side {
                    OrderSide::Sell => order.trail_high_price = Some(stop),
                    OrderSide::Buy => order.trail_low_price = Some(stop),
                }
            }
        }

        // Validate order
        self.validate_order(&order, &portfolio)?;

        // Persist and cache
        self.sqlite.create_order(&order)?;
        self.orders.insert(order.id.clone(), order.clone());

        // Broadcast order creation
        self.broadcast_order_update(&order, OrderUpdateType::Created);

        info!("Placed order {} for {} {}", order.id, order.side, order.symbol);
        Ok(order)
    }

    /// Validate an order before placement.
    fn validate_order(&self, order: &Order, portfolio: &Portfolio) -> Result<(), TradingError> {
        // Validate quantity
        if order.quantity <= 0.0 {
            return Err(TradingError::InvalidOrder("Quantity must be positive".to_string()));
        }

        // Validate prices for limit/stop orders
        match order.order_type {
            OrderType::Limit => {
                if order.price.is_none() {
                    return Err(TradingError::InvalidOrder(
                        "Limit order requires price".to_string(),
                    ));
                }
            }
            OrderType::StopLoss | OrderType::TakeProfit | OrderType::StopLimit => {
                if order.stop_price.is_none() {
                    return Err(TradingError::InvalidOrder(
                        "Stop order requires stop_price".to_string(),
                    ));
                }
            }
            OrderType::TrailingStop => {
                if order.trail_amount.is_none() && order.trail_percent.is_none() {
                    return Err(TradingError::InvalidOrder(
                        "Trailing stop requires trail_amount or trail_percent".to_string(),
                    ));
                }
            }
            _ => {}
        }

        // Check if buy order has sufficient funds (rough estimate)
        if order.side == OrderSide::Buy {
            // For market orders, we can't know exact cost without price
            // For limit orders, use limit price
            if let Some(price) = order.price {
                let estimated_cost = order.quantity * price / order.leverage;
                if estimated_cost > portfolio.margin_available {
                    return Err(TradingError::InsufficientMargin {
                        needed: estimated_cost,
                        available: portfolio.margin_available,
                    });
                }
            }
        }

        Ok(())
    }

    /// Get an order by ID.
    pub fn get_order(&self, order_id: &str) -> Option<Order> {
        if let Some(order) = self.orders.get(order_id) {
            return Some(order.clone());
        }

        if let Some(order) = self.sqlite.get_order(order_id) {
            self.orders.insert(order.id.clone(), order.clone());
            return Some(order);
        }

        None
    }

    /// Get all open orders for a portfolio.
    pub fn get_open_orders(&self, portfolio_id: &str) -> Vec<Order> {
        self.sqlite.get_open_orders(portfolio_id)
    }

    /// Get order history for a portfolio.
    pub fn get_order_history(&self, portfolio_id: &str, limit: usize) -> Vec<Order> {
        self.sqlite.get_portfolio_orders(portfolio_id, None, limit)
    }

    /// Cancel an order.
    pub fn cancel_order(&self, order_id: &str) -> Result<Order, TradingError> {
        let mut order = self
            .get_order(order_id)
            .ok_or_else(|| TradingError::OrderNotFound(order_id.to_string()))?;

        if !order.can_cancel() {
            return Err(TradingError::CannotCancelOrder(order.status.to_string()));
        }

        order.status = OrderStatus::Cancelled;
        order.updated_at = chrono::Utc::now().timestamp_millis();

        self.sqlite.update_order(&order)?;
        self.orders.insert(order.id.clone(), order.clone());

        // Broadcast order cancellation
        self.broadcast_order_update(&order, OrderUpdateType::Cancelled);

        info!("Cancelled order {}", order_id);
        Ok(order)
    }

    // ==========================================================================
    // Order Execution
    // ==========================================================================

    /// Execute a market order immediately.
    pub fn execute_market_order(
        &self,
        order_id: &str,
        current_price: f64,
        order_book: Option<&AggregatedOrderBook>,
    ) -> Result<Trade, TradingError> {
        let mut order = self
            .get_order(order_id)
            .ok_or_else(|| TradingError::OrderNotFound(order_id.to_string()))?;

        if order.is_terminal() {
            return Err(TradingError::InvalidOrder(format!(
                "Order {} is already {}",
                order_id, order.status
            )));
        }

        // Calculate execution price with slippage
        let (execution_price, slippage) =
            self.calculate_execution_price(&order, current_price, order_book);

        // Calculate fee
        let notional = order.quantity * execution_price;
        let fee = notional * self.config.fee_pct;

        // Create fill
        let fill = Fill::new(order.quantity, execution_price, fee);
        order.add_fill(fill);

        // Update portfolio
        let mut portfolio = self
            .get_portfolio(&order.portfolio_id)
            .ok_or_else(|| TradingError::PortfolioNotFound(order.portfolio_id.clone()))?;

        // Create or update position
        let position_id = self.update_position_for_trade(&mut portfolio, &order, execution_price)?;

        // Persist order
        self.sqlite.update_order(&order)?;
        self.orders.insert(order.id.clone(), order.clone());

        // Persist portfolio
        self.sqlite.update_portfolio(&portfolio)?;
        self.portfolios.insert(portfolio.id.clone(), portfolio.clone());

        // Create trade record
        let mut trade = Trade::new(
            order.id.clone(),
            order.portfolio_id.clone(),
            order.symbol.clone(),
            order.asset_class,
            order.side,
            order.quantity,
            execution_price,
            fee,
            slippage,
        );
        trade.position_id = Some(position_id.clone());

        self.sqlite.create_trade(&trade)?;

        // Broadcast updates
        self.broadcast_order_update(&order, OrderUpdateType::Filled);
        self.broadcast_trade_execution(&trade, Some(position_id));
        self.broadcast_portfolio_update(&portfolio, PortfolioUpdateType::BalanceChanged);

        info!(
            "Executed order {} at {} (slippage: {:.4}%)",
            order.id,
            execution_price,
            slippage / execution_price * 100.0
        );

        Ok(trade)
    }

    /// Calculate execution price with slippage simulation.
    /// Uses the liquidity simulator to walk the order book for realistic VWAP.
    fn calculate_execution_price(
        &self,
        order: &Order,
        current_price: f64,
        order_book: Option<&AggregatedOrderBook>,
    ) -> (f64, f64) {
        // Use liquidity simulator for order book-based execution
        if let Some(book) = order_book {
            if !book.asks.is_empty() || !book.bids.is_empty() {
                let (exec_price, slippage, _filled) = self.liquidity_sim.calculate_execution_price(
                    book,
                    order.side,
                    order.quantity,
                );
                return (exec_price, slippage);
            }
        }

        // Fallback: simple slippage model when no order book available
        let base_slippage = current_price * self.config.base_slippage_pct;
        let execution_price = match order.side {
            OrderSide::Buy => current_price + base_slippage,
            OrderSide::Sell => current_price - base_slippage,
        };

        (execution_price, base_slippage.abs())
    }

    /// Simulate a market order to get expected execution details.
    /// Useful for showing users expected slippage before execution.
    pub fn simulate_market_order(
        &self,
        order_book: &AggregatedOrderBook,
        side: OrderSide,
        quantity: f64,
    ) -> crate::services::liquidity_sim::MarketOrderSimulation {
        self.liquidity_sim.simulate_market_order(order_book, side, quantity)
    }

    /// Simulate a limit order to get fill probability and timing.
    pub fn simulate_limit_order(
        &self,
        order_book: &AggregatedOrderBook,
        side: OrderSide,
        quantity: f64,
        limit_price: f64,
        volume_24h: Option<f64>,
    ) -> crate::services::liquidity_sim::LimitOrderSimulation {
        self.liquidity_sim.simulate_limit_order(order_book, side, quantity, limit_price, volume_24h)
    }

    /// Update or create position based on trade.
    fn update_position_for_trade(
        &self,
        portfolio: &mut Portfolio,
        order: &Order,
        execution_price: f64,
    ) -> Result<String, TradingError> {
        let notional = order.quantity * execution_price;
        let margin_required = notional / order.leverage;

        // Determine position side from order
        let position_side = match order.side {
            OrderSide::Buy => PositionSide::Long,
            OrderSide::Sell => PositionSide::Short,
        };

        // Check for existing position
        let existing = self.sqlite.get_position_by_symbol(
            &order.portfolio_id,
            &order.symbol,
            position_side,
        );

        let position_id = if let Some(mut position) = existing {
            // Add to existing position (average in)
            let total_qty = position.quantity + order.quantity;
            let total_cost =
                position.entry_price * position.quantity + execution_price * order.quantity;
            position.entry_price = total_cost / total_qty;
            position.quantity = total_qty;
            position.margin_used += margin_required;

            // Add cost basis entry
            position.cost_basis.push(CostBasisEntry {
                quantity: order.quantity,
                price: execution_price,
                acquired_at: chrono::Utc::now().timestamp_millis(),
            });

            position.update_price(execution_price);
            position.calculate_liquidation_price();

            self.sqlite.update_position(&position)?;
            self.positions.insert(position.id.clone(), position.clone());

            // Broadcast position increase
            self.broadcast_position_update(&position, PositionUpdateType::Increased);

            position.id
        } else {
            // Check for opposite position (close or flip)
            let opposite_side = match position_side {
                PositionSide::Long => PositionSide::Short,
                PositionSide::Short => PositionSide::Long,
            };

            if let Some(mut opposite_position) =
                self.sqlite
                    .get_position_by_symbol(&order.portfolio_id, &order.symbol, opposite_side)
            {
                // Closing position
                let realized_pnl = self.calculate_realized_pnl(&opposite_position, order.quantity, execution_price);

                if order.quantity >= opposite_position.quantity {
                    // Fully close position
                    portfolio.realized_pnl += realized_pnl;
                    portfolio.cash_balance += opposite_position.margin_used + realized_pnl;
                    portfolio.margin_used -= opposite_position.margin_used;

                    self.sqlite.close_position(&opposite_position.id)?;
                    self.positions.remove(&opposite_position.id);

                    // Broadcast position closed
                    self.broadcast_position_update(&opposite_position, PositionUpdateType::Closed);

                    // If there's remaining quantity, open new position in opposite direction
                    let remaining = order.quantity - opposite_position.quantity;
                    if remaining > 0.0 {
                        let new_position = self.create_new_position(
                            portfolio,
                            order,
                            execution_price,
                            remaining,
                            position_side,
                        )?;
                        // Broadcast new position opened
                        self.broadcast_position_update(&new_position, PositionUpdateType::Opened);
                        return Ok(new_position.id);
                    }

                    opposite_position.id
                } else {
                    // Partially close position
                    let close_ratio = order.quantity / opposite_position.quantity;
                    let margin_released = opposite_position.margin_used * close_ratio;

                    opposite_position.quantity -= order.quantity;
                    opposite_position.margin_used -= margin_released;
                    opposite_position.realized_pnl += realized_pnl;
                    opposite_position.update_price(execution_price);

                    // Remove closed portion from cost basis (FIFO by default)
                    self.reduce_cost_basis(&mut opposite_position, order.quantity, portfolio.cost_basis_method);

                    portfolio.realized_pnl += realized_pnl;
                    portfolio.cash_balance += margin_released + realized_pnl;
                    portfolio.margin_used -= margin_released;

                    self.sqlite.update_position(&opposite_position)?;
                    self.positions.insert(opposite_position.id.clone(), opposite_position.clone());

                    // Broadcast position decreased
                    self.broadcast_position_update(&opposite_position, PositionUpdateType::Decreased);

                    opposite_position.id
                }
            } else {
                // Open new position
                let position = self.create_new_position(
                    portfolio,
                    order,
                    execution_price,
                    order.quantity,
                    position_side,
                )?;
                // Broadcast new position opened
                self.broadcast_position_update(&position, PositionUpdateType::Opened);
                position.id
            }
        };

        // Update portfolio
        portfolio.recalculate();

        Ok(position_id)
    }

    /// Create a new position.
    fn create_new_position(
        &self,
        portfolio: &mut Portfolio,
        order: &Order,
        price: f64,
        quantity: f64,
        side: PositionSide,
    ) -> Result<Position, TradingError> {
        let notional = quantity * price;
        let margin_required = notional / order.leverage;

        if margin_required > portfolio.margin_available {
            return Err(TradingError::InsufficientMargin {
                needed: margin_required,
                available: portfolio.margin_available,
            });
        }

        let mut position = Position::new(
            order.portfolio_id.clone(),
            order.symbol.clone(),
            order.asset_class,
            side,
            quantity,
            price,
            order.leverage,
        );

        position.calculate_liquidation_price();

        // Update portfolio
        portfolio.cash_balance -= margin_required;
        portfolio.margin_used += margin_required;
        portfolio.margin_available = portfolio.cash_balance - portfolio.margin_used;

        self.sqlite.create_position(&position)?;
        self.positions.insert(position.id.clone(), position.clone());

        debug!("Opened position {} for {} {}", position.id, quantity, order.symbol);
        Ok(position)
    }

    /// Calculate realized P&L for closing a position.
    fn calculate_realized_pnl(&self, position: &Position, close_qty: f64, close_price: f64) -> f64 {
        let entry_value = close_qty * position.entry_price;
        let exit_value = close_qty * close_price;

        match position.side {
            PositionSide::Long => exit_value - entry_value,
            PositionSide::Short => entry_value - exit_value,
        }
    }

    /// Reduce cost basis using the specified method.
    fn reduce_cost_basis(
        &self,
        position: &mut Position,
        mut qty_to_remove: f64,
        method: CostBasisMethod,
    ) {
        match method {
            CostBasisMethod::Fifo => {
                // Remove from front
                while qty_to_remove > 0.0 && !position.cost_basis.is_empty() {
                    let entry = &mut position.cost_basis[0];
                    if entry.quantity <= qty_to_remove {
                        qty_to_remove -= entry.quantity;
                        position.cost_basis.remove(0);
                    } else {
                        entry.quantity -= qty_to_remove;
                        qty_to_remove = 0.0;
                    }
                }
            }
            CostBasisMethod::Lifo => {
                // Remove from back
                while qty_to_remove > 0.0 && !position.cost_basis.is_empty() {
                    let last_idx = position.cost_basis.len() - 1;
                    let entry = &mut position.cost_basis[last_idx];
                    if entry.quantity <= qty_to_remove {
                        qty_to_remove -= entry.quantity;
                        position.cost_basis.pop();
                    } else {
                        entry.quantity -= qty_to_remove;
                        qty_to_remove = 0.0;
                    }
                }
            }
            CostBasisMethod::Average => {
                // Just reduce proportionally
                let total_qty: f64 = position.cost_basis.iter().map(|e| e.quantity).sum();
                let ratio = (total_qty - qty_to_remove) / total_qty;
                for entry in &mut position.cost_basis {
                    entry.quantity *= ratio;
                }
                position.cost_basis.retain(|e| e.quantity > 0.0001);
            }
        }
    }

    // ==========================================================================
    // Position Management
    // ==========================================================================

    /// Get a position by ID.
    pub fn get_position(&self, position_id: &str) -> Option<Position> {
        if let Some(position) = self.positions.get(position_id) {
            return Some(position.clone());
        }

        if let Some(position) = self.sqlite.get_position(position_id) {
            self.positions.insert(position.id.clone(), position.clone());
            return Some(position);
        }

        None
    }

    /// Get all open positions for a portfolio.
    pub fn get_positions(&self, portfolio_id: &str) -> Vec<Position> {
        self.sqlite.get_portfolio_positions(portfolio_id)
    }

    /// Update position with current market price.
    pub fn update_position_price(
        &self,
        position_id: &str,
        current_price: f64,
    ) -> Result<Position, TradingError> {
        let mut position = self
            .get_position(position_id)
            .ok_or_else(|| TradingError::PositionNotFound(position_id.to_string()))?;

        position.update_price(current_price);

        self.sqlite.update_position(&position)?;
        self.positions.insert(position.id.clone(), position.clone());

        // Update portfolio unrealized P&L
        self.recalculate_portfolio_pnl(&position.portfolio_id)?;

        Ok(position)
    }

    /// Update stop loss and take profit for a position.
    pub fn modify_position(
        &self,
        position_id: &str,
        stop_loss: Option<f64>,
        take_profit: Option<f64>,
    ) -> Result<Position, TradingError> {
        let mut position = self
            .get_position(position_id)
            .ok_or_else(|| TradingError::PositionNotFound(position_id.to_string()))?;

        position.stop_loss = stop_loss;
        position.take_profit = take_profit;
        position.updated_at = chrono::Utc::now().timestamp_millis();

        self.sqlite.update_position(&position)?;
        self.positions.insert(position.id.clone(), position.clone());

        // Broadcast position modification
        self.broadcast_position_update(&position, PositionUpdateType::Modified);

        Ok(position)
    }

    /// Close a position at market price.
    pub fn close_position(
        &self,
        position_id: &str,
        current_price: f64,
    ) -> Result<Trade, TradingError> {
        let position = self
            .get_position(position_id)
            .ok_or_else(|| TradingError::PositionNotFound(position_id.to_string()))?;

        // Create closing order
        let close_side = match position.side {
            PositionSide::Long => OrderSide::Sell,
            PositionSide::Short => OrderSide::Buy,
        };

        let order = Order::market(
            position.portfolio_id.clone(),
            position.symbol.clone(),
            position.asset_class,
            close_side,
            position.quantity,
        );

        self.sqlite.create_order(&order)?;
        self.orders.insert(order.id.clone(), order.clone());

        // Execute the closing order
        self.execute_market_order(&order.id, current_price, None)
    }

    /// Recalculate portfolio unrealized P&L from all positions.
    fn recalculate_portfolio_pnl(&self, portfolio_id: &str) -> Result<(), TradingError> {
        let mut portfolio = self
            .get_portfolio(portfolio_id)
            .ok_or_else(|| TradingError::PortfolioNotFound(portfolio_id.to_string()))?;

        let positions = self.sqlite.get_portfolio_positions(portfolio_id);
        let total_unrealized: f64 = positions.iter().map(|p| p.unrealized_pnl).sum();

        portfolio.unrealized_pnl = total_unrealized;
        portfolio.recalculate();

        self.sqlite.update_portfolio(&portfolio)?;
        self.portfolios.insert(portfolio.id.clone(), portfolio);

        Ok(())
    }

    // ==========================================================================
    // Trade History
    // ==========================================================================

    /// Get trade history for a portfolio.
    pub fn get_trades(&self, portfolio_id: &str, limit: usize) -> Vec<Trade> {
        self.sqlite.get_portfolio_trades(portfolio_id, limit)
    }

    /// Get trades for a specific order.
    pub fn get_order_trades(&self, order_id: &str) -> Vec<Trade> {
        self.sqlite.get_order_trades(order_id)
    }

    // ==========================================================================
    // Advanced Order Management
    // ==========================================================================

    /// Update all trailing stop orders for a symbol based on current price.
    /// Returns the number of orders updated.
    pub fn update_trailing_stops(&self, symbol: &str, current_price: f64) -> usize {
        let mut updated_count = 0;

        // Find all trailing stop orders for this symbol
        let trailing_orders: Vec<Order> = self
            .orders
            .iter()
            .filter(|entry| {
                let order = entry.value();
                order.symbol == symbol
                    && order.order_type == OrderType::TrailingStop
                    && !order.is_terminal()
            })
            .map(|entry| entry.value().clone())
            .collect();

        for mut order in trailing_orders {
            if order.update_trailing_stop(current_price) {
                // Persist and cache updated order
                if self.sqlite.update_order(&order).is_ok() {
                    self.orders.insert(order.id.clone(), order);
                    updated_count += 1;
                }
            }
        }

        updated_count
    }

    /// Cancel the linked OCO order when one order in the pair fills or is cancelled.
    pub fn cancel_linked_order(&self, order_id: &str) -> Result<Option<Order>, TradingError> {
        let order = self
            .get_order(order_id)
            .ok_or_else(|| TradingError::OrderNotFound(order_id.to_string()))?;

        if let Some(linked_id) = &order.linked_order_id {
            if let Some(mut linked_order) = self.get_order(linked_id) {
                if linked_order.can_cancel() {
                    linked_order.status = OrderStatus::Cancelled;
                    linked_order.updated_at = chrono::Utc::now().timestamp_millis();

                    self.sqlite.update_order(&linked_order)?;
                    self.orders.insert(linked_order.id.clone(), linked_order.clone());

                    // Broadcast cancellation
                    self.broadcast_order_update(&linked_order, OrderUpdateType::Cancelled);

                    info!("Cancelled linked OCO order {}", linked_id);
                    return Ok(Some(linked_order));
                }
            }
        }

        Ok(None)
    }

    /// Check and expire GTD orders that have passed their expiration time.
    /// Returns the number of orders expired.
    pub fn expire_gtd_orders(&self) -> usize {
        let now = chrono::Utc::now().timestamp_millis();
        let mut expired_count = 0;

        // Find all expired GTD orders
        let expired_orders: Vec<Order> = self
            .orders
            .iter()
            .filter(|entry| {
                let order = entry.value();
                order.time_in_force == TimeInForce::Gtd
                    && !order.is_terminal()
                    && order.expires_at.map(|exp| now >= exp).unwrap_or(false)
            })
            .map(|entry| entry.value().clone())
            .collect();

        for mut order in expired_orders {
            order.status = OrderStatus::Expired;
            order.updated_at = now;

            if self.sqlite.update_order(&order).is_ok() {
                self.orders.insert(order.id.clone(), order.clone());

                // Cancel any linked orders
                let _ = self.cancel_linked_order(&order.id);

                // Broadcast expiration
                self.broadcast_order_update(&order, OrderUpdateType::Expired);

                expired_count += 1;
                info!("Expired GTD order {}", order.id);
            }
        }

        expired_count
    }

    /// Validate and potentially reject a FOK order if it can't be fully filled.
    /// Returns Err if the order should be rejected, Ok(()) if it can proceed.
    pub fn validate_fok_order(
        &self,
        order: &Order,
        available_quantity: f64,
    ) -> Result<(), TradingError> {
        if order.time_in_force == TimeInForce::Fok && available_quantity < order.quantity {
            return Err(TradingError::InvalidOrder(
                "FOK order cannot be fully filled - rejecting".to_string(),
            ));
        }
        Ok(())
    }

    /// Execute an IOC order - fill what's available, cancel the rest.
    /// Returns the partial fill quantity (0 if nothing filled).
    pub fn execute_ioc_order(
        &self,
        order_id: &str,
        available_quantity: f64,
        current_price: f64,
        order_book: Option<&AggregatedOrderBook>,
    ) -> Result<Option<Trade>, TradingError> {
        let mut order = self
            .get_order(order_id)
            .ok_or_else(|| TradingError::OrderNotFound(order_id.to_string()))?;

        if order.time_in_force != TimeInForce::Ioc {
            return Err(TradingError::InvalidOrder(
                "Order is not IOC".to_string(),
            ));
        }

        if available_quantity <= 0.0 {
            // Nothing available - cancel entirely
            order.status = OrderStatus::Cancelled;
            order.updated_at = chrono::Utc::now().timestamp_millis();

            self.sqlite.update_order(&order)?;
            self.orders.insert(order.id.clone(), order.clone());

            self.broadcast_order_update(&order, OrderUpdateType::Cancelled);
            return Ok(None);
        }

        // Fill what's available
        let fill_quantity = available_quantity.min(order.quantity);

        // Create a temporary order with reduced quantity for execution
        let original_quantity = order.quantity;
        order.quantity = fill_quantity;

        // Execute the partial fill
        let trade = self.execute_market_order(order_id, current_price, order_book)?;

        // If there was remaining quantity, the order should be cancelled
        // (the execute_market_order already handles status updates)
        if fill_quantity < original_quantity {
            // Order was partially filled and should now be marked as such
            // or cancelled for the remainder
            if let Some(mut updated_order) = self.get_order(order_id) {
                if updated_order.status == OrderStatus::PartiallyFilled {
                    updated_order.status = OrderStatus::Cancelled;
                    updated_order.updated_at = chrono::Utc::now().timestamp_millis();

                    self.sqlite.update_order(&updated_order)?;
                    self.orders.insert(updated_order.id.clone(), updated_order.clone());

                    self.broadcast_order_update(&updated_order, OrderUpdateType::Cancelled);
                }
            }
        }

        Ok(Some(trade))
    }

    /// Activate bracket order SL/TP orders after entry fills.
    pub fn activate_bracket_orders(&self, bracket_id: &str) -> Result<Vec<Order>, TradingError> {
        let mut activated = Vec::new();

        // Find all orders in this bracket
        let bracket_orders: Vec<Order> = self
            .orders
            .iter()
            .filter(|entry| {
                entry.value().bracket_id.as_deref() == Some(bracket_id)
            })
            .map(|entry| entry.value().clone())
            .collect();

        // Check if entry is filled
        let entry_filled = bracket_orders
            .iter()
            .find(|o| o.bracket_role == Some(BracketRole::Entry))
            .map(|o| o.status == OrderStatus::Filled)
            .unwrap_or(false);

        if !entry_filled {
            return Ok(activated);
        }

        // Activate SL and TP orders
        for mut order in bracket_orders {
            if order.bracket_role == Some(BracketRole::StopLoss)
                || order.bracket_role == Some(BracketRole::TakeProfit)
            {
                if order.status == OrderStatus::Pending {
                    order.status = OrderStatus::Open;
                    order.updated_at = chrono::Utc::now().timestamp_millis();

                    self.sqlite.update_order(&order)?;
                    self.orders.insert(order.id.clone(), order.clone());

                    self.broadcast_order_update(&order, OrderUpdateType::Created);
                    activated.push(order);
                }
            }
        }

        info!(
            "Activated {} bracket orders for bracket {}",
            activated.len(),
            bracket_id
        );
        Ok(activated)
    }

    /// Place a bracket order (entry + stop loss + take profit).
    pub fn place_bracket_order(
        &self,
        portfolio_id: &str,
        symbol: &str,
        asset_class: AssetClass,
        entry_side: OrderSide,
        quantity: f64,
        entry_price: Option<f64>,
        stop_loss_price: f64,
        take_profit_price: f64,
        leverage: f64,
    ) -> Result<BracketOrder, TradingError> {
        // Validate portfolio
        let portfolio = self
            .get_portfolio(portfolio_id)
            .ok_or_else(|| TradingError::PortfolioNotFound(portfolio_id.to_string()))?;

        if portfolio.is_stopped() {
            return Err(TradingError::PortfolioStopped);
        }

        // Create bracket order
        let mut bracket = BracketOrder::new(
            portfolio_id.to_string(),
            symbol.to_string(),
            asset_class,
            entry_side,
            quantity,
            entry_price,
            stop_loss_price,
            take_profit_price,
        );

        // Set leverage
        bracket.entry.leverage = leverage;
        bracket.stop_loss.leverage = leverage;
        bracket.take_profit.leverage = leverage;

        // Validate entry order
        self.validate_order(&bracket.entry, &portfolio)?;

        // Persist all orders
        self.sqlite.create_order(&bracket.entry)?;
        self.sqlite.create_order(&bracket.stop_loss)?;
        self.sqlite.create_order(&bracket.take_profit)?;

        // Cache orders
        self.orders.insert(bracket.entry.id.clone(), bracket.entry.clone());
        self.orders.insert(bracket.stop_loss.id.clone(), bracket.stop_loss.clone());
        self.orders.insert(bracket.take_profit.id.clone(), bracket.take_profit.clone());

        // Broadcast entry order creation
        self.broadcast_order_update(&bracket.entry, OrderUpdateType::Created);

        info!(
            "Placed bracket order {} with entry {}, SL {}, TP {}",
            bracket.bracket_id, bracket.entry.id, bracket.stop_loss.id, bracket.take_profit.id
        );

        Ok(bracket)
    }

    /// Place an OCO (One-Cancels-Other) order pair.
    pub fn place_oco_order(
        &self,
        portfolio_id: &str,
        symbol: &str,
        asset_class: AssetClass,
        side: OrderSide,
        quantity: f64,
        stop_loss_price: f64,
        take_profit_price: f64,
    ) -> Result<OcoOrder, TradingError> {
        // Validate portfolio
        let portfolio = self
            .get_portfolio(portfolio_id)
            .ok_or_else(|| TradingError::PortfolioNotFound(portfolio_id.to_string()))?;

        if portfolio.is_stopped() {
            return Err(TradingError::PortfolioStopped);
        }

        // Create OCO order
        let oco = OcoOrder::stop_loss_take_profit(
            portfolio_id.to_string(),
            symbol.to_string(),
            asset_class,
            side,
            quantity,
            stop_loss_price,
            take_profit_price,
        );

        // Validate both orders
        self.validate_order(&oco.order1, &portfolio)?;
        self.validate_order(&oco.order2, &portfolio)?;

        // Persist orders
        self.sqlite.create_order(&oco.order1)?;
        self.sqlite.create_order(&oco.order2)?;

        // Cache orders
        self.orders.insert(oco.order1.id.clone(), oco.order1.clone());
        self.orders.insert(oco.order2.id.clone(), oco.order2.clone());

        // Broadcast order creations
        self.broadcast_order_update(&oco.order1, OrderUpdateType::Created);
        self.broadcast_order_update(&oco.order2, OrderUpdateType::Created);

        info!(
            "Placed OCO order pair: {} (SL) and {} (TP)",
            oco.order1.id, oco.order2.id
        );

        Ok(oco)
    }

    // ==========================================================================
    // Order Monitoring (for price triggers)
    // ==========================================================================

    /// Check and execute triggered orders based on current price.
    pub fn check_triggered_orders(
        &self,
        symbol: &str,
        current_price: f64,
        order_book: Option<&AggregatedOrderBook>,
    ) -> Vec<Result<Trade, TradingError>> {
        let mut results = Vec::new();

        // Find all open orders for this symbol
        let triggered_orders: Vec<Order> = self
            .orders
            .iter()
            .filter(|entry| {
                let order = entry.value();
                order.symbol == symbol && !order.is_terminal() && self.should_trigger(order, current_price)
            })
            .map(|entry| entry.value().clone())
            .collect();

        for order in triggered_orders {
            let result = self.execute_market_order(&order.id, current_price, order_book);
            results.push(result);
        }

        results
    }

    /// Check if an order should trigger at the given price.
    fn should_trigger(&self, order: &Order, price: f64) -> bool {
        match order.order_type {
            OrderType::Market => order.status == OrderStatus::Pending,
            OrderType::Limit => {
                if let Some(limit_price) = order.price {
                    match order.side {
                        OrderSide::Buy => price <= limit_price,
                        OrderSide::Sell => price >= limit_price,
                    }
                } else {
                    false
                }
            }
            OrderType::StopLoss => {
                if let Some(stop_price) = order.stop_price {
                    match order.side {
                        OrderSide::Sell => price <= stop_price, // Long position stop
                        OrderSide::Buy => price >= stop_price,  // Short position stop
                    }
                } else {
                    false
                }
            }
            OrderType::TakeProfit => {
                if let Some(stop_price) = order.stop_price {
                    match order.side {
                        OrderSide::Sell => price >= stop_price, // Long position TP
                        OrderSide::Buy => price <= stop_price,  // Short position TP
                    }
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Check positions for stop loss and take profit triggers.
    pub fn check_position_triggers(
        &self,
        symbol: &str,
        current_price: f64,
    ) -> Vec<Result<Trade, TradingError>> {
        let mut results = Vec::new();

        // Find positions that should trigger
        let triggered: Vec<Position> = self
            .positions
            .iter()
            .filter(|entry| {
                let pos = entry.value();
                pos.symbol == symbol && (pos.should_stop_loss() || pos.should_take_profit() || pos.should_liquidate())
            })
            .map(|entry| {
                let mut pos = entry.value().clone();
                pos.update_price(current_price);
                pos
            })
            .filter(|pos| pos.should_stop_loss() || pos.should_take_profit() || pos.should_liquidate())
            .collect();

        for position in triggered {
            let is_liquidation = position.should_liquidate();
            let is_stop_loss = position.should_stop_loss();
            let reason = if is_liquidation {
                "liquidation"
            } else if is_stop_loss {
                "stop_loss"
            } else {
                "take_profit"
            };

            info!(
                "Position {} triggered {} at price {}",
                position.id, reason, current_price
            );

            // Broadcast liquidation alert before closing
            if is_liquidation {
                let loss_amount = position.unrealized_pnl.min(0.0).abs();
                self.broadcast_liquidation_alert(
                    &position.portfolio_id,
                    &position.id,
                    &position.symbol,
                    current_price,
                    loss_amount,
                );
            }

            let result = self.close_position(&position.id, current_price);

            // Broadcast appropriate position update type
            if let Ok(ref _trade) = result {
                let update_type = if is_liquidation {
                    PositionUpdateType::Liquidated
                } else if is_stop_loss {
                    PositionUpdateType::StopLossTriggered
                } else {
                    PositionUpdateType::TakeProfitTriggered
                };
                self.broadcast_position_update(&position, update_type);
            }

            results.push(result);
        }

        results
    }

    /// Load open orders into cache (call on startup).
    pub fn load_open_orders(&self) {
        // Get all portfolios
        let conn_check = self.sqlite.get_connection();
        if conn_check.is_none() {
            warn!("SQLite not available, skipping order load");
            return;
        }

        // We'd need to iterate all portfolios - for now just skip
        // This would typically be done per-user on login
        debug!("Order cache ready for lazy loading");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AssetClass;

    fn create_test_service() -> TradingService {
        let sqlite = Arc::new(SqliteStore::new_in_memory().unwrap());
        TradingService::new(sqlite)
    }

    #[test]
    fn test_create_portfolio() {
        let service = create_test_service();

        let portfolio = service
            .create_portfolio("user123", "Test Portfolio", None, None)
            .unwrap();

        assert_eq!(portfolio.user_id, "user123");
        assert_eq!(portfolio.name, "Test Portfolio");
        assert_eq!(portfolio.starting_balance, 5_000_000.0);

        // Verify it's in the cache
        let loaded = service.get_portfolio(&portfolio.id).unwrap();
        assert_eq!(loaded.id, portfolio.id);
    }

    #[test]
    fn test_place_market_order() {
        let service = create_test_service();

        let portfolio = service
            .create_portfolio("user123", "Trading", None, None)
            .unwrap();

        let request = PlaceOrderRequest {
            portfolio_id: portfolio.id.clone(),
            symbol: "BTC".to_string(),
            asset_class: AssetClass::CryptoSpot,
            side: OrderSide::Buy,
            order_type: OrderType::Market,
            quantity: 1.0,
            price: None,
            stop_price: None,
            trail_amount: None,
            trail_percent: None,
            time_in_force: None,
            leverage: None,
            stop_loss: None,
            take_profit: None,
            client_order_id: None,
        };

        let order = service.place_order(request).unwrap();

        assert_eq!(order.symbol, "BTC");
        assert_eq!(order.side, OrderSide::Buy);
        assert_eq!(order.status, OrderStatus::Pending);
    }

    #[test]
    fn test_execute_market_order() {
        let service = create_test_service();

        let portfolio = service
            .create_portfolio("user123", "Trading", None, None)
            .unwrap();

        let request = PlaceOrderRequest {
            portfolio_id: portfolio.id.clone(),
            symbol: "BTC".to_string(),
            asset_class: AssetClass::CryptoSpot,
            side: OrderSide::Buy,
            order_type: OrderType::Market,
            quantity: 1.0,
            price: None,
            stop_price: None,
            trail_amount: None,
            trail_percent: None,
            time_in_force: None,
            leverage: None,
            stop_loss: None,
            take_profit: None,
            client_order_id: None,
        };

        let order = service.place_order(request).unwrap();
        let trade = service.execute_market_order(&order.id, 50000.0, None).unwrap();

        assert_eq!(trade.symbol, "BTC");
        assert!(trade.price > 0.0);
        assert!(trade.fee > 0.0);

        // Check order is filled
        let filled_order = service.get_order(&order.id).unwrap();
        assert_eq!(filled_order.status, OrderStatus::Filled);

        // Check position was created
        let positions = service.get_positions(&portfolio.id);
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].symbol, "BTC");
        assert_eq!(positions[0].side, PositionSide::Long);
    }

    #[test]
    fn test_close_position() {
        let service = create_test_service();

        let portfolio = service
            .create_portfolio("user123", "Trading", None, None)
            .unwrap();

        // Open position
        let request = PlaceOrderRequest {
            portfolio_id: portfolio.id.clone(),
            symbol: "ETH".to_string(),
            asset_class: AssetClass::CryptoSpot,
            side: OrderSide::Buy,
            order_type: OrderType::Market,
            quantity: 10.0,
            price: None,
            stop_price: None,
            trail_amount: None,
            trail_percent: None,
            time_in_force: None,
            leverage: None,
            stop_loss: None,
            take_profit: None,
            client_order_id: None,
        };

        let order = service.place_order(request).unwrap();
        service.execute_market_order(&order.id, 2500.0, None).unwrap();

        let positions = service.get_positions(&portfolio.id);
        assert_eq!(positions.len(), 1);

        // Close position at higher price (profit)
        let close_trade = service
            .close_position(&positions[0].id, 2600.0)
            .unwrap();

        assert_eq!(close_trade.side, OrderSide::Sell);

        // Check position is closed
        let remaining_positions = service.get_positions(&portfolio.id);
        assert_eq!(remaining_positions.len(), 0);

        // Check realized P&L
        let updated_portfolio = service.get_portfolio(&portfolio.id).unwrap();
        assert!(updated_portfolio.realized_pnl > 0.0);
    }

    #[test]
    fn test_leveraged_position() {
        let service = create_test_service();

        let portfolio = service
            .create_portfolio("user123", "Perps", None, None)
            .unwrap();

        let request = PlaceOrderRequest {
            portfolio_id: portfolio.id.clone(),
            symbol: "BTC".to_string(),
            asset_class: AssetClass::Perp,
            side: OrderSide::Buy,
            order_type: OrderType::Market,
            quantity: 1.0,
            price: None,
            stop_price: None,
            trail_amount: None,
            trail_percent: None,
            time_in_force: None,
            leverage: Some(10.0),
            stop_loss: None,
            take_profit: None,
            client_order_id: None,
        };

        let order = service.place_order(request).unwrap();
        service.execute_market_order(&order.id, 50000.0, None).unwrap();

        let positions = service.get_positions(&portfolio.id);
        assert_eq!(positions[0].leverage, 10.0);
        assert!(positions[0].liquidation_price.is_some());

        // Margin used should be position_size / leverage
        assert!((positions[0].margin_used - 5000.0).abs() < 100.0);
    }

    #[test]
    fn test_cancel_order() {
        let service = create_test_service();

        let portfolio = service
            .create_portfolio("user123", "Trading", None, None)
            .unwrap();

        let request = PlaceOrderRequest {
            portfolio_id: portfolio.id.clone(),
            symbol: "BTC".to_string(),
            asset_class: AssetClass::CryptoSpot,
            side: OrderSide::Buy,
            order_type: OrderType::Limit,
            quantity: 1.0,
            price: Some(40000.0), // Below current price
            stop_price: None,
            trail_amount: None,
            trail_percent: None,
            time_in_force: None,
            leverage: None,
            stop_loss: None,
            take_profit: None,
            client_order_id: None,
        };

        let order = service.place_order(request).unwrap();
        let cancelled = service.cancel_order(&order.id).unwrap();

        assert_eq!(cancelled.status, OrderStatus::Cancelled);
    }

    #[test]
    fn test_reset_portfolio() {
        let service = create_test_service();

        let portfolio = service
            .create_portfolio("user123", "Trading", None, None)
            .unwrap();

        // Create some positions
        let request = PlaceOrderRequest {
            portfolio_id: portfolio.id.clone(),
            symbol: "BTC".to_string(),
            asset_class: AssetClass::CryptoSpot,
            side: OrderSide::Buy,
            order_type: OrderType::Market,
            quantity: 1.0,
            price: None,
            stop_price: None,
            trail_amount: None,
            trail_percent: None,
            time_in_force: None,
            leverage: None,
            stop_loss: None,
            take_profit: None,
            client_order_id: None,
        };

        let order = service.place_order(request).unwrap();
        service.execute_market_order(&order.id, 50000.0, None).unwrap();

        // Reset
        let reset = service.reset_portfolio(&portfolio.id).unwrap();

        assert_eq!(reset.cash_balance, reset.starting_balance);
        assert_eq!(reset.unrealized_pnl, 0.0);

        let positions = service.get_positions(&portfolio.id);
        assert!(positions.is_empty());
    }

    #[test]
    fn test_insufficient_margin() {
        let service = create_test_service();

        let portfolio = service
            .create_portfolio("user123", "Small", None, None)
            .unwrap();

        // Try to buy more than we can afford
        let request = PlaceOrderRequest {
            portfolio_id: portfolio.id.clone(),
            symbol: "BTC".to_string(),
            asset_class: AssetClass::CryptoSpot,
            side: OrderSide::Buy,
            order_type: OrderType::Limit,
            quantity: 1000.0,       // Way too much
            price: Some(50000.0),   // $50M worth
            stop_price: None,
            trail_amount: None,
            trail_percent: None,
            time_in_force: None,
            leverage: None,
            stop_loss: None,
            take_profit: None,
            client_order_id: None,
        };

        let result = service.place_order(request);
        assert!(matches!(result, Err(TradingError::InsufficientMargin { .. })));
    }

    #[test]
    fn test_leverage_exceeded() {
        let service = create_test_service();

        let portfolio = service
            .create_portfolio("user123", "Trading", None, None)
            .unwrap();

        let request = PlaceOrderRequest {
            portfolio_id: portfolio.id.clone(),
            symbol: "BTC".to_string(),
            asset_class: AssetClass::CryptoSpot,
            side: OrderSide::Buy,
            order_type: OrderType::Market,
            quantity: 1.0,
            price: None,
            stop_price: None,
            trail_amount: None,
            trail_percent: None,
            time_in_force: None,
            leverage: Some(20.0), // Max for crypto spot is 10x
            stop_loss: None,
            take_profit: None,
            client_order_id: None,
        };

        let result = service.place_order(request);
        assert!(matches!(result, Err(TradingError::LeverageExceeded { .. })));
    }
}
