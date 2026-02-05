//! Trading Types
//!
//! Types for paper trading system including portfolios, orders, positions, and trades.

use serde::{Deserialize, Serialize};

// =============================================================================
// Enums
// =============================================================================

/// Asset type for trading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetClass {
    /// Cryptocurrency spot trading
    CryptoSpot,
    /// Stocks
    Stock,
    /// Exchange-traded funds
    Etf,
    /// Perpetual futures
    Perp,
    /// Options contracts
    Option,
    /// Foreign exchange
    Forex,
}

impl AssetClass {
    /// Get the maximum allowed leverage for this asset class.
    pub fn max_leverage(&self) -> f64 {
        match self {
            AssetClass::CryptoSpot => 100.0,
            AssetClass::Stock => 4.0,
            AssetClass::Etf => 4.0,
            AssetClass::Perp => 100.0,
            AssetClass::Option => 1.0, // Premium-based, no leverage
            AssetClass::Forex => 100.0,
        }
    }

    /// Get the initial margin requirement as a decimal (e.g., 0.1 = 10%).
    pub fn initial_margin(&self) -> f64 {
        match self {
            AssetClass::CryptoSpot => 0.01,  // 1% (100x max leverage)
            AssetClass::Stock => 0.25,       // 25%
            AssetClass::Etf => 0.25,         // 25%
            AssetClass::Perp => 0.01,        // 1% at max leverage
            AssetClass::Option => 1.0,       // 100% (premium)
            AssetClass::Forex => 0.01,       // 1% (100x max leverage)
        }
    }

    /// Get the maintenance margin requirement as a decimal.
    pub fn maintenance_margin(&self) -> f64 {
        match self {
            AssetClass::CryptoSpot => 0.005, // 0.5% (for 100x leverage)
            AssetClass::Stock => 0.25,       // 25%
            AssetClass::Etf => 0.25,         // 25%
            AssetClass::Perp => 0.005,       // 0.5%
            AssetClass::Option => 0.0,       // N/A
            AssetClass::Forex => 0.005,      // 0.5% (for 100x leverage)
        }
    }
}

impl std::fmt::Display for AssetClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssetClass::CryptoSpot => write!(f, "crypto_spot"),
            AssetClass::Stock => write!(f, "stock"),
            AssetClass::Etf => write!(f, "etf"),
            AssetClass::Perp => write!(f, "perp"),
            AssetClass::Option => write!(f, "option"),
            AssetClass::Forex => write!(f, "forex"),
        }
    }
}

/// Order side (buy or sell).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderSide {
    Buy,
    Sell,
}

impl std::fmt::Display for OrderSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderSide::Buy => write!(f, "buy"),
            OrderSide::Sell => write!(f, "sell"),
        }
    }
}

/// Order type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderType {
    /// Execute immediately at best available price
    Market,
    /// Execute at specified price or better
    Limit,
    /// Trigger sell when price drops to threshold
    StopLoss,
    /// Trigger sell when price reaches profit target
    TakeProfit,
    /// Stop loss that becomes a limit order when triggered
    StopLimit,
    /// Dynamic stop that follows price by fixed amount or %
    TrailingStop,
}

impl std::fmt::Display for OrderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderType::Market => write!(f, "market"),
            OrderType::Limit => write!(f, "limit"),
            OrderType::StopLoss => write!(f, "stop_loss"),
            OrderType::TakeProfit => write!(f, "take_profit"),
            OrderType::StopLimit => write!(f, "stop_limit"),
            OrderType::TrailingStop => write!(f, "trailing_stop"),
        }
    }
}

/// Order status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    /// Order is pending, waiting for conditions
    Pending,
    /// Order is open and active
    Open,
    /// Order is partially filled
    PartiallyFilled,
    /// Order is completely filled
    Filled,
    /// Order was cancelled
    Cancelled,
    /// Order expired (GTD)
    Expired,
    /// Order was rejected
    Rejected,
}

impl std::fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderStatus::Pending => write!(f, "pending"),
            OrderStatus::Open => write!(f, "open"),
            OrderStatus::PartiallyFilled => write!(f, "partially_filled"),
            OrderStatus::Filled => write!(f, "filled"),
            OrderStatus::Cancelled => write!(f, "cancelled"),
            OrderStatus::Expired => write!(f, "expired"),
            OrderStatus::Rejected => write!(f, "rejected"),
        }
    }
}

/// Time in force for orders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeInForce {
    /// Good till cancelled - remains active until filled or cancelled
    Gtc,
    /// Good till date - expires at specified date/time
    Gtd,
    /// Fill or kill - execute entire order immediately or cancel
    Fok,
    /// Immediate or cancel - fill what's available, cancel rest
    Ioc,
}

impl Default for TimeInForce {
    fn default() -> Self {
        TimeInForce::Gtc
    }
}

impl std::fmt::Display for TimeInForce {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TimeInForce::Gtc => write!(f, "gtc"),
            TimeInForce::Gtd => write!(f, "gtd"),
            TimeInForce::Fok => write!(f, "fok"),
            TimeInForce::Ioc => write!(f, "ioc"),
        }
    }
}

/// Position side (long or short).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PositionSide {
    Long,
    Short,
}

impl std::fmt::Display for PositionSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PositionSide::Long => write!(f, "long"),
            PositionSide::Short => write!(f, "short"),
        }
    }
}

/// Margin mode for leveraged positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarginMode {
    /// Each position has independent margin
    Isolated,
    /// All positions share account margin
    Cross,
}

impl Default for MarginMode {
    fn default() -> Self {
        MarginMode::Isolated
    }
}

impl std::fmt::Display for MarginMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MarginMode::Isolated => write!(f, "isolated"),
            MarginMode::Cross => write!(f, "cross"),
        }
    }
}

/// Cost basis calculation method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CostBasisMethod {
    /// First in, first out
    Fifo,
    /// Last in, first out
    Lifo,
    /// Weighted average cost
    Average,
}

impl Default for CostBasisMethod {
    fn default() -> Self {
        CostBasisMethod::Fifo
    }
}

impl std::fmt::Display for CostBasisMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CostBasisMethod::Fifo => write!(f, "fifo"),
            CostBasisMethod::Lifo => write!(f, "lifo"),
            CostBasisMethod::Average => write!(f, "average"),
        }
    }
}

// =============================================================================
// Portfolio Types
// =============================================================================

/// Risk settings for a portfolio.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RiskSettings {
    /// Maximum position size as percentage of portfolio (0.0-1.0)
    #[serde(default = "default_max_position_size")]
    pub max_position_size_pct: f64,
    /// Maximum daily loss as percentage of portfolio (0.0-1.0)
    #[serde(default = "default_daily_loss_limit")]
    pub daily_loss_limit_pct: f64,
    /// Maximum number of concurrent open positions
    #[serde(default = "default_max_open_positions")]
    pub max_open_positions: u32,
    /// Maximum risk per trade as percentage of portfolio (0.0-1.0)
    #[serde(default = "default_risk_per_trade")]
    pub risk_per_trade_pct: f64,
    /// Portfolio stop - pause trading at this drawdown percentage
    #[serde(default = "default_portfolio_stop")]
    pub portfolio_stop_pct: f64,
}

fn default_max_position_size() -> f64 { 0.25 }
fn default_daily_loss_limit() -> f64 { 0.10 }
fn default_max_open_positions() -> u32 { 20 }
fn default_risk_per_trade() -> f64 { 0.02 }
fn default_portfolio_stop() -> f64 { 0.25 }

impl Default for RiskSettings {
    fn default() -> Self {
        Self {
            max_position_size_pct: default_max_position_size(),
            daily_loss_limit_pct: default_daily_loss_limit(),
            max_open_positions: default_max_open_positions(),
            risk_per_trade_pct: default_risk_per_trade(),
            portfolio_stop_pct: default_portfolio_stop(),
        }
    }
}

/// User's paper trading portfolio.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Portfolio {
    /// Unique portfolio ID
    pub id: String,
    /// Owner's user ID (public key)
    pub user_id: String,
    /// Portfolio name
    pub name: String,
    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Base currency for the portfolio
    #[serde(default = "default_base_currency")]
    pub base_currency: String,
    /// Initial starting balance
    pub starting_balance: f64,
    /// Current cash balance (not in positions)
    pub cash_balance: f64,
    /// Total margin currently used
    pub margin_used: f64,
    /// Available margin for new positions
    pub margin_available: f64,
    /// Unrealized P&L across all open positions
    pub unrealized_pnl: f64,
    /// Realized P&L from closed positions
    pub realized_pnl: f64,
    /// Total portfolio value (cash + unrealized P&L)
    pub total_value: f64,
    /// Total number of completed trades
    #[serde(default)]
    pub total_trades: u64,
    /// Number of winning trades
    #[serde(default)]
    pub winning_trades: u64,
    /// Cost basis calculation method
    #[serde(default)]
    pub cost_basis_method: CostBasisMethod,
    /// Risk settings
    #[serde(default)]
    pub risk_settings: RiskSettings,
    /// Whether this is a competition portfolio
    #[serde(default)]
    pub is_competition: bool,
    /// Competition ID if this is a competition portfolio
    #[serde(skip_serializing_if = "Option::is_none")]
    pub competition_id: Option<String>,
    /// When portfolio was created (ms)
    pub created_at: i64,
    /// When portfolio was last updated (ms)
    pub updated_at: i64,
}

fn default_base_currency() -> String {
    "USD".to_string()
}

impl Portfolio {
    /// Create a new portfolio with default starting balance of $250,000.
    pub fn new(user_id: String, name: String) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        let starting_balance = 250_000.0;

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            user_id,
            name,
            description: None,
            base_currency: "USD".to_string(),
            starting_balance,
            cash_balance: starting_balance,
            margin_used: 0.0,
            margin_available: starting_balance,
            unrealized_pnl: 0.0,
            realized_pnl: 0.0,
            total_value: starting_balance,
            total_trades: 0,
            winning_trades: 0,
            cost_basis_method: CostBasisMethod::default(),
            risk_settings: RiskSettings::default(),
            is_competition: false,
            competition_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Calculate the current equity (cash + unrealized P&L).
    pub fn equity(&self) -> f64 {
        self.cash_balance + self.unrealized_pnl
    }

    /// Calculate the margin level as a percentage.
    pub fn margin_level(&self) -> f64 {
        if self.margin_used > 0.0 {
            (self.equity() / self.margin_used) * 100.0
        } else {
            f64::INFINITY
        }
    }

    /// Calculate the total return percentage.
    pub fn total_return_pct(&self) -> f64 {
        if self.starting_balance > 0.0 {
            ((self.total_value - self.starting_balance) / self.starting_balance) * 100.0
        } else {
            0.0
        }
    }

    /// Check if the portfolio has hit its stop loss.
    pub fn is_stopped(&self) -> bool {
        let drawdown = (self.starting_balance - self.total_value) / self.starting_balance;
        drawdown >= self.risk_settings.portfolio_stop_pct
    }

    /// Update portfolio values. Call this after position changes.
    pub fn recalculate(&mut self) {
        // Total value = cash + margin_used (value in positions) + unrealized P&L
        self.total_value = self.cash_balance + self.margin_used + self.unrealized_pnl;
        self.margin_available = self.cash_balance - self.margin_used;
        if self.margin_available < 0.0 {
            self.margin_available = 0.0;
        }
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }
}

// =============================================================================
// Order Types
// =============================================================================

/// A single fill event for an order.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fill {
    /// Unique fill ID
    pub id: String,
    /// Quantity filled
    pub quantity: f64,
    /// Price at which filled
    pub price: f64,
    /// Fee charged for this fill
    pub fee: f64,
    /// Timestamp of fill (ms)
    pub filled_at: i64,
}

impl Fill {
    /// Create a new fill.
    pub fn new(quantity: f64, price: f64, fee: f64) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            quantity,
            price,
            fee,
            filled_at: chrono::Utc::now().timestamp_millis(),
        }
    }
}

/// A trading order.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    /// Unique order ID
    pub id: String,
    /// Portfolio this order belongs to
    pub portfolio_id: String,
    /// Symbol being traded (e.g., "BTC", "AAPL")
    pub symbol: String,
    /// Asset class
    pub asset_class: AssetClass,
    /// Buy or sell
    pub side: OrderSide,
    /// Order type
    pub order_type: OrderType,
    /// Total quantity to fill
    pub quantity: f64,
    /// Quantity already filled
    pub filled_quantity: f64,
    /// Limit price (for limit orders)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<f64>,
    /// Stop/trigger price (for stop orders)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_price: Option<f64>,
    /// Trailing amount (fixed value for trailing stop)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trail_amount: Option<f64>,
    /// Trailing percentage (for trailing stop)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trail_percent: Option<f64>,
    /// Time in force
    #[serde(default)]
    pub time_in_force: TimeInForce,
    /// Current order status
    pub status: OrderStatus,
    /// Linked order ID (for OCO orders)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linked_order_id: Option<String>,
    /// Bracket order ID (for bracket orders)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bracket_id: Option<String>,
    /// Leverage for this order (1.0 = no leverage)
    #[serde(default = "default_leverage")]
    pub leverage: f64,
    /// List of fills for this order
    #[serde(default)]
    pub fills: Vec<Fill>,
    /// Average fill price
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_fill_price: Option<f64>,
    /// Total fees paid
    #[serde(default)]
    pub total_fees: f64,
    /// Optional client-provided order ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_order_id: Option<String>,
    /// When order was created (ms)
    pub created_at: i64,
    /// When order was last updated (ms)
    pub updated_at: i64,
    /// When order expires (for GTD orders)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
    /// Highest price seen (for trailing stop - long positions)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trail_high_price: Option<f64>,
    /// Lowest price seen (for trailing stop - short positions)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trail_low_price: Option<f64>,
    /// Whether this is part of a bracket order (entry, stop_loss, or take_profit)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bracket_role: Option<BracketRole>,
}

/// Role in a bracket order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BracketRole {
    /// Entry order for the bracket
    Entry,
    /// Stop loss order (activated after entry fills)
    StopLoss,
    /// Take profit order (activated after entry fills)
    TakeProfit,
}

impl std::fmt::Display for BracketRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BracketRole::Entry => write!(f, "entry"),
            BracketRole::StopLoss => write!(f, "stop_loss"),
            BracketRole::TakeProfit => write!(f, "take_profit"),
        }
    }
}

fn default_leverage() -> f64 { 1.0 }

