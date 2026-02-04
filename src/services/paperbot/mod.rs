//! PaperBot Trading Agents
//!
//! AI-powered trading bots that compete on the leaderboard and provide
//! users with bots to trade against and follow.

pub mod config;
pub mod decision;
pub mod grandma;
pub mod runner;

pub use config::{BotConfig, BotPersonality};
pub use decision::{DecisionContext, SellReason, SignalStrength, TradeDecision, TradeSignal};
pub use grandma::GrandmaBot;
pub use runner::{BotRunner, BotStatus};

use crate::error::AppError;
use crate::types::AssetClass;
use std::future::Future;
use std::pin::Pin;

/// Core trait that all trading bots must implement
pub trait TradingBot: Send + Sync {
    /// Returns the bot's unique name
    fn name(&self) -> &str;

    /// Returns the bot's personality type
    fn personality(&self) -> BotPersonality;

    /// Returns the bot's configuration
    fn config(&self) -> &BotConfig;

    /// Returns which asset classes this bot trades
    fn supported_asset_classes(&self) -> Vec<AssetClass>;

    /// Analyze the market and decide whether to trade
    fn analyze<'a>(
        &'a self,
        ctx: &'a DecisionContext,
    ) -> Pin<Box<dyn Future<Output = Result<TradeDecision, AppError>> + Send + 'a>>;

    /// Called after a trade is executed to update internal state
    fn on_trade_executed<'a>(
        &'a self,
        symbol: &'a str,
        decision: &'a TradeDecision,
        execution_price: f64,
    ) -> Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + 'a>>;

    /// Called periodically to update any internal state (e.g., learning)
    fn tick(&self) -> Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    /// Get the bot's current state for persistence
    fn get_state(&self) -> serde_json::Value {
        serde_json::json!({})
    }

    /// Restore the bot's state from persistence
    fn restore_state(&mut self, _state: serde_json::Value) -> Result<(), AppError> {
        Ok(())
    }
}
