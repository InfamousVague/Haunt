//! Bot configuration types

use serde::{Deserialize, Serialize};
use crate::types::AssetClass;

/// Bot personality types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BotPersonality {
    /// Conservative, long-term focused trader
    Grandma,
    /// Aggressive momentum chaser
    CryptoBro,
    /// Data-driven ML-powered trader
    Quant,
}

impl BotPersonality {
    pub fn display_name(&self) -> &'static str {
        match self {
            BotPersonality::Grandma => "Grandma",
            BotPersonality::CryptoBro => "Crypto Bro",
            BotPersonality::Quant => "Quant",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            BotPersonality::Grandma => "Conservative trader using simple moving averages. Slow and steady wins the race.",
            BotPersonality::CryptoBro => "Aggressive momentum chaser. YOLO energy with occasional diamond hands.",
            BotPersonality::Quant => "Data-driven trader using machine learning. Calculated risk with adaptive strategies.",
        }
    }
}

/// Configuration for a trading bot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotConfig {
    /// Bot's unique identifier
    pub id: String,

    /// Display name
    pub name: String,

    /// Bot personality type
    pub personality: BotPersonality,

    /// Asset classes this bot trades
    pub asset_classes: Vec<AssetClass>,

    /// Specific symbols to trade (empty = trade all available)
    pub symbols: Vec<String>,

    /// Maximum position size as percentage of portfolio (0.0 - 1.0)
    pub max_position_size_pct: f64,

    /// Risk per trade as percentage of portfolio (0.0 - 1.0)
    pub risk_per_trade_pct: f64,

    /// Stop loss percentage (0.0 - 1.0)
    pub stop_loss_pct: f64,

    /// Take profit percentage (0.0 - 1.0)
    pub take_profit_pct: f64,

    /// Maximum trades per day per symbol
    pub max_trades_per_day: u32,

    /// Minimum seconds between trade decisions
    pub decision_interval_secs: u64,

    /// Whether the bot is currently active
    pub enabled: bool,

    /// Starting portfolio value in USD
    pub initial_capital: f64,
}

impl BotConfig {
    /// Create a new Grandma bot configuration
    pub fn grandma() -> Self {
        Self {
            id: "grandma".to_string(),
            name: "Grandma".to_string(),
            personality: BotPersonality::Grandma,
            asset_classes: vec![AssetClass::CryptoSpot, AssetClass::Stock, AssetClass::Forex],
            symbols: vec![],
            max_position_size_pct: 0.05,      // 5% max position
            risk_per_trade_pct: 0.02,         // 2% risk per trade
            stop_loss_pct: 0.10,              // 10% stop loss
            take_profit_pct: 0.20,            // 20% take profit
            max_trades_per_day: 1,            // Max 1 trade per day
            decision_interval_secs: 900,      // Check every 15 minutes
            enabled: true,
            initial_capital: 100_000.0,       // $100k starting
        }
    }

    /// Create a new Crypto Bro bot configuration
    pub fn crypto_bro() -> Self {
        Self {
            id: "crypto_bro".to_string(),
            name: "Crypto Bro".to_string(),
            personality: BotPersonality::CryptoBro,
            asset_classes: vec![AssetClass::CryptoSpot],
            symbols: vec![],
            max_position_size_pct: 0.25,      // 25% max position (YOLO)
            risk_per_trade_pct: 0.10,         // 10% risk per trade
            stop_loss_pct: 0.05,              // 5% stop loss (paper hands)
            take_profit_pct: 0.50,            // 50% take profit (moon)
            max_trades_per_day: 20,           // Very active
            decision_interval_secs: 60,       // Check every minute
            enabled: true,
            initial_capital: 100_000.0,
        }
    }

    /// Create a new Quant bot configuration
    pub fn quant() -> Self {
        Self {
            id: "quant".to_string(),
            name: "Quant".to_string(),
            personality: BotPersonality::Quant,
            asset_classes: vec![AssetClass::CryptoSpot, AssetClass::Stock, AssetClass::Forex],
            symbols: vec![],
            max_position_size_pct: 0.10,      // 10% max position
            risk_per_trade_pct: 0.03,         // 3% risk per trade
            stop_loss_pct: 0.08,              // 8% stop loss
            take_profit_pct: 0.15,            // 15% take profit
            max_trades_per_day: 10,           // Moderate activity
            decision_interval_secs: 300,      // Check every 5 minutes
            enabled: true,
            initial_capital: 100_000.0,
        }
    }
}

impl Default for BotConfig {
    fn default() -> Self {
        Self::grandma()
    }
}