impl Order {
    /// Create a new market order.
    pub fn market(
        portfolio_id: String,
        symbol: String,
        asset_class: AssetClass,
        side: OrderSide,
        quantity: f64,
    ) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            portfolio_id,
            symbol,
            asset_class,
            side,
            order_type: OrderType::Market,
            quantity,
            filled_quantity: 0.0,
            price: None,
            stop_price: None,
            trail_amount: None,
            trail_percent: None,
            time_in_force: TimeInForce::Gtc,
            status: OrderStatus::Pending,
            linked_order_id: None,
            bracket_id: None,
            leverage: 1.0,
            fills: Vec::new(),
            avg_fill_price: None,
            total_fees: 0.0,
            client_order_id: None,
            created_at: now,
            updated_at: now,
            expires_at: None,
            trail_high_price: None,
            trail_low_price: None,
            bracket_role: None,
        }
    }

    /// Create a new limit order.
    pub fn limit(
        portfolio_id: String,
        symbol: String,
        asset_class: AssetClass,
        side: OrderSide,
        quantity: f64,
        price: f64,
    ) -> Self {
        let mut order = Self::market(portfolio_id, symbol, asset_class, side, quantity);
        order.order_type = OrderType::Limit;
        order.price = Some(price);
        order
    }

    /// Create a new stop loss order.
    pub fn stop_loss(
        portfolio_id: String,
        symbol: String,
        asset_class: AssetClass,
        side: OrderSide,
        quantity: f64,
        stop_price: f64,
    ) -> Self {
        let mut order = Self::market(portfolio_id, symbol, asset_class, side, quantity);
        order.order_type = OrderType::StopLoss;
        order.stop_price = Some(stop_price);
        order
    }

    /// Create a new take profit order.
    pub fn take_profit(
        portfolio_id: String,
        symbol: String,
        asset_class: AssetClass,
        side: OrderSide,
        quantity: f64,
        stop_price: f64,
    ) -> Self {
        let mut order = Self::market(portfolio_id, symbol, asset_class, side, quantity);
        order.order_type = OrderType::TakeProfit;
        order.stop_price = Some(stop_price);
        order
    }

    /// Check if order is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            OrderStatus::Filled | OrderStatus::Cancelled | OrderStatus::Expired | OrderStatus::Rejected
        )
    }

    /// Check if order can be cancelled.
    pub fn can_cancel(&self) -> bool {
        matches!(
            self.status,
            OrderStatus::Pending | OrderStatus::Open | OrderStatus::PartiallyFilled
        )
    }

    /// Get remaining quantity to fill.
    pub fn remaining_quantity(&self) -> f64 {
        self.quantity - self.filled_quantity
    }

    /// Add a fill to this order.
    pub fn add_fill(&mut self, fill: Fill) {
        self.filled_quantity += fill.quantity;
        self.total_fees += fill.fee;
        self.fills.push(fill);

        // Recalculate average fill price
        let total_value: f64 = self.fills.iter().map(|f| f.price * f.quantity).sum();
        if self.filled_quantity > 0.0 {
            self.avg_fill_price = Some(total_value / self.filled_quantity);
        }

        // Update status
        if self.filled_quantity >= self.quantity {
            self.status = OrderStatus::Filled;
        } else if self.filled_quantity > 0.0 {
            self.status = OrderStatus::PartiallyFilled;
        }

        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// Create a new trailing stop order.
    pub fn trailing_stop(
        portfolio_id: String,
        symbol: String,
        asset_class: AssetClass,
        side: OrderSide,
        quantity: f64,
        trail_amount: Option<f64>,
        trail_percent: Option<f64>,
        initial_price: f64,
    ) -> Self {
        let mut order = Self::market(portfolio_id, symbol, asset_class, side, quantity);
        order.order_type = OrderType::TrailingStop;
        order.trail_amount = trail_amount;
        order.trail_percent = trail_percent;

        // Initialize tracking prices based on side
        match side {
            OrderSide::Sell => {
                // For sell trailing stop (protecting long), track high price
                order.trail_high_price = Some(initial_price);
                order.stop_price = Some(order.calculate_trailing_stop_price(initial_price));
            }
            OrderSide::Buy => {
                // For buy trailing stop (protecting short), track low price
                order.trail_low_price = Some(initial_price);
                order.stop_price = Some(order.calculate_trailing_stop_price(initial_price));
            }
        }
        order
    }

    /// Calculate trailing stop trigger price based on reference price.
    pub fn calculate_trailing_stop_price(&self, reference_price: f64) -> f64 {
        let trail_distance = if let Some(amount) = self.trail_amount {
            amount
        } else if let Some(percent) = self.trail_percent {
            reference_price * (percent / 100.0)
        } else {
            0.0
        };

        match self.side {
            OrderSide::Sell => reference_price - trail_distance,
            OrderSide::Buy => reference_price + trail_distance,
        }
    }

    /// Update trailing stop based on current price.
    /// Returns true if stop price was updated.
    pub fn update_trailing_stop(&mut self, current_price: f64) -> bool {
        if self.order_type != OrderType::TrailingStop {
            return false;
        }

        let mut updated = false;

        match self.side {
            OrderSide::Sell => {
                // For sell trailing stop, update if price makes new high
                let current_high = self.trail_high_price.unwrap_or(current_price);
                if current_price > current_high {
                    self.trail_high_price = Some(current_price);
                    self.stop_price = Some(self.calculate_trailing_stop_price(current_price));
                    updated = true;
                }
            }
            OrderSide::Buy => {
                // For buy trailing stop, update if price makes new low
                let current_low = self.trail_low_price.unwrap_or(current_price);
                if current_price < current_low {
                    self.trail_low_price = Some(current_price);
                    self.stop_price = Some(self.calculate_trailing_stop_price(current_price));
                    updated = true;
                }
            }
        }

        if updated {
            self.updated_at = chrono::Utc::now().timestamp_millis();
        }
        updated
    }

    /// Check if a GTD order has expired.
    pub fn is_expired(&self) -> bool {
        if self.time_in_force != TimeInForce::Gtd {
            return false;
        }
        if let Some(expires_at) = self.expires_at {
            chrono::Utc::now().timestamp_millis() >= expires_at
        } else {
            false
        }
    }

    /// Check if this order is part of an OCO pair.
    pub fn is_oco(&self) -> bool {
        self.linked_order_id.is_some() && self.bracket_id.is_none()
    }

    /// Check if this order is part of a bracket.
    pub fn is_bracket(&self) -> bool {
        self.bracket_id.is_some()
    }

    /// Create a stop-limit order.
    pub fn stop_limit(
        portfolio_id: String,
        symbol: String,
        asset_class: AssetClass,
        side: OrderSide,
        quantity: f64,
        stop_price: f64,
        limit_price: f64,
    ) -> Self {
        let mut order = Self::market(portfolio_id, symbol, asset_class, side, quantity);
        order.order_type = OrderType::StopLimit;
        order.stop_price = Some(stop_price);
        order.price = Some(limit_price);
        order
    }

    /// Set order as FOK (Fill or Kill).
    pub fn with_fok(mut self) -> Self {
        self.time_in_force = TimeInForce::Fok;
        self
    }

    /// Set order as IOC (Immediate or Cancel).
    pub fn with_ioc(mut self) -> Self {
        self.time_in_force = TimeInForce::Ioc;
        self
    }

    /// Set order as GTD (Good Till Date).
    pub fn with_gtd(mut self, expires_at: i64) -> Self {
        self.time_in_force = TimeInForce::Gtd;
        self.expires_at = Some(expires_at);
        self
    }

    /// Link this order to another for OCO behavior.
    pub fn with_linked_order(mut self, linked_order_id: String) -> Self {
        self.linked_order_id = Some(linked_order_id);
        self
    }

    /// Set bracket ID for bracket order grouping.
    pub fn with_bracket(mut self, bracket_id: String, role: BracketRole) -> Self {
        self.bracket_id = Some(bracket_id);
        self.bracket_role = Some(role);
        self
    }
}

/// Result of creating a bracket order.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BracketOrder {
    /// Unique bracket ID
    pub bracket_id: String,
    /// Entry order
    pub entry: Order,
    /// Stop loss order (pending until entry fills)
    pub stop_loss: Order,
    /// Take profit order (pending until entry fills)
    pub take_profit: Order,
}

impl BracketOrder {
    /// Create a new bracket order with entry, stop loss, and take profit.
    pub fn new(
        portfolio_id: String,
        symbol: String,
        asset_class: AssetClass,
        entry_side: OrderSide,
        quantity: f64,
        entry_price: Option<f64>,  // None for market entry
        stop_loss_price: f64,
        take_profit_price: f64,
    ) -> Self {
        let bracket_id = uuid::Uuid::new_v4().to_string();
        let exit_side = match entry_side {
            OrderSide::Buy => OrderSide::Sell,
            OrderSide::Sell => OrderSide::Buy,
        };

        // Create entry order
        let mut entry = if let Some(price) = entry_price {
            Order::limit(
                portfolio_id.clone(),
                symbol.clone(),
                asset_class,
                entry_side,
                quantity,
                price,
            )
        } else {
            Order::market(
                portfolio_id.clone(),
                symbol.clone(),
                asset_class,
                entry_side,
                quantity,
            )
        };
        entry.bracket_id = Some(bracket_id.clone());
        entry.bracket_role = Some(BracketRole::Entry);

        // Create stop loss order (pending until entry fills)
        let mut stop_loss = Order::stop_loss(
            portfolio_id.clone(),
            symbol.clone(),
            asset_class,
            exit_side,
            quantity,
            stop_loss_price,
        );
        stop_loss.bracket_id = Some(bracket_id.clone());
        stop_loss.bracket_role = Some(BracketRole::StopLoss);
        stop_loss.status = OrderStatus::Pending; // Will be activated when entry fills

        // Create take profit order (pending until entry fills)
        let mut take_profit = Order::take_profit(
            portfolio_id,
            symbol,
            asset_class,
            exit_side,
            quantity,
            take_profit_price,
        );
        take_profit.bracket_id = Some(bracket_id.clone());
        take_profit.bracket_role = Some(BracketRole::TakeProfit);
        take_profit.status = OrderStatus::Pending; // Will be activated when entry fills

        // Link SL and TP as OCO
        stop_loss.linked_order_id = Some(take_profit.id.clone());
        take_profit.linked_order_id = Some(stop_loss.id.clone());

        Self {
            bracket_id,
            entry,
            stop_loss,
            take_profit,
        }
    }
}

/// Result of creating an OCO (One-Cancels-Other) order pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcoOrder {
    /// First order
    pub order1: Order,
    /// Second order (cancels when first fills)
    pub order2: Order,
}

impl OcoOrder {
    /// Create an OCO pair from two orders.
    pub fn new(mut order1: Order, mut order2: Order) -> Self {
        order1.linked_order_id = Some(order2.id.clone());
        order2.linked_order_id = Some(order1.id.clone());
        Self { order1, order2 }
    }

    /// Create a common OCO pattern: stop loss + take profit.
    pub fn stop_loss_take_profit(
        portfolio_id: String,
        symbol: String,
        asset_class: AssetClass,
        side: OrderSide,  // Exit side (opposite of position)
        quantity: f64,
        stop_loss_price: f64,
        take_profit_price: f64,
    ) -> Self {
        let stop_loss = Order::stop_loss(
            portfolio_id.clone(),
            symbol.clone(),
            asset_class,
            side,
            quantity,
            stop_loss_price,
        );
        let take_profit = Order::take_profit(
            portfolio_id,
            symbol,
            asset_class,
            side,
            quantity,
            take_profit_price,
        );
        Self::new(stop_loss, take_profit)
    }
}

// =============================================================================
// Perpetual Futures Types
// =============================================================================

/// Leverage tier based on position size (for perps).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LeverageTier {
    /// Maximum position size in USD for this tier
    pub max_position_size: f64,
    /// Maximum leverage allowed for this tier
    pub max_leverage: f64,
    /// Initial margin rate for this tier
    pub initial_margin_rate: f64,
    /// Maintenance margin rate for this tier
    pub maintenance_margin_rate: f64,
}

impl LeverageTier {
    /// Get the leverage tier for a given position size.
    pub fn for_position_size(position_size: f64) -> Self {
        // Tier system from TradingPlan.md:
        // Tier 1: < $50,000 = 100x
        // Tier 2: < $250,000 = 50x
        // Tier 3: < $1,000,000 = 20x
        // Tier 4: < $5,000,000 = 10x
        // Tier 5: > $5,000,000 = 5x
        if position_size < 50_000.0 {
            Self {
                max_position_size: 50_000.0,
                max_leverage: 100.0,
                initial_margin_rate: 0.01,    // 1%
                maintenance_margin_rate: 0.005, // 0.5%
            }
        } else if position_size < 250_000.0 {
            Self {
                max_position_size: 250_000.0,
                max_leverage: 50.0,
                initial_margin_rate: 0.02,    // 2%
                maintenance_margin_rate: 0.01, // 1%
            }
        } else if position_size < 1_000_000.0 {
            Self {
                max_position_size: 1_000_000.0,
                max_leverage: 20.0,
                initial_margin_rate: 0.05,    // 5%
                maintenance_margin_rate: 0.025, // 2.5%
            }
        } else if position_size < 5_000_000.0 {
            Self {
                max_position_size: 5_000_000.0,
                max_leverage: 10.0,
                initial_margin_rate: 0.10,    // 10%
                maintenance_margin_rate: 0.05, // 5%
            }
        } else {
            Self {
                max_position_size: f64::INFINITY,
                max_leverage: 5.0,
                initial_margin_rate: 0.20,    // 20%
                maintenance_margin_rate: 0.10, // 10%
            }
        }
    }
}

/// Funding rate for a perpetual futures symbol.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FundingRate {
    /// Symbol (e.g., "BTC-PERP")
    pub symbol: String,
    /// Current funding rate (e.g., 0.0001 = 0.01%)
    pub rate: f64,
    /// Predicted next funding rate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub predicted_rate: Option<f64>,
    /// Index price (spot price reference)
    pub index_price: f64,
    /// Mark price (used for liquidation)
    pub mark_price: f64,
    /// Next funding time (UTC timestamp ms)
    pub next_funding_time: i64,
    /// Time interval between funding (8 hours = 28800000 ms)
    pub funding_interval_ms: i64,
    /// When this rate was calculated
    pub timestamp: i64,
}

impl FundingRate {
    /// Create a new funding rate.
    pub fn new(symbol: String, rate: f64, index_price: f64, mark_price: f64) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        // Calculate next funding time (00:00, 08:00, 16:00 UTC)
        let eight_hours_ms = 8 * 60 * 60 * 1000;
        let next_funding_time = ((now / eight_hours_ms) + 1) * eight_hours_ms;

        Self {
            symbol,
            rate,
            predicted_rate: None,
            index_price,
            mark_price,
            next_funding_time,
            funding_interval_ms: eight_hours_ms,
            timestamp: now,
        }
    }

    /// Calculate funding payment for a position.
    /// Returns positive if position pays funding, negative if receives.
    pub fn calculate_payment(&self, position_size: f64, side: PositionSide) -> f64 {
        let payment = position_size * self.rate;
        match side {
            // Longs pay when rate is positive
            PositionSide::Long => payment,
            // Shorts receive when rate is positive
            PositionSide::Short => -payment,
        }
    }

    /// Check if funding should be applied now.
    pub fn should_apply_funding(&self) -> bool {
        let now = chrono::Utc::now().timestamp_millis();
        now >= self.next_funding_time
    }
}

/// A funding payment record.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FundingPayment {
    /// Unique payment ID
    pub id: String,
    /// Position ID this payment applies to
    pub position_id: String,
    /// Portfolio ID
    pub portfolio_id: String,
    /// Symbol
    pub symbol: String,
    /// Position size at time of payment
    pub position_size: f64,
    /// Position side
    pub side: PositionSide,
    /// Funding rate applied
    pub funding_rate: f64,
    /// Payment amount (positive = paid, negative = received)
    pub payment: f64,
    /// Timestamp of payment
    pub paid_at: i64,
}

impl FundingPayment {
    /// Create a new funding payment.
    pub fn new(
        position_id: String,
        portfolio_id: String,
        symbol: String,
        position_size: f64,
        side: PositionSide,
        funding_rate: f64,
    ) -> Self {
        let payment = match side {
            PositionSide::Long => position_size * funding_rate,
            PositionSide::Short => -(position_size * funding_rate),
        };

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            position_id,
            portfolio_id,
            symbol,
            position_size,
            side,
            funding_rate,
            payment,
            paid_at: chrono::Utc::now().timestamp_millis(),
        }
    }
}

// =============================================================================
// Liquidation Types
// =============================================================================

/// Liquidation warning level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiquidationWarningLevel {
    /// 80% of maintenance margin used
    Warning80,
    /// 90% of maintenance margin used
    Warning90,
    /// 95% of maintenance margin used (final warning)
    Warning95,
    /// Liquidation threshold reached
    Liquidation,
}

impl LiquidationWarningLevel {
    /// Get the margin level percentage for this warning.
    pub fn margin_level_threshold(&self) -> f64 {
        match self {
            // Warning at 125% margin level (80% of 156.25% margin)
            LiquidationWarningLevel::Warning80 => 125.0,
            // Warning at 111% margin level (90% of 123.45% margin)
            LiquidationWarningLevel::Warning90 => 111.0,
            // Warning at 105% margin level (95% of 110.5% margin)
            LiquidationWarningLevel::Warning95 => 105.0,
            // Liquidation at 100% margin level
            LiquidationWarningLevel::Liquidation => 100.0,
        }
    }

    /// Get the warning level for a given margin level.
    pub fn from_margin_level(margin_level: f64) -> Option<Self> {
        if margin_level <= 100.0 {
            Some(LiquidationWarningLevel::Liquidation)
        } else if margin_level <= 105.0 {
            Some(LiquidationWarningLevel::Warning95)
        } else if margin_level <= 111.0 {
            Some(LiquidationWarningLevel::Warning90)
        } else if margin_level <= 125.0 {
            Some(LiquidationWarningLevel::Warning80)
        } else {
            None
        }
    }
}

/// A liquidation event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Liquidation {
    /// Unique liquidation ID
    pub id: String,
    /// Position ID that was liquidated
    pub position_id: String,
    /// Portfolio ID
    pub portfolio_id: String,
    /// Symbol
    pub symbol: String,
    /// Quantity liquidated
    pub quantity: f64,
    /// Price at liquidation
    pub liquidation_price: f64,
    /// Mark price at liquidation
    pub mark_price: f64,
    /// Loss from this liquidation
    pub loss: f64,
    /// Fee charged (goes to insurance fund)
    pub liquidation_fee: f64,
    /// Whether this was a partial liquidation
    pub is_partial: bool,
    /// Remaining quantity after partial liquidation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remaining_quantity: Option<f64>,
    /// Timestamp of liquidation
    pub liquidated_at: i64,
}

impl Liquidation {
    /// Create a new liquidation record.
    pub fn new(
        position_id: String,
        portfolio_id: String,
        symbol: String,
        quantity: f64,
        liquidation_price: f64,
        mark_price: f64,
        entry_price: f64,
        side: PositionSide,
        is_partial: bool,
        remaining_quantity: Option<f64>,
    ) -> Self {
        // Calculate loss
        let notional = quantity * liquidation_price;
        let entry_notional = quantity * entry_price;
        let loss = match side {
            PositionSide::Long => entry_notional - notional,
            PositionSide::Short => notional - entry_notional,
        };

        // Liquidation fee is 0.5% of position value
        let liquidation_fee = notional * 0.005;

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            position_id,
            portfolio_id,
            symbol,
            quantity,
            liquidation_price,
            mark_price,
            loss: loss.max(0.0),
            liquidation_fee,
            is_partial,
            remaining_quantity,
            liquidated_at: chrono::Utc::now().timestamp_millis(),
        }
    }
}

/// Margin history entry for tracking margin changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarginHistory {
    /// Unique entry ID
    pub id: String,
    /// Portfolio ID
    pub portfolio_id: String,
    /// Position ID (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_id: Option<String>,
    /// Type of margin change
    pub change_type: MarginChangeType,
    /// Previous margin level
    pub previous_margin_level: f64,
    /// New margin level
    pub new_margin_level: f64,
    /// Previous margin used
    pub previous_margin_used: f64,
    /// New margin used
    pub new_margin_used: f64,
    /// Amount changed
    pub amount_changed: f64,
    /// Reason for change
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Timestamp
    pub timestamp: i64,
}

/// Type of margin change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarginChangeType {
    /// Position opened
    PositionOpened,
    /// Position closed
    PositionClosed,
    /// Position size increased
    PositionIncreased,
    /// Position size decreased
    PositionDecreased,
    /// Funding payment
    FundingPayment,
    /// Unrealized P&L change
    UnrealizedPnlChange,
    /// Liquidation
    Liquidation,
    /// Manual margin adjustment (cross margin)
    ManualAdjustment,
}

impl std::fmt::Display for MarginChangeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MarginChangeType::PositionOpened => write!(f, "position_opened"),
            MarginChangeType::PositionClosed => write!(f, "position_closed"),
            MarginChangeType::PositionIncreased => write!(f, "position_increased"),
            MarginChangeType::PositionDecreased => write!(f, "position_decreased"),
            MarginChangeType::FundingPayment => write!(f, "funding_payment"),
            MarginChangeType::UnrealizedPnlChange => write!(f, "unrealized_pnl_change"),
            MarginChangeType::Liquidation => write!(f, "liquidation"),
            MarginChangeType::ManualAdjustment => write!(f, "manual_adjustment"),
        }
    }
}

impl MarginHistory {
    /// Create a new margin history entry.
    pub fn new(
        portfolio_id: String,
        position_id: Option<String>,
        change_type: MarginChangeType,
        previous_margin_level: f64,
        new_margin_level: f64,
        previous_margin_used: f64,
        new_margin_used: f64,
        reason: Option<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            portfolio_id,
            position_id,
            change_type,
            previous_margin_level,
            new_margin_level,
            previous_margin_used,
            new_margin_used,
            amount_changed: new_margin_used - previous_margin_used,
            reason,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}

/// Insurance fund for absorbing liquidation losses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsuranceFund {
    /// Current balance
    pub balance: f64,
    /// Total contributions from liquidation fees
    pub total_contributions: f64,
    /// Total payouts to cover losses
    pub total_payouts: f64,
    /// Number of liquidations covered
    pub liquidations_covered: u64,
    /// Last updated timestamp
    pub updated_at: i64,
}

impl Default for InsuranceFund {
    fn default() -> Self {
        Self {
            balance: 0.0,
            total_contributions: 0.0,
            total_payouts: 0.0,
            liquidations_covered: 0,
            updated_at: chrono::Utc::now().timestamp_millis(),
        }
    }
}

impl InsuranceFund {
    /// Add contribution from liquidation fee.
    pub fn add_contribution(&mut self, amount: f64) {
        self.balance += amount;
        self.total_contributions += amount;
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// Pay out to cover a loss.
    /// Returns the actual amount paid out (may be less if fund is insufficient).
    pub fn cover_loss(&mut self, loss: f64) -> f64 {
        let payout = loss.min(self.balance);
        self.balance -= payout;
        self.total_payouts += payout;
        self.liquidations_covered += 1;
        self.updated_at = chrono::Utc::now().timestamp_millis();
        payout
    }

    /// Check if fund can cover a loss.
    pub fn can_cover(&self, loss: f64) -> bool {
        self.balance >= loss
    }
}

/// ADL (Auto-Deleverage) priority entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdlEntry {
    /// Position ID
    pub position_id: String,
    /// Portfolio ID
    pub portfolio_id: String,
    /// Symbol
    pub symbol: String,
    /// Position side
    pub side: PositionSide,
    /// Position size
    pub position_size: f64,
    /// Unrealized profit (for ranking)
    pub unrealized_profit: f64,
    /// Leverage used
    pub leverage: f64,
    /// ADL score (higher = more likely to be deleveraged)
    pub adl_score: f64,
}

impl AdlEntry {
    /// Calculate ADL score based on profit and leverage.
    /// Higher score = higher priority for deleveraging.
    pub fn calculate_score(unrealized_profit_pct: f64, leverage: f64) -> f64 {
        // ADL priority = profit percentile * leverage percentile
        // Simplified: profit_ratio * leverage_ratio
        // Those with highest profits and highest leverage get deleveraged first
        let profit_factor = if unrealized_profit_pct > 0.0 {
            unrealized_profit_pct
        } else {
            0.0
        };
        profit_factor * leverage
    }

    /// Create a new ADL entry for a position.
    pub fn from_position(position: &Position) -> Self {
        let unrealized_profit = if position.unrealized_pnl > 0.0 {
            position.unrealized_pnl
        } else {
            0.0
        };
        let adl_score = Self::calculate_score(position.unrealized_pnl_pct, position.leverage);

        Self {
            position_id: position.id.clone(),
            portfolio_id: position.portfolio_id.clone(),
            symbol: position.symbol.clone(),
            side: position.side,
            position_size: position.notional_value(),
            unrealized_profit,
            leverage: position.leverage,
            adl_score,
        }
    }
}

// =============================================================================
// Options Trading Types
// =============================================================================

/// Option type (Call or Put).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptionType {
    /// Right to buy at strike price
    Call,
    /// Right to sell at strike price
    Put,
}

impl std::fmt::Display for OptionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OptionType::Call => write!(f, "call"),
            OptionType::Put => write!(f, "put"),
        }
    }
}

/// Option exercise style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptionStyle {
    /// Can be exercised any time before expiration
    American,
    /// Can only be exercised at expiration
    European,
}

impl Default for OptionStyle {
    fn default() -> Self {
        OptionStyle::American
    }
}

impl std::fmt::Display for OptionStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OptionStyle::American => write!(f, "american"),
            OptionStyle::European => write!(f, "european"),
        }
    }
}

/// Greeks for an option position.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Greeks {
    /// Price sensitivity to underlying price change (dV/dS)
    pub delta: f64,
    /// Rate of change of delta (d²V/dS²)
    pub gamma: f64,
    /// Time decay per day (dV/dt)
    pub theta: f64,
    /// Sensitivity to volatility change (dV/dσ)
    pub vega: f64,
    /// Sensitivity to interest rate change (dV/dr)
    pub rho: f64,
}

impl Greeks {
    /// Create new Greeks.
    pub fn new(delta: f64, gamma: f64, theta: f64, vega: f64, rho: f64) -> Self {
        Self { delta, gamma, theta, vega, rho }
    }
}

/// An option contract from an options chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionContract {
    /// Contract symbol (e.g., "AAPL230120C00150000")
    pub contract_symbol: String,
    /// Underlying symbol (e.g., "AAPL")
    pub underlying_symbol: String,
    /// Option type (call or put)
    pub option_type: OptionType,
    /// Strike price
    pub strike: f64,
    /// Expiration date (Unix timestamp ms)
    pub expiration: i64,
    /// Exercise style
    pub style: OptionStyle,
    /// Current bid price
    pub bid: f64,
    /// Current ask price
    pub ask: f64,
    /// Last trade price
    pub last: f64,
    /// Trading volume
    pub volume: u64,
    /// Open interest
    pub open_interest: u64,
    /// Implied volatility (as decimal, e.g., 0.25 = 25%)
    pub implied_volatility: f64,
    /// Current Greeks
    pub greeks: Greeks,
    /// Contract multiplier (usually 100 for equity options)
    pub multiplier: u32,
}

impl OptionContract {
    /// Create a new option contract.
    pub fn new(
        underlying_symbol: String,
        option_type: OptionType,
        strike: f64,
        expiration: i64,
        style: OptionStyle,
    ) -> Self {
        // Generate contract symbol (e.g., "AAPL230120C00150000")
        let datetime = chrono::DateTime::from_timestamp_millis(expiration)
            .unwrap_or_else(|| chrono::Utc::now());
        let date_str = datetime.format("%y%m%d").to_string();
        let type_char = match option_type {
            OptionType::Call => "C",
            OptionType::Put => "P",
        };
        let strike_str = format!("{:08}", (strike * 1000.0) as u64);
        let contract_symbol = format!("{}{}{}{}", underlying_symbol, date_str, type_char, strike_str);

        Self {
            contract_symbol,
            underlying_symbol,
            option_type,
            strike,
            expiration,
            style,
            bid: 0.0,
            ask: 0.0,
            last: 0.0,
            volume: 0,
            open_interest: 0,
            implied_volatility: 0.0,
            greeks: Greeks::default(),
            multiplier: 100,
        }
    }

    /// Get the mid price (average of bid and ask).
    pub fn mid_price(&self) -> f64 {
        (self.bid + self.ask) / 2.0
    }

    /// Check if the option is in the money given the underlying price.
    pub fn is_itm(&self, underlying_price: f64) -> bool {
        match self.option_type {
            OptionType::Call => underlying_price > self.strike,
            OptionType::Put => underlying_price < self.strike,
        }
    }

    /// Check if the option is at the money (within 1% of strike).
    pub fn is_atm(&self, underlying_price: f64) -> bool {
        let diff = (underlying_price - self.strike).abs() / self.strike;
        diff < 0.01
    }

    /// Check if the option is out of the money.
    pub fn is_otm(&self, underlying_price: f64) -> bool {
        !self.is_itm(underlying_price) && !self.is_atm(underlying_price)
    }

    /// Get intrinsic value given underlying price.
    pub fn intrinsic_value(&self, underlying_price: f64) -> f64 {
        match self.option_type {
            OptionType::Call => (underlying_price - self.strike).max(0.0),
            OptionType::Put => (self.strike - underlying_price).max(0.0),
        }
    }

    /// Get extrinsic (time) value given underlying price.
    pub fn extrinsic_value(&self, underlying_price: f64) -> f64 {
        self.mid_price() - self.intrinsic_value(underlying_price)
    }

    /// Get days until expiration.
    pub fn days_to_expiration(&self) -> f64 {
        let now = chrono::Utc::now().timestamp_millis();
        let diff_ms = (self.expiration - now).max(0) as f64;
        diff_ms / (24.0 * 60.0 * 60.0 * 1000.0)
    }

    /// Check if the option has expired.
    pub fn is_expired(&self) -> bool {
        chrono::Utc::now().timestamp_millis() >= self.expiration
    }
}

/// An options chain for a specific underlying and expiration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionsChain {
    /// Underlying symbol
    pub underlying_symbol: String,
    /// Current underlying price
    pub underlying_price: f64,
    /// Expiration date
    pub expiration: i64,
    /// Call contracts sorted by strike
    pub calls: Vec<OptionContract>,
    /// Put contracts sorted by strike
    pub puts: Vec<OptionContract>,
    /// Timestamp when chain was fetched
    pub timestamp: i64,
}

impl OptionsChain {
    /// Get the ATM strike (closest to underlying price).
    pub fn atm_strike(&self) -> Option<f64> {
        if self.calls.is_empty() {
            return None;
        }
        self.calls
            .iter()
            .min_by(|a, b| {
                let diff_a = (a.strike - self.underlying_price).abs();
                let diff_b = (b.strike - self.underlying_price).abs();
                diff_a.partial_cmp(&diff_b).unwrap()
            })
            .map(|c| c.strike)
    }

    /// Get call contract at a specific strike.
    pub fn get_call(&self, strike: f64) -> Option<&OptionContract> {
        self.calls.iter().find(|c| (c.strike - strike).abs() < 0.01)
    }

    /// Get put contract at a specific strike.
    pub fn get_put(&self, strike: f64) -> Option<&OptionContract> {
        self.puts.iter().find(|p| (p.strike - strike).abs() < 0.01)
    }
}

/// An option position (long or short options).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionPosition {
    /// Unique position ID
    pub id: String,
    /// Portfolio this position belongs to
    pub portfolio_id: String,
    /// Contract symbol
    pub contract_symbol: String,
    /// Underlying symbol
    pub underlying_symbol: String,
    /// Option type
    pub option_type: OptionType,
    /// Strike price
    pub strike: f64,
    /// Expiration date (ms)
    pub expiration: i64,
    /// Exercise style
    pub style: OptionStyle,
    /// Number of contracts (positive = long, negative = short)
    pub contracts: i32,
    /// Contract multiplier
    pub multiplier: u32,
    /// Average entry premium per contract
    pub entry_premium: f64,
    /// Current premium per contract
    pub current_premium: f64,
    /// Current underlying price
    pub underlying_price: f64,
    /// Unrealized P&L
    pub unrealized_pnl: f64,
    /// Realized P&L (from closed portions)
    pub realized_pnl: f64,
    /// Current Greeks
    pub greeks: Greeks,
    /// Implied volatility at entry
    pub entry_iv: f64,
    /// Current implied volatility
    pub current_iv: f64,
    /// When position was opened
    pub created_at: i64,
    /// When position was last updated
    pub updated_at: i64,
}

impl OptionPosition {
    /// Create a new option position.
    pub fn new(
        portfolio_id: String,
        contract: &OptionContract,
        contracts: i32,
        entry_premium: f64,
    ) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            portfolio_id,
            contract_symbol: contract.contract_symbol.clone(),
            underlying_symbol: contract.underlying_symbol.clone(),
            option_type: contract.option_type,
            strike: contract.strike,
            expiration: contract.expiration,
            style: contract.style,
            contracts,
            multiplier: contract.multiplier,
            entry_premium,
            current_premium: entry_premium,
            underlying_price: 0.0,
            unrealized_pnl: 0.0,
            realized_pnl: 0.0,
            greeks: contract.greeks,
            entry_iv: contract.implied_volatility,
            current_iv: contract.implied_volatility,
            created_at: now,
            updated_at: now,
        }
    }

    /// Check if this is a long position.
    pub fn is_long(&self) -> bool {
        self.contracts > 0
    }

    /// Check if this is a short position.
    pub fn is_short(&self) -> bool {
        self.contracts < 0
    }

    /// Get the notional value of the position.
    pub fn notional_value(&self) -> f64 {
        self.contracts.abs() as f64 * self.current_premium * self.multiplier as f64
    }

    /// Get the total cost/credit at entry.
    pub fn entry_cost(&self) -> f64 {
        self.contracts as f64 * self.entry_premium * self.multiplier as f64
    }

    /// Update the position with new market data.
    pub fn update(&mut self, current_premium: f64, underlying_price: f64, greeks: Greeks, iv: f64) {
        self.current_premium = current_premium;
        self.underlying_price = underlying_price;
        self.greeks = greeks;
        self.current_iv = iv;

        // Calculate unrealized P&L
        let current_value = self.contracts as f64 * current_premium * self.multiplier as f64;
        let entry_value = self.contracts as f64 * self.entry_premium * self.multiplier as f64;
        self.unrealized_pnl = current_value - entry_value;

        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// Check if the option is in the money.
    pub fn is_itm(&self) -> bool {
        match self.option_type {
            OptionType::Call => self.underlying_price > self.strike,
            OptionType::Put => self.underlying_price < self.strike,
        }
    }

    /// Check if the option has expired.
    pub fn is_expired(&self) -> bool {
        chrono::Utc::now().timestamp_millis() >= self.expiration
    }

    /// Get days until expiration.
    pub fn days_to_expiration(&self) -> f64 {
        let now = chrono::Utc::now().timestamp_millis();
        let diff_ms = (self.expiration - now).max(0) as f64;
        diff_ms / (24.0 * 60.0 * 60.0 * 1000.0)
    }

    /// Calculate exercise value if exercised now.
    pub fn exercise_value(&self) -> f64 {
        let intrinsic = match self.option_type {
            OptionType::Call => (self.underlying_price - self.strike).max(0.0),
            OptionType::Put => (self.strike - self.underlying_price).max(0.0),
        };
        self.contracts.abs() as f64 * intrinsic * self.multiplier as f64
    }

    /// Get maximum loss for this position.
    pub fn max_loss(&self) -> f64 {
        if self.is_long() {
            // Long options: max loss is premium paid
            self.entry_cost().abs()
        } else {
            // Short options: max loss is theoretically unlimited for calls
            match self.option_type {
                OptionType::Call => f64::INFINITY,
                OptionType::Put => {
                    // Max loss on short put is strike price minus premium received
                    (self.strike * self.contracts.abs() as f64 * self.multiplier as f64)
                        - self.entry_cost().abs()
                }
            }
        }
    }

    /// Get maximum profit for this position.
    pub fn max_profit(&self) -> f64 {
        if self.is_long() {
            // Long calls: unlimited, Long puts: strike - premium
            match self.option_type {
                OptionType::Call => f64::INFINITY,
                OptionType::Put => {
                    (self.strike * self.contracts as f64 * self.multiplier as f64)
                        - self.entry_cost()
                }
            }
        } else {
            // Short options: max profit is premium received
            self.entry_cost().abs()
        }
    }

    /// Get breakeven price for the position.
    pub fn breakeven_price(&self) -> f64 {
        let premium_per_share = self.entry_premium;
        match self.option_type {
            OptionType::Call => {
                if self.is_long() {
                    self.strike + premium_per_share
                } else {
                    self.strike + premium_per_share
                }
            }
            OptionType::Put => {
                if self.is_long() {
                    self.strike - premium_per_share
                } else {
                    self.strike - premium_per_share
                }
            }
        }
    }
}

/// Multi-leg option strategy type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptionStrategyType {
    /// Single call or put
    Single,
    /// Long stock + short call
    CoveredCall,
    /// Long put for downside protection
    ProtectivePut,
    /// Buy call, sell higher strike call
    BullCallSpread,
    /// Buy put, sell lower strike put
    BearPutSpread,
    /// Buy call, sell higher strike call (credit)
    BearCallSpread,
    /// Buy put, sell lower strike put (credit)
    BullPutSpread,
    /// Long call + long put at same strike
    Straddle,
    /// Long call + long put at different strikes
    Strangle,
    /// Short call spread + short put spread
    IronCondor,
    /// Short straddle + long wings
    IronButterfly,
    /// Options at same strike, different expirations
    CalendarSpread,
    /// Custom multi-leg
    Custom,
}

impl std::fmt::Display for OptionStrategyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OptionStrategyType::Single => write!(f, "single"),
            OptionStrategyType::CoveredCall => write!(f, "covered_call"),
            OptionStrategyType::ProtectivePut => write!(f, "protective_put"),
            OptionStrategyType::BullCallSpread => write!(f, "bull_call_spread"),
            OptionStrategyType::BearPutSpread => write!(f, "bear_put_spread"),
            OptionStrategyType::BearCallSpread => write!(f, "bear_call_spread"),
            OptionStrategyType::BullPutSpread => write!(f, "bull_put_spread"),
            OptionStrategyType::Straddle => write!(f, "straddle"),
            OptionStrategyType::Strangle => write!(f, "strangle"),
            OptionStrategyType::IronCondor => write!(f, "iron_condor"),
            OptionStrategyType::IronButterfly => write!(f, "iron_butterfly"),
            OptionStrategyType::CalendarSpread => write!(f, "calendar_spread"),
            OptionStrategyType::Custom => write!(f, "custom"),
        }
    }
}

/// A multi-leg option strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionStrategy {
    /// Unique strategy ID
    pub id: String,
    /// Portfolio this strategy belongs to
    pub portfolio_id: String,
    /// Strategy type
    pub strategy_type: OptionStrategyType,
    /// Underlying symbol
    pub underlying_symbol: String,
    /// Legs of the strategy
    pub legs: Vec<OptionPosition>,
    /// Net premium paid (positive) or received (negative)
    pub net_premium: f64,
    /// Combined Greeks for the strategy
    pub net_greeks: Greeks,
    /// Maximum potential loss
    pub max_loss: f64,
    /// Maximum potential profit
    pub max_profit: f64,
    /// Breakeven prices (may have multiple)
    pub breakeven_prices: Vec<f64>,
    /// When strategy was opened
    pub created_at: i64,
    /// When strategy was last updated
    pub updated_at: i64,
}

impl OptionStrategy {
    /// Create a new option strategy.
    pub fn new(
        portfolio_id: String,
        strategy_type: OptionStrategyType,
        underlying_symbol: String,
        legs: Vec<OptionPosition>,
    ) -> Self {
        let now = chrono::Utc::now().timestamp_millis();

        // Calculate net premium
        let net_premium: f64 = legs.iter().map(|l| l.entry_cost()).sum();

        // Calculate net Greeks
        let net_greeks = Greeks {
            delta: legs.iter().map(|l| l.greeks.delta * l.contracts as f64).sum(),
            gamma: legs.iter().map(|l| l.greeks.gamma * l.contracts as f64).sum(),
            theta: legs.iter().map(|l| l.greeks.theta * l.contracts as f64).sum(),
            vega: legs.iter().map(|l| l.greeks.vega * l.contracts as f64).sum(),
            rho: legs.iter().map(|l| l.greeks.rho * l.contracts as f64).sum(),
        };

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            portfolio_id,
            strategy_type,
            underlying_symbol,
            legs,
            net_premium,
            net_greeks,
            max_loss: 0.0,       // Calculated separately based on strategy type
            max_profit: 0.0,     // Calculated separately based on strategy type
            breakeven_prices: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Get total unrealized P&L for the strategy.
    pub fn unrealized_pnl(&self) -> f64 {
        self.legs.iter().map(|l| l.unrealized_pnl).sum()
    }

    /// Get total realized P&L for the strategy.
    pub fn realized_pnl(&self) -> f64 {
        self.legs.iter().map(|l| l.realized_pnl).sum()
    }

    /// Update net Greeks from leg Greeks.
    pub fn update_greeks(&mut self) {
        self.net_greeks = Greeks {
            delta: self.legs.iter().map(|l| l.greeks.delta * l.contracts as f64).sum(),
            gamma: self.legs.iter().map(|l| l.greeks.gamma * l.contracts as f64).sum(),
            theta: self.legs.iter().map(|l| l.greeks.theta * l.contracts as f64).sum(),
            vega: self.legs.iter().map(|l| l.greeks.vega * l.contracts as f64).sum(),
            rho: self.legs.iter().map(|l| l.greeks.rho * l.contracts as f64).sum(),
        };
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// Check if any leg has expired.
    pub fn has_expired_leg(&self) -> bool {
        self.legs.iter().any(|l| l.is_expired())
    }

    /// Get the earliest expiration among all legs.
    pub fn earliest_expiration(&self) -> Option<i64> {
        self.legs.iter().map(|l| l.expiration).min()
    }
}

// =============================================================================
// Position Types
// =============================================================================

/// A cost basis entry for tracking purchase lots.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CostBasisEntry {
    /// Quantity in this lot
    pub quantity: f64,
    /// Price per unit when acquired
    pub price: f64,
    /// When this lot was acquired (ms)
    pub acquired_at: i64,
}

/// An open trading position.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    /// Unique position ID
    pub id: String,
    /// Portfolio this position belongs to
    pub portfolio_id: String,
    /// Symbol (e.g., "BTC", "AAPL")
    pub symbol: String,
    /// Asset class
    pub asset_class: AssetClass,
    /// Long or short
    pub side: PositionSide,
    /// Current quantity held
    pub quantity: f64,
    /// Average entry price
    pub entry_price: f64,
    /// Current market price
    pub current_price: f64,
    /// Unrealized P&L
    pub unrealized_pnl: f64,
    /// Unrealized P&L as percentage
    pub unrealized_pnl_pct: f64,
    /// Realized P&L (from partial closes)
    pub realized_pnl: f64,
    /// Margin used for this position
    pub margin_used: f64,
    /// Leverage applied
    pub leverage: f64,
    /// Margin mode
    pub margin_mode: MarginMode,
    /// Liquidation price (if leveraged)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liquidation_price: Option<f64>,
    /// Stop loss price
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_loss: Option<f64>,
    /// Take profit price
    #[serde(skip_serializing_if = "Option::is_none")]
    pub take_profit: Option<f64>,
    /// Cost basis entries for this position
    #[serde(default)]
    pub cost_basis: Vec<CostBasisEntry>,
    /// Cumulative funding payments (for perps)
    #[serde(default)]
    pub funding_payments: f64,
    /// When position was opened (ms)
    pub created_at: i64,
    /// When position was last updated (ms)
    pub updated_at: i64,
}

impl Position {
    /// Create a new position.
    pub fn new(
        portfolio_id: String,
        symbol: String,
        asset_class: AssetClass,
        side: PositionSide,
        quantity: f64,
        entry_price: f64,
        leverage: f64,
    ) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        let notional = quantity * entry_price;
        let margin_used = notional / leverage;

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            portfolio_id,
            symbol,
            asset_class,
            side,
            quantity,
            entry_price,
            current_price: entry_price,
            unrealized_pnl: 0.0,
            unrealized_pnl_pct: 0.0,
            realized_pnl: 0.0,
            margin_used,
            leverage,
            margin_mode: MarginMode::default(),
            liquidation_price: None,
            stop_loss: None,
            take_profit: None,
            cost_basis: vec![CostBasisEntry {
                quantity,
                price: entry_price,
                acquired_at: now,
            }],
            funding_payments: 0.0,
            created_at: now,
            updated_at: now,
        }
    }

    /// Update the position with a new market price.
    pub fn update_price(&mut self, price: f64) {
        self.current_price = price;

        // Calculate unrealized P&L
        let notional_entry = self.quantity * self.entry_price;
        let notional_current = self.quantity * self.current_price;

        self.unrealized_pnl = match self.side {
            PositionSide::Long => notional_current - notional_entry,
            PositionSide::Short => notional_entry - notional_current,
        };

        // Include funding payments in P&L
        self.unrealized_pnl += self.funding_payments;

        // Calculate percentage
        if notional_entry > 0.0 {
            self.unrealized_pnl_pct = (self.unrealized_pnl / notional_entry) * 100.0;
        }

        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// Calculate liquidation price for leveraged positions.
    pub fn calculate_liquidation_price(&mut self) {
        if self.leverage <= 1.0 {
            self.liquidation_price = None;
            return;
        }

        let maintenance_margin = self.asset_class.maintenance_margin();
        let initial_margin = 1.0 / self.leverage;

        self.liquidation_price = Some(match self.side {
            PositionSide::Long => {
                self.entry_price * (1.0 - initial_margin + maintenance_margin)
            }
            PositionSide::Short => {
                self.entry_price * (1.0 + initial_margin - maintenance_margin)
            }
        });
    }

    /// Check if position should be liquidated at current price.
    pub fn should_liquidate(&self) -> bool {
        if let Some(liq_price) = self.liquidation_price {
            match self.side {
                PositionSide::Long => self.current_price <= liq_price,
                PositionSide::Short => self.current_price >= liq_price,
            }
        } else {
            false
        }
    }

    /// Check if stop loss should trigger.
    pub fn should_stop_loss(&self) -> bool {
        if let Some(sl) = self.stop_loss {
            match self.side {
                PositionSide::Long => self.current_price <= sl,
                PositionSide::Short => self.current_price >= sl,
            }
        } else {
            false
        }
    }

    /// Check if take profit should trigger.
    pub fn should_take_profit(&self) -> bool {
        if let Some(tp) = self.take_profit {
            match self.side {
                PositionSide::Long => self.current_price >= tp,
                PositionSide::Short => self.current_price <= tp,
            }
        } else {
            false
        }
    }

    /// Get the notional value of this position.
    pub fn notional_value(&self) -> f64 {
        self.quantity * self.current_price
    }

    /// Apply a funding payment to this position.
    pub fn apply_funding(&mut self, payment: f64) {
        self.funding_payments += payment;
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// Get the leverage tier for this position.
    pub fn leverage_tier(&self) -> LeverageTier {
        LeverageTier::for_position_size(self.notional_value())
    }

    /// Calculate the effective margin level for this position.
    pub fn margin_level(&self) -> f64 {
        if self.margin_used > 0.0 {
            let equity = self.margin_used + self.unrealized_pnl;
            (equity / self.margin_used) * 100.0
        } else {
            f64::INFINITY
        }
    }

    /// Check if this position is in a liquidation warning zone.
    pub fn warning_level(&self) -> Option<LiquidationWarningLevel> {
        LiquidationWarningLevel::from_margin_level(self.margin_level())
    }

    /// Calculate liquidation price using mark price (for perps).
    pub fn calculate_liquidation_price_with_tier(&mut self) {
        if self.leverage <= 1.0 {
            self.liquidation_price = None;
            return;
        }

        let tier = self.leverage_tier();
        let initial_margin = 1.0 / self.leverage;

        self.liquidation_price = Some(match self.side {
            PositionSide::Long => {
                self.entry_price * (1.0 - initial_margin + tier.maintenance_margin_rate)
            }
            PositionSide::Short => {
                self.entry_price * (1.0 + initial_margin - tier.maintenance_margin_rate)
            }
        });
    }

    /// Check if leverage is valid for the current position size.
    pub fn validate_leverage(&self) -> Result<(), String> {
        if self.asset_class != AssetClass::Perp {
            return Ok(());
        }

        let tier = self.leverage_tier();
        if self.leverage > tier.max_leverage {
            return Err(format!(
                "Leverage {}x exceeds maximum {}x for position size ${}",
                self.leverage, tier.max_leverage, self.notional_value()
            ));
        }
        Ok(())
    }

    /// Get the required maintenance margin for this position.
    pub fn maintenance_margin_required(&self) -> f64 {
        if self.asset_class == AssetClass::Perp {
            let tier = self.leverage_tier();
            self.notional_value() * tier.maintenance_margin_rate
        } else {
            self.notional_value() * self.asset_class.maintenance_margin()
        }
    }
}

// =============================================================================
// Trade Types
// =============================================================================

/// A completed trade (execution record).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trade {
    /// Unique trade ID
    pub id: String,
    /// Order that generated this trade
    pub order_id: String,
    /// Portfolio this trade belongs to
    pub portfolio_id: String,
    /// Position this trade affected (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_id: Option<String>,
    /// Symbol traded
    pub symbol: String,
    /// Asset class
    pub asset_class: AssetClass,
    /// Buy or sell
    pub side: OrderSide,
    /// Quantity traded
    pub quantity: f64,
    /// Execution price
    pub price: f64,
    /// Fee charged
    pub fee: f64,
    /// Slippage from expected price
    pub slippage: f64,
    /// Realized P&L from this trade (if closing a position)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub realized_pnl: Option<f64>,
    /// When trade was executed (ms)
    pub executed_at: i64,
}

impl Trade {
    /// Create a new trade record.
    pub fn new(
        order_id: String,
        portfolio_id: String,
        symbol: String,
        asset_class: AssetClass,
        side: OrderSide,
        quantity: f64,
        price: f64,
        fee: f64,
        slippage: f64,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            order_id,
            portfolio_id,
            position_id: None,
            symbol,
            asset_class,
            side,
            quantity,
            price,
            fee,
            slippage,
            realized_pnl: None,
            executed_at: chrono::Utc::now().timestamp_millis(),
        }
    }

    /// Get the total cost of this trade (quantity * price + fee).
    pub fn total_cost(&self) -> f64 {
        (self.quantity * self.price) + self.fee
    }
}

// =============================================================================
// Request/Response Types for API
// =============================================================================

/// Request to create a new portfolio.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePortfolioRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_currency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_basis_method: Option<CostBasisMethod>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_settings: Option<RiskSettings>,
}

/// Request to place an order.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceOrderRequest {
    pub portfolio_id: String,
    pub symbol: String,
    pub asset_class: AssetClass,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trail_amount: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trail_percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_in_force: Option<TimeInForce>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leverage: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_loss: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub take_profit: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_order_id: Option<String>,
    /// Bypass drawdown protection for this order (one-time override)
    #[serde(default)]
    pub bypass_drawdown: bool,
}

/// Request to modify a position.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModifyPositionRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_loss: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub take_profit: Option<f64>,
}

/// Summary of portfolio performance.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortfolioSummary {
    pub portfolio_id: String,
    pub total_value: f64,
    pub cash_balance: f64,
    pub unrealized_pnl: f64,
    pub realized_pnl: f64,
    pub total_return_pct: f64,
    pub margin_used: f64,
    pub margin_available: f64,
    pub margin_level: f64,
    pub open_positions: u32,
    pub open_orders: u32,
}

/// Leaderboard entry for portfolio rankings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaderboardEntry {
    pub portfolio_id: String,
    pub name: String,
    pub user_id: String,
    pub total_value: f64,
    pub starting_balance: f64,
    pub realized_pnl: f64,
    pub unrealized_pnl: f64,
    pub total_return_pct: f64,
    pub total_trades: u64,
    pub winning_trades: u64,
    pub win_rate: f64,
    pub open_positions: u32,
}

/// Order query parameters.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub portfolio_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<OrderStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub side: Option<OrderSide>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,
}

/// Position query parameters.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub portfolio_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub side: Option<PositionSide>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_class: Option<AssetClass>,
}

// =============================================================================
// Auto-Trading Strategy Types
// =============================================================================

/// Strategy status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrategyStatus {
    /// Strategy is active and will execute trades
    Active,
    /// Strategy is paused (will not execute)
    Paused,
    /// Strategy is disabled (manual reactivation required)
    Disabled,
    /// Strategy was deleted
    Deleted,
}

impl Default for StrategyStatus {
    fn default() -> Self {
        StrategyStatus::Paused
    }
}

impl std::fmt::Display for StrategyStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StrategyStatus::Active => write!(f, "active"),
            StrategyStatus::Paused => write!(f, "paused"),
            StrategyStatus::Disabled => write!(f, "disabled"),
            StrategyStatus::Deleted => write!(f, "deleted"),
        }
    }
}

/// Indicator types supported in strategy rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndicatorType {
    Rsi,
    Macd,
    Ema,
    Sma,
    Bollinger,
    Atr,
    Adx,
    Stochastic,
    Obv,
    Vwap,
    Cci,
    Mfi,
    Price,
}

impl std::fmt::Display for IndicatorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IndicatorType::Rsi => write!(f, "rsi"),
            IndicatorType::Macd => write!(f, "macd"),
            IndicatorType::Ema => write!(f, "ema"),
            IndicatorType::Sma => write!(f, "sma"),
            IndicatorType::Bollinger => write!(f, "bollinger"),
            IndicatorType::Atr => write!(f, "atr"),
            IndicatorType::Adx => write!(f, "adx"),
            IndicatorType::Stochastic => write!(f, "stochastic"),
            IndicatorType::Obv => write!(f, "obv"),
            IndicatorType::Vwap => write!(f, "vwap"),
            IndicatorType::Cci => write!(f, "cci"),
            IndicatorType::Mfi => write!(f, "mfi"),
            IndicatorType::Price => write!(f, "price"),
        }
    }
}

/// Comparison operators for rule conditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonOperator {
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    Equal,
    NotEqual,
    CrossesAbove,
    CrossesBelow,
}

impl std::fmt::Display for ComparisonOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComparisonOperator::LessThan => write!(f, "lt"),
            ComparisonOperator::LessThanOrEqual => write!(f, "lte"),
            ComparisonOperator::GreaterThan => write!(f, "gt"),
            ComparisonOperator::GreaterThanOrEqual => write!(f, "gte"),
            ComparisonOperator::Equal => write!(f, "eq"),
            ComparisonOperator::NotEqual => write!(f, "ne"),
            ComparisonOperator::CrossesAbove => write!(f, "crosses_above"),
            ComparisonOperator::CrossesBelow => write!(f, "crosses_below"),
        }
    }
}

/// Logical operators for combining conditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogicalOperator {
    And,
    Or,
}

impl Default for LogicalOperator {
    fn default() -> Self {
        LogicalOperator::And
    }
}

/// Order action type for strategy rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleActionType {
    /// Place a market buy order
    MarketBuy,
    /// Place a market sell order
    MarketSell,
    /// Place a limit buy order
    LimitBuy,
    /// Place a limit sell order
    LimitSell,
    /// Close existing position
    ClosePosition,
    /// Close partial position
    ClosePartial,
}

impl std::fmt::Display for RuleActionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuleActionType::MarketBuy => write!(f, "market_buy"),
            RuleActionType::MarketSell => write!(f, "market_sell"),
            RuleActionType::LimitBuy => write!(f, "limit_buy"),
            RuleActionType::LimitSell => write!(f, "limit_sell"),
            RuleActionType::ClosePosition => write!(f, "close_position"),
            RuleActionType::ClosePartial => write!(f, "close_partial"),
        }
    }
}

/// Position size type for strategy actions.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PositionSizeType {
    /// Fixed dollar amount
    FixedAmount,
    /// Percentage of portfolio
    PortfolioPercent,
    /// Risk-based (% of portfolio at risk with stop loss)
    RiskPercent,
    /// Fixed number of units/shares
    FixedUnits,
}

impl Default for PositionSizeType {
    fn default() -> Self {
        PositionSizeType::PortfolioPercent
    }
}

/// A single condition in a strategy rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleCondition {
    /// Unique condition ID
    pub id: String,
    /// Indicator to check
    pub indicator: IndicatorType,
    /// Indicator period (e.g., 14 for RSI-14)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<u32>,
    /// Comparison operator
    pub operator: ComparisonOperator,
    /// Value to compare against
    pub value: f64,
    /// Optional second indicator for cross comparisons
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compare_indicator: Option<IndicatorType>,
    /// Optional period for second indicator
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compare_period: Option<u32>,
}

impl RuleCondition {
    /// Create a new simple condition (indicator op value).
    pub fn new(indicator: IndicatorType, operator: ComparisonOperator, value: f64) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            indicator,
            period: None,
            operator,
            value,
            compare_indicator: None,
            compare_period: None,
        }
    }

    /// Create a condition with period.
    pub fn with_period(mut self, period: u32) -> Self {
        self.period = Some(period);
        self
    }
}

/// Action to take when rule conditions are met.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleAction {
    /// Action type
    pub action_type: RuleActionType,
    /// Position size type
    pub size_type: PositionSizeType,
    /// Size value (meaning depends on size_type)
    pub size_value: f64,
    /// Optional stop loss (percentage from entry)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_loss_pct: Option<f64>,
    /// Optional take profit (percentage from entry)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub take_profit_pct: Option<f64>,
    /// Leverage to use (1.0 for spot)
    #[serde(default = "default_action_leverage")]
    pub leverage: f64,
}

fn default_action_leverage() -> f64 {
    1.0
}

impl RuleAction {
    /// Create a market buy action.
    pub fn market_buy(size_type: PositionSizeType, size_value: f64) -> Self {
        Self {
            action_type: RuleActionType::MarketBuy,
            size_type,
            size_value,
            stop_loss_pct: None,
            take_profit_pct: None,
            leverage: 1.0,
        }
    }

    /// Create a market sell action.
    pub fn market_sell(size_type: PositionSizeType, size_value: f64) -> Self {
        Self {
            action_type: RuleActionType::MarketSell,
            size_type,
            size_value,
            stop_loss_pct: None,
            take_profit_pct: None,
            leverage: 1.0,
        }
    }

    /// Set stop loss percentage.
    pub fn with_stop_loss(mut self, pct: f64) -> Self {
        self.stop_loss_pct = Some(pct);
        self
    }

    /// Set take profit percentage.
    pub fn with_take_profit(mut self, pct: f64) -> Self {
        self.take_profit_pct = Some(pct);
        self
    }

    /// Set leverage.
    pub fn with_leverage(mut self, leverage: f64) -> Self {
        self.leverage = leverage;
        self
    }
}

/// A trading rule that defines when to take action.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingRule {
    /// Unique rule ID
    pub id: String,
    /// Rule name
    pub name: String,
    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Conditions that must be met
    pub conditions: Vec<RuleCondition>,
    /// Logical operator to combine conditions
    #[serde(default)]
    pub condition_operator: LogicalOperator,
    /// Action to take when conditions are met
    pub action: RuleAction,
    /// Whether this rule is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Priority (lower = higher priority)
    #[serde(default)]
    pub priority: u32,
}

fn default_true() -> bool {
    true
}

impl TradingRule {
    /// Create a new trading rule.
    pub fn new(name: String, conditions: Vec<RuleCondition>, action: RuleAction) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            description: None,
            conditions,
            condition_operator: LogicalOperator::And,
            action,
            enabled: true,
            priority: 0,
        }
    }
}

/// An automated trading strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingStrategy {
    /// Unique strategy ID
    pub id: String,
    /// Portfolio this strategy is attached to
    pub portfolio_id: String,
    /// Strategy name
    pub name: String,
    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Symbols this strategy applies to
    pub symbols: Vec<String>,
    /// Asset class restriction
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_class: Option<AssetClass>,
    /// Trading rules
    pub rules: Vec<TradingRule>,
    /// Strategy status
    pub status: StrategyStatus,
    /// Cooldown period between trades (seconds)
    #[serde(default = "default_cooldown")]
    pub cooldown_seconds: u64,
    /// Maximum positions this strategy can open
    #[serde(default = "default_max_positions")]
    pub max_positions: u32,
    /// Maximum position size (portfolio percentage)
    #[serde(default = "default_strategy_max_position_size")]
    pub max_position_size_pct: f64,
    /// Last time a trade was executed (ms)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_trade_at: Option<i64>,
    /// Total trades executed by this strategy
    pub total_trades: u64,
    /// Profitable trades
    pub winning_trades: u64,
    /// Losing trades
    pub losing_trades: u64,
    /// Total realized P&L from this strategy
    pub realized_pnl: f64,
    /// When strategy was created
    pub created_at: i64,
    /// When strategy was last updated
    pub updated_at: i64,
}

fn default_cooldown() -> u64 {
    3600 // 1 hour
}

fn default_max_positions() -> u32 {
    3
}

fn default_strategy_max_position_size() -> f64 {
    0.10 // 10% of portfolio
}

impl TradingStrategy {
    /// Create a new trading strategy.
    pub fn new(portfolio_id: String, name: String, symbols: Vec<String>) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            portfolio_id,
            name,
            description: None,
            symbols,
            asset_class: None,
            rules: Vec::new(),
            status: StrategyStatus::Paused,
            cooldown_seconds: default_cooldown(),
            max_positions: default_max_positions(),
            max_position_size_pct: default_max_position_size(),
            last_trade_at: None,
            total_trades: 0,
            winning_trades: 0,
            losing_trades: 0,
            realized_pnl: 0.0,
            created_at: now,
            updated_at: now,
        }
    }

    /// Add a rule to the strategy.
    pub fn add_rule(&mut self, rule: TradingRule) {
        self.rules.push(rule);
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// Activate the strategy.
    pub fn activate(&mut self) {
        self.status = StrategyStatus::Active;
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// Pause the strategy.
    pub fn pause(&mut self) {
        self.status = StrategyStatus::Paused;
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// Disable the strategy.
    pub fn disable(&mut self) {
        self.status = StrategyStatus::Disabled;
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// Check if the strategy is in cooldown.
    pub fn is_in_cooldown(&self) -> bool {
        if let Some(last_trade) = self.last_trade_at {
            let now = chrono::Utc::now().timestamp_millis();
            let cooldown_ms = self.cooldown_seconds as i64 * 1000;
            now - last_trade < cooldown_ms
        } else {
            false
        }
    }

    /// Record a trade execution.
    pub fn record_trade(&mut self, is_profitable: bool, pnl: f64) {
        self.total_trades += 1;
        if is_profitable {
            self.winning_trades += 1;
        } else {
            self.losing_trades += 1;
        }
        self.realized_pnl += pnl;
        self.last_trade_at = Some(chrono::Utc::now().timestamp_millis());
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// Get the win rate.
    pub fn win_rate(&self) -> f64 {
        if self.total_trades == 0 {
            0.0
        } else {
            self.winning_trades as f64 / self.total_trades as f64
        }
    }

    /// Check if strategy can trade (active and not in cooldown).
    pub fn can_trade(&self) -> bool {
        self.status == StrategyStatus::Active && !self.is_in_cooldown()
    }
}

/// Result of a strategy evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategySignal {
    /// Strategy that generated the signal
    pub strategy_id: String,
    /// Rule that triggered
    pub rule_id: String,
    /// Symbol to trade
    pub symbol: String,
    /// Action to take
    pub action: RuleAction,
    /// Signal strength (0.0-1.0)
    pub strength: f64,
    /// When signal was generated
    pub generated_at: i64,
}

impl StrategySignal {
    /// Create a new strategy signal.
    pub fn new(strategy_id: String, rule_id: String, symbol: String, action: RuleAction) -> Self {
        Self {
            strategy_id,
            rule_id,
            symbol,
            action,
            strength: 1.0,
            generated_at: chrono::Utc::now().timestamp_millis(),
        }
    }

    /// Set signal strength.
    pub fn with_strength(mut self, strength: f64) -> Self {
        self.strength = strength.clamp(0.0, 1.0);
        self
    }
}

// =============================================================================
// Backtesting
// =============================================================================

/// Status of a backtest run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BacktestStatus {
    /// Backtest is queued
    Pending,
    /// Backtest is running
    Running,
    /// Backtest completed successfully
    Completed,
    /// Backtest failed with error
    Failed,
    /// Backtest was cancelled
    Cancelled,
}

/// Configuration for a backtest run.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BacktestConfig {
    /// Strategy to test
    pub strategy_id: String,
    /// Symbols to test (if empty, uses strategy symbols)
    pub symbols: Vec<String>,
    /// Start timestamp (ms)
    pub start_time: i64,
    /// End timestamp (ms)
    pub end_time: i64,
    /// Initial portfolio balance
    pub initial_balance: f64,
    /// Commission rate per trade (e.g., 0.001 = 0.1%)
    pub commission_rate: f64,
    /// Slippage simulation in percent (e.g., 0.001 = 0.1%)
    pub slippage_pct: f64,
    /// Use OHLC candles or just close price
    pub use_ohlc: bool,
    /// Candle interval for simulation (seconds)
    pub candle_interval: u32,
    /// Enable margin trading simulation
    pub enable_margin: bool,
    /// Enable Monte Carlo simulation
    pub monte_carlo_runs: Option<u32>,
}

impl BacktestConfig {
    /// Create a new backtest config with defaults.
    pub fn new(strategy_id: String, start_time: i64, end_time: i64) -> Self {
        Self {
            strategy_id,
            symbols: Vec::new(),
            start_time,
            end_time,
            initial_balance: 10_000.0,
            commission_rate: 0.001,
            slippage_pct: 0.0005,
            use_ohlc: true,
            candle_interval: 300, // 5 minute candles
            enable_margin: false,
            monte_carlo_runs: None,
        }
    }

    /// Duration of backtest in milliseconds.
    pub fn duration_ms(&self) -> i64 {
        self.end_time - self.start_time
    }

    /// Duration of backtest in days.
    pub fn duration_days(&self) -> f64 {
        self.duration_ms() as f64 / (24.0 * 60.0 * 60.0 * 1000.0)
    }
}

/// A single trade executed during backtesting.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BacktestTrade {
    /// Trade ID
    pub id: String,
    /// Symbol traded
    pub symbol: String,
    /// Trade side
    pub side: OrderSide,
    /// Entry price
    pub entry_price: f64,
    /// Exit price (None if still open)
    pub exit_price: Option<f64>,
    /// Quantity
    pub quantity: f64,
    /// Entry timestamp (ms)
    pub entry_time: i64,
    /// Exit timestamp (ms, None if still open)
    pub exit_time: Option<i64>,
    /// Realized P&L (set when closed)
    pub pnl: f64,
    /// P&L percentage
    pub pnl_pct: f64,
    /// Commission paid
    pub commission: f64,
    /// Rule that triggered entry
    pub entry_rule_id: Option<String>,
    /// Rule that triggered exit
    pub exit_rule_id: Option<String>,
    /// Whether this was a winning trade
    pub is_winner: bool,
    /// Maximum favorable excursion (best unrealized P&L)
    pub max_favorable_excursion: f64,
    /// Maximum adverse excursion (worst unrealized P&L)
    pub max_adverse_excursion: f64,
}

impl BacktestTrade {
    /// Create a new open trade.
    pub fn open(
        symbol: String,
        side: OrderSide,
        entry_price: f64,
        quantity: f64,
        entry_time: i64,
        entry_rule_id: Option<String>,
        commission: f64,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            symbol,
            side,
            entry_price,
            exit_price: None,
            quantity,
            entry_time,
            exit_time: None,
            pnl: -commission, // Start with commission loss
            pnl_pct: 0.0,
            commission,
            entry_rule_id,
            exit_rule_id: None,
            is_winner: false,
            max_favorable_excursion: 0.0,
            max_adverse_excursion: 0.0,
        }
    }

    /// Close the trade.
    pub fn close(
        &mut self,
        exit_price: f64,
        exit_time: i64,
        exit_rule_id: Option<String>,
        exit_commission: f64,
    ) {
        self.exit_price = Some(exit_price);
        self.exit_time = Some(exit_time);
        self.exit_rule_id = exit_rule_id;
        self.commission += exit_commission;

        // Calculate P&L
        let gross_pnl = match self.side {
            OrderSide::Buy => (exit_price - self.entry_price) * self.quantity,
            OrderSide::Sell => (self.entry_price - exit_price) * self.quantity,
        };
        self.pnl = gross_pnl - self.commission;
        self.pnl_pct = self.pnl / (self.entry_price * self.quantity) * 100.0;
        self.is_winner = self.pnl > 0.0;
    }

    /// Update maximum excursions during trade.
    pub fn update_excursion(&mut self, current_price: f64) {
        let unrealized = match self.side {
            OrderSide::Buy => (current_price - self.entry_price) * self.quantity,
            OrderSide::Sell => (self.entry_price - current_price) * self.quantity,
        };

        if unrealized > self.max_favorable_excursion {
            self.max_favorable_excursion = unrealized;
        }
        if unrealized < self.max_adverse_excursion {
            self.max_adverse_excursion = unrealized;
        }
    }

    /// Check if trade is still open.
    pub fn is_open(&self) -> bool {
        self.exit_price.is_none()
    }

    /// Duration of trade in milliseconds.
    pub fn duration_ms(&self) -> Option<i64> {
        self.exit_time.map(|exit| exit - self.entry_time)
    }
}

/// A point on the equity curve.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EquityPoint {
    /// Timestamp (ms)
    pub timestamp: i64,
    /// Portfolio equity (cash + positions)
    pub equity: f64,
    /// Cash balance
    pub cash: f64,
    /// Positions value
    pub positions_value: f64,
    /// Cumulative realized P&L
    pub realized_pnl: f64,
    /// Current unrealized P&L
    pub unrealized_pnl: f64,
    /// Drawdown from peak (percentage)
    pub drawdown_pct: f64,
}

/// Performance metrics for a backtest.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BacktestMetrics {
    // -- Return Metrics --
    /// Total return percentage
    pub total_return_pct: f64,
    /// Annualized return percentage
    pub annualized_return_pct: f64,
    /// Total P&L in currency
    pub total_pnl: f64,
    /// Gross profit (sum of winning trades)
    pub gross_profit: f64,
    /// Gross loss (sum of losing trades)
    pub gross_loss: f64,
    /// Profit factor (gross profit / gross loss)
    pub profit_factor: f64,

    // -- Risk Metrics --
    /// Maximum drawdown percentage
    pub max_drawdown_pct: f64,
    /// Maximum drawdown in currency
    pub max_drawdown: f64,
    /// Average drawdown percentage
    pub avg_drawdown_pct: f64,
    /// Sharpe ratio (annualized)
    pub sharpe_ratio: f64,
    /// Sortino ratio (annualized)
    pub sortino_ratio: f64,
    /// Calmar ratio (annual return / max drawdown)
    pub calmar_ratio: f64,
    /// Daily volatility (standard deviation of returns)
    pub daily_volatility: f64,

    // -- Trade Metrics --
    /// Total number of trades
    pub total_trades: u32,
    /// Number of winning trades
    pub winning_trades: u32,
    /// Number of losing trades
    pub losing_trades: u32,
    /// Win rate percentage
    pub win_rate_pct: f64,
    /// Average trade P&L
    pub avg_trade_pnl: f64,
    /// Average winning trade P&L
    pub avg_win: f64,
    /// Average losing trade P&L
    pub avg_loss: f64,
    /// Largest winning trade
    pub largest_win: f64,
    /// Largest losing trade
    pub largest_loss: f64,
    /// Average trade duration (ms)
    pub avg_trade_duration_ms: i64,
    /// Expectancy (avg win * win rate - avg loss * loss rate)
    pub expectancy: f64,

    // -- Streak Metrics --
    /// Maximum consecutive wins
    pub max_consecutive_wins: u32,
    /// Maximum consecutive losses
    pub max_consecutive_losses: u32,
    /// Current streak (positive = wins, negative = losses)
    pub current_streak: i32,

    // -- Exposure Metrics --
    /// Percentage of time in market
    pub time_in_market_pct: f64,
    /// Total commission paid
    pub total_commission: f64,
    /// Total slippage cost
    pub total_slippage: f64,
}

impl Default for BacktestMetrics {
    fn default() -> Self {
        Self {
            total_return_pct: 0.0,
            annualized_return_pct: 0.0,
            total_pnl: 0.0,
            gross_profit: 0.0,
            gross_loss: 0.0,
            profit_factor: 0.0,
            max_drawdown_pct: 0.0,
            max_drawdown: 0.0,
            avg_drawdown_pct: 0.0,
            sharpe_ratio: 0.0,
            sortino_ratio: 0.0,
            calmar_ratio: 0.0,
            daily_volatility: 0.0,
            total_trades: 0,
            winning_trades: 0,
            losing_trades: 0,
            win_rate_pct: 0.0,
            avg_trade_pnl: 0.0,
            avg_win: 0.0,
            avg_loss: 0.0,
            largest_win: 0.0,
            largest_loss: 0.0,
            avg_trade_duration_ms: 0,
            expectancy: 0.0,
            max_consecutive_wins: 0,
            max_consecutive_losses: 0,
            current_streak: 0,
            time_in_market_pct: 0.0,
            total_commission: 0.0,
            total_slippage: 0.0,
        }
    }
}

/// Buy-and-hold comparison metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuyAndHoldComparison {
    /// Buy and hold return percentage
    pub bnh_return_pct: f64,
    /// Strategy outperformance (strategy return - bnh return)
    pub outperformance_pct: f64,
    /// Strategy max drawdown
    pub strategy_max_dd: f64,
    /// Buy and hold max drawdown
    pub bnh_max_dd: f64,
    /// Strategy Sharpe
    pub strategy_sharpe: f64,
    /// Buy and hold Sharpe
    pub bnh_sharpe: f64,
}

/// Monte Carlo simulation results.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonteCarloResults {
    /// Number of simulation runs
    pub num_runs: u32,
    /// 5th percentile return
    pub return_p5: f64,
    /// 25th percentile return
    pub return_p25: f64,
    /// 50th percentile return (median)
    pub return_p50: f64,
    /// 75th percentile return
    pub return_p75: f64,
    /// 95th percentile return
    pub return_p95: f64,
    /// 5th percentile max drawdown
    pub max_dd_p5: f64,
    /// 50th percentile max drawdown
    pub max_dd_p50: f64,
    /// 95th percentile max drawdown (worst case)
    pub max_dd_p95: f64,
    /// Probability of profit (% of runs with positive return)
    pub probability_of_profit: f64,
    /// Probability of ruin (% of runs with > 50% drawdown)
    pub probability_of_ruin: f64,
}

/// Complete backtest result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BacktestResult {
    /// Unique ID for this backtest
    pub id: String,
    /// Strategy ID that was tested
    pub strategy_id: String,
    /// Backtest status
    pub status: BacktestStatus,
    /// Configuration used
    pub config: BacktestConfig,
    /// Performance metrics
    pub metrics: BacktestMetrics,
    /// All trades executed
    pub trades: Vec<BacktestTrade>,
    /// Equity curve (sampled)
    pub equity_curve: Vec<EquityPoint>,
    /// Buy and hold comparison
    pub buy_and_hold: Option<BuyAndHoldComparison>,
    /// Monte Carlo results (if enabled)
    pub monte_carlo: Option<MonteCarloResults>,
    /// Final portfolio balance
    pub final_balance: f64,
    /// Error message if failed
    pub error_message: Option<String>,
    /// When backtest was created
    pub created_at: i64,
    /// When backtest started running
    pub started_at: Option<i64>,
    /// When backtest completed
    pub completed_at: Option<i64>,
    /// Execution time in milliseconds
    pub execution_time_ms: Option<i64>,
}

impl BacktestResult {
    /// Create a new pending backtest result.
    pub fn new(strategy_id: String, config: BacktestConfig) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            strategy_id,
            status: BacktestStatus::Pending,
            config,
            metrics: BacktestMetrics::default(),
            trades: Vec::new(),
            equity_curve: Vec::new(),
            buy_and_hold: None,
            monte_carlo: None,
            final_balance: 0.0,
            error_message: None,
            created_at: chrono::Utc::now().timestamp_millis(),
            started_at: None,
            completed_at: None,
            execution_time_ms: None,
        }
    }

    /// Mark as started.
    pub fn start(&mut self) {
        self.status = BacktestStatus::Running;
        self.started_at = Some(chrono::Utc::now().timestamp_millis());
    }

    /// Mark as completed.
    pub fn complete(&mut self, final_balance: f64) {
        self.status = BacktestStatus::Completed;
        self.final_balance = final_balance;
        self.completed_at = Some(chrono::Utc::now().timestamp_millis());
        if let Some(started) = self.started_at {
            self.execution_time_ms = Some(self.completed_at.unwrap() - started);
        }
    }

    /// Mark as failed.
    pub fn fail(&mut self, error: String) {
        self.status = BacktestStatus::Failed;
        self.error_message = Some(error);
        self.completed_at = Some(chrono::Utc::now().timestamp_millis());
        if let Some(started) = self.started_at {
            self.execution_time_ms = Some(self.completed_at.unwrap() - started);
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // AssetClass Tests
    // =========================================================================

    #[test]
    fn test_asset_class_max_leverage() {
        assert_eq!(AssetClass::CryptoSpot.max_leverage(), 100.0);
        assert_eq!(AssetClass::Stock.max_leverage(), 4.0);
        assert_eq!(AssetClass::Perp.max_leverage(), 100.0);
        assert_eq!(AssetClass::Forex.max_leverage(), 100.0);
    }

    #[test]
    fn test_asset_class_margins() {
        assert_eq!(AssetClass::CryptoSpot.initial_margin(), 0.01);
        assert_eq!(AssetClass::CryptoSpot.maintenance_margin(), 0.005);
        assert_eq!(AssetClass::Perp.initial_margin(), 0.01);
        assert_eq!(AssetClass::Perp.maintenance_margin(), 0.005);
    }

    #[test]
    fn test_asset_class_serialization() {
        let json = serde_json::to_string(&AssetClass::CryptoSpot).unwrap();
        assert_eq!(json, "\"crypto_spot\"");

        let parsed: AssetClass = serde_json::from_str("\"perp\"").unwrap();
        assert_eq!(parsed, AssetClass::Perp);
    }

    // =========================================================================
    // OrderSide Tests
    // =========================================================================

    #[test]
    fn test_order_side_serialization() {
        assert_eq!(serde_json::to_string(&OrderSide::Buy).unwrap(), "\"buy\"");
        assert_eq!(serde_json::to_string(&OrderSide::Sell).unwrap(), "\"sell\"");
    }

    // =========================================================================
    // OrderType Tests
    // =========================================================================

    #[test]
    fn test_order_type_serialization() {
        assert_eq!(serde_json::to_string(&OrderType::Market).unwrap(), "\"market\"");
        assert_eq!(serde_json::to_string(&OrderType::StopLoss).unwrap(), "\"stop_loss\"");
        assert_eq!(serde_json::to_string(&OrderType::TrailingStop).unwrap(), "\"trailing_stop\"");
    }

    // =========================================================================
    // Portfolio Tests
    // =========================================================================

    #[test]
    fn test_portfolio_creation() {
        let portfolio = Portfolio::new("user123".to_string(), "My Portfolio".to_string());

        assert_eq!(portfolio.user_id, "user123");
        assert_eq!(portfolio.name, "My Portfolio");
        assert_eq!(portfolio.starting_balance, 250_000.0);
        assert_eq!(portfolio.cash_balance, 250_000.0);
        assert_eq!(portfolio.base_currency, "USD");
        assert!(!portfolio.is_competition);
    }

    #[test]
    fn test_portfolio_equity() {
        let mut portfolio = Portfolio::new("user".to_string(), "Test".to_string());
        portfolio.cash_balance = 100_000.0;
        portfolio.unrealized_pnl = 5_000.0;

        assert_eq!(portfolio.equity(), 105_000.0);
    }

    #[test]
    fn test_portfolio_margin_level() {
        let mut portfolio = Portfolio::new("user".to_string(), "Test".to_string());
        portfolio.cash_balance = 100_000.0;
        portfolio.margin_used = 50_000.0;
        portfolio.unrealized_pnl = 0.0;

        assert_eq!(portfolio.margin_level(), 200.0); // 100%
    }

    #[test]
    fn test_portfolio_total_return() {
        let mut portfolio = Portfolio::new("user".to_string(), "Test".to_string());
        portfolio.total_value = 275_000.0; // 10% gain from $250k

        assert!((portfolio.total_return_pct() - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_portfolio_is_stopped() {
        let mut portfolio = Portfolio::new("user".to_string(), "Test".to_string());
        portfolio.risk_settings.portfolio_stop_pct = 0.25;

        // Not stopped yet - 20% drawdown from $250k = $200k
        portfolio.total_value = 200_000.0;
        assert!(!portfolio.is_stopped());

        // Now stopped - 30% drawdown from $250k = $175k
        portfolio.total_value = 175_000.0;
        assert!(portfolio.is_stopped());
    }

    #[test]
    fn test_portfolio_serialization() {
        let portfolio = Portfolio::new("user123".to_string(), "Test Portfolio".to_string());
        let json = serde_json::to_string(&portfolio).unwrap();

        assert!(json.contains("\"userId\":\"user123\""));
        assert!(json.contains("\"startingBalance\":250000"));
        assert!(json.contains("\"baseCurrency\":\"USD\""));
    }

    // =========================================================================
    // Order Tests
    // =========================================================================

    #[test]
    fn test_market_order_creation() {
        let order = Order::market(
            "portfolio1".to_string(),
            "BTC".to_string(),
            AssetClass::CryptoSpot,
            OrderSide::Buy,
            1.0,
        );

        assert_eq!(order.order_type, OrderType::Market);
        assert_eq!(order.symbol, "BTC");
        assert_eq!(order.side, OrderSide::Buy);
        assert_eq!(order.quantity, 1.0);
        assert_eq!(order.status, OrderStatus::Pending);
        assert!(order.price.is_none());
    }

    #[test]
    fn test_limit_order_creation() {
        let order = Order::limit(
            "portfolio1".to_string(),
            "ETH".to_string(),
            AssetClass::CryptoSpot,
            OrderSide::Sell,
            10.0,
            2500.0,
        );

        assert_eq!(order.order_type, OrderType::Limit);
        assert_eq!(order.price, Some(2500.0));
    }

    #[test]
    fn test_stop_loss_order_creation() {
        let order = Order::stop_loss(
            "portfolio1".to_string(),
            "AAPL".to_string(),
            AssetClass::Stock,
            OrderSide::Sell,
            100.0,
            150.0,
        );

        assert_eq!(order.order_type, OrderType::StopLoss);
        assert_eq!(order.stop_price, Some(150.0));
    }

    #[test]
    fn test_order_is_terminal() {
        let mut order = Order::market(
            "p1".to_string(),
            "BTC".to_string(),
            AssetClass::CryptoSpot,
            OrderSide::Buy,
            1.0,
        );

        assert!(!order.is_terminal());

        order.status = OrderStatus::Filled;
        assert!(order.is_terminal());

        order.status = OrderStatus::Cancelled;
        assert!(order.is_terminal());
    }

    #[test]
    fn test_order_can_cancel() {
        let mut order = Order::market(
            "p1".to_string(),
            "BTC".to_string(),
            AssetClass::CryptoSpot,
            OrderSide::Buy,
            1.0,
        );

        order.status = OrderStatus::Open;
        assert!(order.can_cancel());

        order.status = OrderStatus::Filled;
        assert!(!order.can_cancel());
    }

    #[test]
    fn test_order_add_fill() {
        let mut order = Order::market(
            "p1".to_string(),
            "BTC".to_string(),
            AssetClass::CryptoSpot,
            OrderSide::Buy,
            2.0,
        );

        order.add_fill(Fill::new(1.0, 50000.0, 50.0));

        assert_eq!(order.filled_quantity, 1.0);
        assert_eq!(order.total_fees, 50.0);
        assert_eq!(order.avg_fill_price, Some(50000.0));
        assert_eq!(order.status, OrderStatus::PartiallyFilled);

        order.add_fill(Fill::new(1.0, 51000.0, 51.0));

        assert_eq!(order.filled_quantity, 2.0);
        assert_eq!(order.total_fees, 101.0);
        assert_eq!(order.avg_fill_price, Some(50500.0));
        assert_eq!(order.status, OrderStatus::Filled);
    }

    #[test]
    fn test_order_remaining_quantity() {
        let mut order = Order::market(
            "p1".to_string(),
            "BTC".to_string(),
            AssetClass::CryptoSpot,
            OrderSide::Buy,
            5.0,
        );

        assert_eq!(order.remaining_quantity(), 5.0);

        order.filled_quantity = 2.0;
        assert_eq!(order.remaining_quantity(), 3.0);
    }

    #[test]
    fn test_order_serialization() {
        let order = Order::limit(
            "p1".to_string(),
            "BTC".to_string(),
            AssetClass::CryptoSpot,
            OrderSide::Buy,
            1.0,
            50000.0,
        );

        let json = serde_json::to_string(&order).unwrap();
        assert!(json.contains("\"orderType\":\"limit\""));
        assert!(json.contains("\"price\":50000"));
        assert!(json.contains("\"assetClass\":\"crypto_spot\""));
    }

    // =========================================================================
    // Position Tests
    // =========================================================================

    #[test]
    fn test_position_creation() {
        let position = Position::new(
            "portfolio1".to_string(),
            "BTC".to_string(),
            AssetClass::CryptoSpot,
            PositionSide::Long,
            1.0,
            50000.0,
            1.0,
        );

        assert_eq!(position.symbol, "BTC");
        assert_eq!(position.side, PositionSide::Long);
        assert_eq!(position.quantity, 1.0);
        assert_eq!(position.entry_price, 50000.0);
        assert_eq!(position.margin_used, 50000.0); // 1x leverage
    }

    #[test]
    fn test_position_with_leverage() {
        let position = Position::new(
            "portfolio1".to_string(),
            "BTC".to_string(),
            AssetClass::Perp,
            PositionSide::Long,
            1.0,
            50000.0,
            10.0,
        );

        assert_eq!(position.margin_used, 5000.0); // 10x leverage
    }

    #[test]
    fn test_position_update_price_long() {
        let mut position = Position::new(
            "p1".to_string(),
            "BTC".to_string(),
            AssetClass::CryptoSpot,
            PositionSide::Long,
            1.0,
            50000.0,
            1.0,
        );

        position.update_price(55000.0);

        assert_eq!(position.current_price, 55000.0);
        assert_eq!(position.unrealized_pnl, 5000.0);
        assert!((position.unrealized_pnl_pct - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_position_update_price_short() {
        let mut position = Position::new(
            "p1".to_string(),
            "BTC".to_string(),
            AssetClass::Perp,
            PositionSide::Short,
            1.0,
            50000.0,
            1.0,
        );

        position.update_price(45000.0);

        assert_eq!(position.unrealized_pnl, 5000.0); // Profit on short
    }

    #[test]
    fn test_position_liquidation_price_long() {
        let mut position = Position::new(
            "p1".to_string(),
            "BTC".to_string(),
            AssetClass::Perp,
            PositionSide::Long,
            1.0,
            50000.0,
            10.0, // 10x leverage
        );

        position.calculate_liquidation_price();

        // With 10x leverage (10% initial margin) and 0.5% maintenance
        // Liquidation = 50000 * (1 - 0.1 + 0.005) = 50000 * 0.905 = 45250
        assert!(position.liquidation_price.is_some());
        let liq = position.liquidation_price.unwrap();
        assert!((liq - 45250.0).abs() < 1.0);
    }

    #[test]
    fn test_position_should_liquidate() {
        let mut position = Position::new(
            "p1".to_string(),
            "BTC".to_string(),
            AssetClass::Perp,
            PositionSide::Long,
            1.0,
            50000.0,
            10.0,
        );

        position.calculate_liquidation_price();

        position.current_price = 46000.0;
        assert!(!position.should_liquidate());

        position.current_price = 45000.0;
        assert!(position.should_liquidate());
    }

    #[test]
    fn test_position_stop_loss_trigger() {
        let mut position = Position::new(
            "p1".to_string(),
            "BTC".to_string(),
            AssetClass::CryptoSpot,
            PositionSide::Long,
            1.0,
            50000.0,
            1.0,
        );

        position.stop_loss = Some(48000.0);

        position.current_price = 49000.0;
        assert!(!position.should_stop_loss());

        position.current_price = 47500.0;
        assert!(position.should_stop_loss());
    }

    #[test]
    fn test_position_take_profit_trigger() {
        let mut position = Position::new(
            "p1".to_string(),
            "BTC".to_string(),
            AssetClass::CryptoSpot,
            PositionSide::Long,
            1.0,
            50000.0,
            1.0,
        );

        position.take_profit = Some(55000.0);

        position.current_price = 54000.0;
        assert!(!position.should_take_profit());

        position.current_price = 56000.0;
        assert!(position.should_take_profit());
    }

    #[test]
    fn test_position_notional_value() {
        let mut position = Position::new(
            "p1".to_string(),
            "BTC".to_string(),
            AssetClass::CryptoSpot,
            PositionSide::Long,
            2.0,
            50000.0,
            1.0,
        );

        position.current_price = 52000.0;
        assert_eq!(position.notional_value(), 104000.0);
    }

    #[test]
    fn test_position_serialization() {
        let position = Position::new(
            "p1".to_string(),
            "ETH".to_string(),
            AssetClass::CryptoSpot,
            PositionSide::Long,
            10.0,
            2500.0,
            1.0,
        );

        let json = serde_json::to_string(&position).unwrap();
        assert!(json.contains("\"symbol\":\"ETH\""));
        assert!(json.contains("\"side\":\"long\""));
        assert!(json.contains("\"entryPrice\":2500"));
    }

    // =========================================================================
    // Trade Tests
    // =========================================================================

    #[test]
    fn test_trade_creation() {
        let trade = Trade::new(
            "order1".to_string(),
            "portfolio1".to_string(),
            "BTC".to_string(),
            AssetClass::CryptoSpot,
            OrderSide::Buy,
            1.0,
            50000.0,
            50.0,
            10.0,
        );

        assert_eq!(trade.order_id, "order1");
        assert_eq!(trade.symbol, "BTC");
        assert_eq!(trade.quantity, 1.0);
        assert_eq!(trade.price, 50000.0);
        assert_eq!(trade.fee, 50.0);
        assert_eq!(trade.slippage, 10.0);
    }

    #[test]
    fn test_trade_total_cost() {
        let trade = Trade::new(
            "o1".to_string(),
            "p1".to_string(),
            "BTC".to_string(),
            AssetClass::CryptoSpot,
            OrderSide::Buy,
            2.0,
            50000.0,
            100.0,
            0.0,
        );

        assert_eq!(trade.total_cost(), 100100.0);
    }

    #[test]
    fn test_trade_serialization() {
        let trade = Trade::new(
            "o1".to_string(),
            "p1".to_string(),
            "AAPL".to_string(),
            AssetClass::Stock,
            OrderSide::Sell,
            100.0,
            180.0,
            1.0,
            0.05,
        );

        let json = serde_json::to_string(&trade).unwrap();
        assert!(json.contains("\"symbol\":\"AAPL\""));
        assert!(json.contains("\"side\":\"sell\""));
        assert!(json.contains("\"assetClass\":\"stock\""));
    }

    // =========================================================================
    // RiskSettings Tests
    // =========================================================================

    #[test]
    fn test_risk_settings_default() {
        let settings = RiskSettings::default();

        assert_eq!(settings.max_position_size_pct, 0.25);
        assert_eq!(settings.daily_loss_limit_pct, 0.10);
        assert_eq!(settings.max_open_positions, 20);
        assert_eq!(settings.risk_per_trade_pct, 0.02);
        assert_eq!(settings.portfolio_stop_pct, 0.25);
    }

    #[test]
    fn test_risk_settings_serialization() {
        let settings = RiskSettings {
            max_position_size_pct: 0.5,
            daily_loss_limit_pct: 0.05,
            max_open_positions: 10,
            risk_per_trade_pct: 0.01,
            portfolio_stop_pct: 0.15,
        };

        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("\"maxPositionSizePct\":0.5"));
        assert!(json.contains("\"dailyLossLimitPct\":0.05"));
    }

    // =========================================================================
    // Fill Tests
    // =========================================================================

    #[test]
    fn test_fill_creation() {
        let fill = Fill::new(1.5, 50000.0, 25.0);

        assert_eq!(fill.quantity, 1.5);
        assert_eq!(fill.price, 50000.0);
        assert_eq!(fill.fee, 25.0);
        assert!(!fill.id.is_empty());
    }

    // =========================================================================
    // CostBasisEntry Tests
    // =========================================================================

    #[test]
    fn test_cost_basis_entry_serialization() {
        let entry = CostBasisEntry {
            quantity: 10.0,
            price: 100.0,
            acquired_at: 1704067200000,
        };

        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"quantity\":10"));
        assert!(json.contains("\"price\":100"));
        assert!(json.contains("\"acquiredAt\":1704067200000"));
    }

    // =========================================================================
    // Request Types Tests
    // =========================================================================

    #[test]
    fn test_create_portfolio_request_deserialization() {
        let json = r#"{"name": "Test Portfolio", "description": "A test"}"#;
        let request: CreatePortfolioRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.name, "Test Portfolio");
        assert_eq!(request.description, Some("A test".to_string()));
    }

    #[test]
    fn test_place_order_request_deserialization() {
        let json = r#"{
            "portfolioId": "p1",
            "symbol": "BTC",
            "assetClass": "crypto_spot",
            "side": "buy",
            "orderType": "market",
            "quantity": 1.0
        }"#;

        let request: PlaceOrderRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.portfolio_id, "p1");
        assert_eq!(request.symbol, "BTC");
        assert_eq!(request.asset_class, AssetClass::CryptoSpot);
        assert_eq!(request.side, OrderSide::Buy);
        assert_eq!(request.order_type, OrderType::Market);
        assert_eq!(request.quantity, 1.0);
    }

    #[test]
    fn test_place_order_request_with_options() {
        let json = r#"{
            "portfolioId": "p1",
            "symbol": "ETH",
            "assetClass": "perp",
            "side": "sell",
            "orderType": "limit",
            "quantity": 10.0,
            "price": 2500.0,
            "leverage": 5.0,
            "stopLoss": 2600.0,
            "takeProfit": 2300.0
        }"#;

        let request: PlaceOrderRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.price, Some(2500.0));
        assert_eq!(request.leverage, Some(5.0));
        assert_eq!(request.stop_loss, Some(2600.0));
        assert_eq!(request.take_profit, Some(2300.0));
    }

    // =========================================================================
    // PortfolioSummary Tests
    // =========================================================================

    #[test]
    fn test_portfolio_summary_serialization() {
        let summary = PortfolioSummary {
            portfolio_id: "p1".to_string(),
            total_value: 5_100_000.0,
            cash_balance: 4_500_000.0,
            unrealized_pnl: 100_000.0,
            realized_pnl: 50_000.0,
            total_return_pct: 2.0,
            margin_used: 500_000.0,
            margin_available: 4_000_000.0,
            margin_level: 900.0,
            open_positions: 5,
            open_orders: 3,
        };

        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.contains("\"totalValue\":5100000"));
        assert!(json.contains("\"openPositions\":5"));
    }

    // =========================================================================
    // Leverage Tier Tests
    // =========================================================================

    #[test]
    fn test_leverage_tier_for_small_position() {
        let tier = LeverageTier::for_position_size(25_000.0);
        assert_eq!(tier.max_leverage, 100.0);
        assert_eq!(tier.initial_margin_rate, 0.01);
        assert_eq!(tier.maintenance_margin_rate, 0.005);
    }

    #[test]
    fn test_leverage_tier_for_medium_position() {
        let tier = LeverageTier::for_position_size(100_000.0);
        assert_eq!(tier.max_leverage, 50.0);
        assert_eq!(tier.initial_margin_rate, 0.02);
    }

    #[test]
    fn test_leverage_tier_for_large_position() {
        let tier = LeverageTier::for_position_size(2_000_000.0);
        assert_eq!(tier.max_leverage, 10.0);
        assert_eq!(tier.initial_margin_rate, 0.10);
    }

    #[test]
    fn test_leverage_tier_for_very_large_position() {
        let tier = LeverageTier::for_position_size(10_000_000.0);
        assert_eq!(tier.max_leverage, 5.0);
        assert_eq!(tier.initial_margin_rate, 0.20);
    }

    // =========================================================================
    // Funding Rate Tests
    // =========================================================================

    #[test]
    fn test_funding_rate_creation() {
        let rate = FundingRate::new("BTC-PERP".to_string(), 0.0001, 50000.0, 50010.0);

        assert_eq!(rate.symbol, "BTC-PERP");
        assert_eq!(rate.rate, 0.0001);
        assert_eq!(rate.index_price, 50000.0);
        assert_eq!(rate.mark_price, 50010.0);
        assert_eq!(rate.funding_interval_ms, 8 * 60 * 60 * 1000);
    }

    #[test]
    fn test_funding_rate_payment_calculation_long() {
        let rate = FundingRate::new("BTC-PERP".to_string(), 0.0001, 50000.0, 50010.0);
        let payment = rate.calculate_payment(100_000.0, PositionSide::Long);

        // Positive rate, longs pay: 100000 * 0.0001 = 10
        assert_eq!(payment, 10.0);
    }

    #[test]
    fn test_funding_rate_payment_calculation_short() {
        let rate = FundingRate::new("BTC-PERP".to_string(), 0.0001, 50000.0, 50010.0);
        let payment = rate.calculate_payment(100_000.0, PositionSide::Short);

        // Positive rate, shorts receive: -10
        assert_eq!(payment, -10.0);
    }

    #[test]
    fn test_funding_rate_serialization() {
        let rate = FundingRate::new("ETH-PERP".to_string(), -0.0002, 2500.0, 2498.0);
        let json = serde_json::to_string(&rate).unwrap();

        assert!(json.contains("\"symbol\":\"ETH-PERP\""));
        assert!(json.contains("\"rate\":-0.0002"));
        assert!(json.contains("\"indexPrice\":2500"));
    }

    // =========================================================================
    // Funding Payment Tests
    // =========================================================================

    #[test]
    fn test_funding_payment_creation() {
        let payment = FundingPayment::new(
            "pos1".to_string(),
            "port1".to_string(),
            "BTC-PERP".to_string(),
            100_000.0,
            PositionSide::Long,
            0.0001,
        );

        assert_eq!(payment.position_id, "pos1");
        assert_eq!(payment.payment, 10.0); // 100000 * 0.0001
        assert!(!payment.id.is_empty());
    }

    #[test]
    fn test_funding_payment_short_receives() {
        let payment = FundingPayment::new(
            "pos1".to_string(),
            "port1".to_string(),
            "BTC-PERP".to_string(),
            100_000.0,
            PositionSide::Short,
            0.0001,
        );

        // Shorts receive when rate is positive
        assert_eq!(payment.payment, -10.0);
    }

    // =========================================================================
    // Liquidation Warning Level Tests
    // =========================================================================

    #[test]
    fn test_liquidation_warning_level_thresholds() {
        assert_eq!(LiquidationWarningLevel::Warning80.margin_level_threshold(), 125.0);
        assert_eq!(LiquidationWarningLevel::Warning90.margin_level_threshold(), 111.0);
        assert_eq!(LiquidationWarningLevel::Warning95.margin_level_threshold(), 105.0);
        assert_eq!(LiquidationWarningLevel::Liquidation.margin_level_threshold(), 100.0);
    }

    #[test]
    fn test_liquidation_warning_level_from_margin() {
        assert_eq!(
            LiquidationWarningLevel::from_margin_level(150.0),
            None
        );
        assert_eq!(
            LiquidationWarningLevel::from_margin_level(120.0),
            Some(LiquidationWarningLevel::Warning80)
        );
        assert_eq!(
            LiquidationWarningLevel::from_margin_level(108.0),
            Some(LiquidationWarningLevel::Warning90)
        );
        assert_eq!(
            LiquidationWarningLevel::from_margin_level(103.0),
            Some(LiquidationWarningLevel::Warning95)
        );
        assert_eq!(
            LiquidationWarningLevel::from_margin_level(99.0),
            Some(LiquidationWarningLevel::Liquidation)
        );
    }

    // =========================================================================
    // Liquidation Tests
    // =========================================================================

    #[test]
    fn test_liquidation_creation() {
        let liquidation = Liquidation::new(
            "pos1".to_string(),
            "port1".to_string(),
            "BTC-PERP".to_string(),
            1.0,
            45000.0,  // liquidation price
            45100.0,  // mark price
            50000.0,  // entry price
            PositionSide::Long,
            false,
            None,
        );

        assert_eq!(liquidation.position_id, "pos1");
        assert_eq!(liquidation.quantity, 1.0);
        assert_eq!(liquidation.loss, 5000.0); // 50000 - 45000
        assert_eq!(liquidation.liquidation_fee, 225.0); // 45000 * 0.005
        assert!(!liquidation.is_partial);
    }

    #[test]
    fn test_partial_liquidation() {
        let liquidation = Liquidation::new(
            "pos1".to_string(),
            "port1".to_string(),
            "BTC-PERP".to_string(),
            0.5,
            45000.0,
            45100.0,
            50000.0,
            PositionSide::Long,
            true,
            Some(0.5),
        );

        assert!(liquidation.is_partial);
        assert_eq!(liquidation.remaining_quantity, Some(0.5));
        assert_eq!(liquidation.loss, 2500.0); // (50000 - 45000) * 0.5
    }

    // =========================================================================
    // Insurance Fund Tests
    // =========================================================================

    #[test]
    fn test_insurance_fund_default() {
        let fund = InsuranceFund::default();
        assert_eq!(fund.balance, 0.0);
        assert_eq!(fund.total_contributions, 0.0);
        assert_eq!(fund.total_payouts, 0.0);
    }

    #[test]
    fn test_insurance_fund_contribution() {
        let mut fund = InsuranceFund::default();
        fund.add_contribution(1000.0);

        assert_eq!(fund.balance, 1000.0);
        assert_eq!(fund.total_contributions, 1000.0);
    }

    #[test]
    fn test_insurance_fund_cover_loss() {
        let mut fund = InsuranceFund::default();
        fund.add_contribution(5000.0);

        let payout = fund.cover_loss(3000.0);

        assert_eq!(payout, 3000.0);
        assert_eq!(fund.balance, 2000.0);
        assert_eq!(fund.total_payouts, 3000.0);
        assert_eq!(fund.liquidations_covered, 1);
    }

    #[test]
    fn test_insurance_fund_insufficient_balance() {
        let mut fund = InsuranceFund::default();
        fund.add_contribution(1000.0);

        let payout = fund.cover_loss(2000.0);

        // Only pays out what's available
        assert_eq!(payout, 1000.0);
        assert_eq!(fund.balance, 0.0);
    }

    // =========================================================================
    // ADL Entry Tests
    // =========================================================================

    #[test]
    fn test_adl_score_calculation() {
        // High profit + high leverage = high score
        let score = AdlEntry::calculate_score(50.0, 10.0);
        assert_eq!(score, 500.0);

        // Negative profit = 0 score
        let score = AdlEntry::calculate_score(-20.0, 10.0);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_adl_entry_from_position() {
        let mut position = Position::new(
            "port1".to_string(),
            "BTC-PERP".to_string(),
            AssetClass::Perp,
            PositionSide::Long,
            1.0,
            50000.0,
            10.0,
        );
        position.update_price(55000.0); // 10% profit

        let adl = AdlEntry::from_position(&position);

        assert_eq!(adl.position_id, position.id);
        assert_eq!(adl.leverage, 10.0);
        assert!(adl.unrealized_profit > 0.0);
        assert!(adl.adl_score > 0.0);
    }

    // =========================================================================
    // Position Perp Methods Tests
    // =========================================================================

    #[test]
    fn test_position_apply_funding() {
        let mut position = Position::new(
            "port1".to_string(),
            "BTC-PERP".to_string(),
            AssetClass::Perp,
            PositionSide::Long,
            1.0,
            50000.0,
            10.0,
        );

        position.apply_funding(100.0);
        assert_eq!(position.funding_payments, 100.0);

        position.apply_funding(-50.0);
        assert_eq!(position.funding_payments, 50.0);
    }

    #[test]
    fn test_position_leverage_tier() {
        let mut position = Position::new(
            "port1".to_string(),
            "BTC-PERP".to_string(),
            AssetClass::Perp,
            PositionSide::Long,
            1.0,
            30000.0,
            10.0,
        );

        // 30000 notional = tier 1
        let tier = position.leverage_tier();
        assert_eq!(tier.max_leverage, 100.0);

        // 100000 notional = tier 2
        position.current_price = 100000.0;
        let tier = position.leverage_tier();
        assert_eq!(tier.max_leverage, 50.0);
    }

    #[test]
    fn test_position_margin_level() {
        let mut position = Position::new(
            "port1".to_string(),
            "BTC-PERP".to_string(),
            AssetClass::Perp,
            PositionSide::Long,
            1.0,
            50000.0,
            10.0,
        );

        // Initial margin level = 100% (no profit/loss)
        assert!((position.margin_level() - 100.0).abs() < 0.01);

        // With 10% profit
        position.unrealized_pnl = 500.0; // 10% of 5000 margin
        assert!((position.margin_level() - 110.0).abs() < 0.01);
    }

    #[test]
    fn test_position_warning_level() {
        let mut position = Position::new(
            "port1".to_string(),
            "BTC-PERP".to_string(),
            AssetClass::Perp,
            PositionSide::Long,
            1.0,
            50000.0,
            10.0,
        );
        // margin_used = 5000

        // Safe - no warning (margin level > 125%)
        // Need equity > 6250, so unrealized_pnl > 1250
        position.unrealized_pnl = 2000.0; // margin_level = 140%
        assert!(position.warning_level().is_none());

        // 80% warning (margin level 111-125%)
        // equity = 5000 + pnl, margin_level = equity/5000 * 100
        // For 120%: equity = 6000, pnl = 1000
        position.unrealized_pnl = 1000.0; // margin_level = 120%
        assert_eq!(position.warning_level(), Some(LiquidationWarningLevel::Warning80));

        // Liquidation (margin level <= 100%)
        // For 100%: equity = 5000, pnl = 0
        // For less: pnl < 0
        position.unrealized_pnl = -100.0; // margin_level = 98%
        assert_eq!(position.warning_level(), Some(LiquidationWarningLevel::Liquidation));
    }

    #[test]
    fn test_position_validate_leverage() {
        let position = Position::new(
            "port1".to_string(),
            "BTC-PERP".to_string(),
            AssetClass::Perp,
            PositionSide::Long,
            1.0,
            30000.0, // $30k notional, tier 1, max 100x
            50.0,    // 50x leverage (valid)
        );

        assert!(position.validate_leverage().is_ok());

        // Large position with too high leverage
        let position = Position::new(
            "port1".to_string(),
            "BTC-PERP".to_string(),
            AssetClass::Perp,
            PositionSide::Long,
            20.0,
            100000.0, // $2M notional, tier 4, max 10x
            50.0,     // 50x leverage (invalid)
        );

        assert!(position.validate_leverage().is_err());
    }

    #[test]
    fn test_position_maintenance_margin_required() {
        let mut position = Position::new(
            "port1".to_string(),
            "BTC-PERP".to_string(),
            AssetClass::Perp,
            PositionSide::Long,
            1.0,
            30000.0, // $30k notional, tier 1
            10.0,
        );

        // Tier 1: 0.5% maintenance = 150
        let mm = position.maintenance_margin_required();
        assert!((mm - 150.0).abs() < 0.01);

        // Non-perp uses fixed maintenance margin
        position.asset_class = AssetClass::CryptoSpot;
        let mm = position.maintenance_margin_required();
        assert!((mm - 1500.0).abs() < 0.01); // 5% of 30000
    }

    // =========================================================================
    // Auto-Trading Strategy Tests
    // =========================================================================

    #[test]
    fn test_rule_condition_creation() {
        let condition = RuleCondition::new(
            IndicatorType::Rsi,
            ComparisonOperator::LessThan,
            30.0,
        ).with_period(14);

        assert_eq!(condition.indicator, IndicatorType::Rsi);
        assert_eq!(condition.operator, ComparisonOperator::LessThan);
        assert_eq!(condition.value, 30.0);
        assert_eq!(condition.period, Some(14));
    }

    #[test]
    fn test_rule_action_creation() {
        let action = RuleAction::market_buy(PositionSizeType::PortfolioPercent, 5.0)
            .with_stop_loss(3.0)
            .with_take_profit(6.0)
            .with_leverage(2.0);

        assert_eq!(action.action_type, RuleActionType::MarketBuy);
        assert_eq!(action.size_type, PositionSizeType::PortfolioPercent);
        assert_eq!(action.size_value, 5.0);
        assert_eq!(action.stop_loss_pct, Some(3.0));
        assert_eq!(action.take_profit_pct, Some(6.0));
        assert_eq!(action.leverage, 2.0);
    }

    #[test]
    fn test_trading_rule_creation() {
        let conditions = vec![
            RuleCondition::new(IndicatorType::Rsi, ComparisonOperator::LessThan, 30.0),
        ];
        let action = RuleAction::market_buy(PositionSizeType::PortfolioPercent, 2.0);
        let rule = TradingRule::new("RSI Oversold Buy".to_string(), conditions, action);

        assert_eq!(rule.name, "RSI Oversold Buy");
        assert_eq!(rule.conditions.len(), 1);
        assert_eq!(rule.condition_operator, LogicalOperator::And);
        assert!(rule.enabled);
    }

    #[test]
    fn test_trading_strategy_creation() {
        let mut strategy = TradingStrategy::new(
            "port1".to_string(),
            "RSI Strategy".to_string(),
            vec!["BTC".to_string(), "ETH".to_string()],
        );

        assert_eq!(strategy.name, "RSI Strategy");
        assert_eq!(strategy.symbols.len(), 2);
        assert_eq!(strategy.status, StrategyStatus::Paused);
        assert_eq!(strategy.cooldown_seconds, 3600);
        assert_eq!(strategy.max_positions, 3);
        assert!(!strategy.can_trade()); // Paused, can't trade

        // Activate and check
        strategy.activate();
        assert_eq!(strategy.status, StrategyStatus::Active);
        assert!(strategy.can_trade());

        // Pause and check
        strategy.pause();
        assert_eq!(strategy.status, StrategyStatus::Paused);
        assert!(!strategy.can_trade());
    }

    #[test]
    fn test_strategy_trade_recording() {
        let mut strategy = TradingStrategy::new(
            "port1".to_string(),
            "Test Strategy".to_string(),
            vec!["BTC".to_string()],
        );

        // Record some trades
        strategy.record_trade(true, 100.0);
        strategy.record_trade(true, 50.0);
        strategy.record_trade(false, -25.0);

        assert_eq!(strategy.total_trades, 3);
        assert_eq!(strategy.winning_trades, 2);
        assert_eq!(strategy.losing_trades, 1);
        assert_eq!(strategy.realized_pnl, 125.0);
        assert!((strategy.win_rate() - 0.6667).abs() < 0.01);
    }

    #[test]
    fn test_strategy_cooldown() {
        let mut strategy = TradingStrategy::new(
            "port1".to_string(),
            "Test Strategy".to_string(),
            vec!["BTC".to_string()],
        );
        strategy.activate();
        strategy.cooldown_seconds = 3600; // 1 hour

        // No trade yet, not in cooldown
        assert!(!strategy.is_in_cooldown());
        assert!(strategy.can_trade());

        // Record a trade, should be in cooldown
        strategy.record_trade(true, 100.0);
        assert!(strategy.is_in_cooldown());
        assert!(!strategy.can_trade());
    }

    #[test]
    fn test_strategy_signal_creation() {
        let action = RuleAction::market_buy(PositionSizeType::PortfolioPercent, 5.0);
        let signal = StrategySignal::new(
            "strat1".to_string(),
            "rule1".to_string(),
            "BTC".to_string(),
            action,
        ).with_strength(0.8);

        assert_eq!(signal.strategy_id, "strat1");
        assert_eq!(signal.rule_id, "rule1");
        assert_eq!(signal.symbol, "BTC");
        assert_eq!(signal.strength, 0.8);
    }

    #[test]
    fn test_strategy_add_rule() {
        let mut strategy = TradingStrategy::new(
            "port1".to_string(),
            "Multi-Rule Strategy".to_string(),
            vec!["BTC".to_string()],
        );

        // Add first rule
        let rule1 = TradingRule::new(
            "Rule 1".to_string(),
            vec![RuleCondition::new(IndicatorType::Rsi, ComparisonOperator::LessThan, 30.0)],
            RuleAction::market_buy(PositionSizeType::PortfolioPercent, 2.0),
        );
        strategy.add_rule(rule1);

        // Add second rule
        let rule2 = TradingRule::new(
            "Rule 2".to_string(),
            vec![RuleCondition::new(IndicatorType::Rsi, ComparisonOperator::GreaterThan, 70.0)],
            RuleAction::market_sell(PositionSizeType::PortfolioPercent, 100.0),
        );
        strategy.add_rule(rule2);

        assert_eq!(strategy.rules.len(), 2);
        assert_eq!(strategy.rules[0].name, "Rule 1");
        assert_eq!(strategy.rules[1].name, "Rule 2");
    }

    #[test]
    fn test_indicator_type_display() {
        assert_eq!(IndicatorType::Rsi.to_string(), "rsi");
        assert_eq!(IndicatorType::Macd.to_string(), "macd");
        assert_eq!(IndicatorType::Bollinger.to_string(), "bollinger");
    }

    #[test]
    fn test_comparison_operator_display() {
        assert_eq!(ComparisonOperator::LessThan.to_string(), "lt");
        assert_eq!(ComparisonOperator::GreaterThanOrEqual.to_string(), "gte");
        assert_eq!(ComparisonOperator::CrossesAbove.to_string(), "crosses_above");
    }

    #[test]
    fn test_rule_action_type_display() {
        assert_eq!(RuleActionType::MarketBuy.to_string(), "market_buy");
        assert_eq!(RuleActionType::ClosePosition.to_string(), "close_position");
    }

    #[test]
    fn test_strategy_status_display() {
        assert_eq!(StrategyStatus::Active.to_string(), "active");
        assert_eq!(StrategyStatus::Paused.to_string(), "paused");
        assert_eq!(StrategyStatus::Disabled.to_string(), "disabled");
    }
}
