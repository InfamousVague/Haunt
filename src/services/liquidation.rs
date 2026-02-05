//! Liquidation Engine
//!
//! Monitors positions for margin level warnings and executes liquidations when necessary.
//! Features:
//! - Real-time margin monitoring
//! - Gradual warning system (80%, 90%, 95%)
//! - Partial and full liquidation execution
//! - Insurance fund management
//! - ADL (Auto-Deleverage) system when insurance fund is insufficient

#![allow(dead_code)]

use crate::services::SqliteStore;
use crate::types::{
    AdlEntry, AssetClass, FundingPayment, FundingRate, InsuranceFund, Liquidation,
    LiquidationAlertData, LiquidationWarningLevel, MarginChangeType, MarginHistory,
    MarginWarningData, Position, PositionSide, PositionUpdateData, PositionUpdateType,
    ServerMessage,
};
use crate::websocket::RoomManager;
use dashmap::DashMap;
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, info, warn};

/// Liquidation engine errors.
#[derive(Debug, Error)]
pub enum LiquidationError {
    #[error("Position not found: {0}")]
    PositionNotFound(String),
    #[error("Portfolio not found: {0}")]
    PortfolioNotFound(String),
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Insufficient insurance fund")]
    InsufficientInsuranceFund,
}

/// Liquidation engine for monitoring and executing position liquidations.
pub struct LiquidationEngine {
    store: Arc<SqliteStore>,
    room_manager: Arc<RoomManager>,
    /// Cache of current funding rates by symbol
    funding_rates: DashMap<String, FundingRate>,
    /// Cache of positions being monitored (position_id -> last warning level sent)
    warning_state: DashMap<String, Option<LiquidationWarningLevel>>,
}

impl LiquidationEngine {
    /// Create a new liquidation engine.
    pub fn new(store: Arc<SqliteStore>, room_manager: Arc<RoomManager>) -> Self {
        Self {
            store,
            room_manager,
            funding_rates: DashMap::new(),
            warning_state: DashMap::new(),
        }
    }

    // ========== Margin Monitoring ==========

    /// Check all positions in a portfolio for margin warnings and liquidations.
    /// Returns a list of positions that were liquidated.
    pub fn check_portfolio_margins(&self, portfolio_id: &str) -> Vec<Liquidation> {
        let positions = self.store.get_portfolio_positions(portfolio_id);
        let mut liquidations = Vec::new();

        for position in positions {
            if position.leverage <= 1.0 {
                continue; // No liquidation for unleveraged positions
            }

            // Check for warning or liquidation
            if let Some(warning_level) = position.warning_level() {
                match warning_level {
                    LiquidationWarningLevel::Liquidation => {
                        // Execute liquidation
                        if let Ok(liq) = self.execute_liquidation(&position) {
                            liquidations.push(liq);
                        }
                    }
                    _ => {
                        // Send warning if not already sent
                        self.send_margin_warning(&position, warning_level);
                    }
                }
            } else {
                // Clear warning state if margin is now healthy
                self.warning_state.remove(&position.id);
            }
        }

        liquidations
    }

    /// Check a single position for margin warnings and liquidation.
    pub fn check_position_margin(&self, position: &Position) -> Option<Liquidation> {
        if position.leverage <= 1.0 {
            return None;
        }

        if let Some(warning_level) = position.warning_level() {
            match warning_level {
                LiquidationWarningLevel::Liquidation => {
                    self.execute_liquidation(position).ok()
                }
                _ => {
                    self.send_margin_warning(position, warning_level);
                    None
                }
            }
        } else {
            self.warning_state.remove(&position.id);
            None
        }
    }

    /// Send a margin warning for a position if not already sent at this level.
    fn send_margin_warning(&self, position: &Position, level: LiquidationWarningLevel) {
        // Check if we already sent this warning level
        let should_send = match self.warning_state.get(&position.id) {
            Some(ref current) => {
                // Only send if this is a more severe warning
                match (*current.value(), level) {
                    (None, _) => true,
                    (Some(LiquidationWarningLevel::Warning80), LiquidationWarningLevel::Warning90 | LiquidationWarningLevel::Warning95) => true,
                    (Some(LiquidationWarningLevel::Warning90), LiquidationWarningLevel::Warning95) => true,
                    _ => false,
                }
            }
            None => true,
        };

        if should_send {
            // Update warning state
            self.warning_state.insert(position.id.clone(), Some(level));

            // Send WebSocket warning
            let warning_data = MarginWarningData {
                portfolio_id: position.portfolio_id.clone(),
                margin_level: position.margin_level(),
                warning_level: level.margin_level_threshold(),
                at_risk_positions: vec![position.id.clone()],
                timestamp: chrono::Utc::now().timestamp_millis(),
            };

            let message = ServerMessage::MarginWarning { data: warning_data };
            if let Ok(json) = serde_json::to_string(&message) {
                self.room_manager.broadcast_trading(&position.portfolio_id, &json);
            }

            info!(
                "Margin warning {:?} for position {} (margin level: {:.2}%)",
                level, position.id, position.margin_level()
            );
        }
    }

    // ========== Liquidation Execution ==========

    /// Execute liquidation of a position.
    pub fn execute_liquidation(&self, position: &Position) -> Result<Liquidation, LiquidationError> {
        let portfolio = self
            .store
            .get_portfolio(&position.portfolio_id)
            .ok_or_else(|| LiquidationError::PortfolioNotFound(position.portfolio_id.clone()))?;

        // Determine if partial or full liquidation
        // For now, we do full liquidation when margin level < 100%
        let is_partial = false;
        let quantity = position.quantity;
        let remaining_quantity = None;

        // Get mark price (use current price for simulation)
        let mark_price = position.current_price;
        let liquidation_price = position.liquidation_price.unwrap_or(position.current_price);

        // Create liquidation record
        let liquidation = Liquidation::new(
            position.id.clone(),
            position.portfolio_id.clone(),
            position.symbol.clone(),
            quantity,
            liquidation_price,
            mark_price,
            position.entry_price,
            position.side,
            is_partial,
            remaining_quantity,
        );

        // Save liquidation to database
        self.store
            .create_liquidation(&liquidation)
            .map_err(|e| LiquidationError::DatabaseError(e.to_string()))?;

        // Add liquidation fee to insurance fund
        self.store
            .add_insurance_contribution(liquidation.liquidation_fee)
            .map_err(|e| LiquidationError::DatabaseError(e.to_string()))?;

        // Try to cover loss from insurance fund
        let loss = liquidation.loss;
        let covered = self
            .store
            .cover_loss_from_insurance(loss)
            .map_err(|e| LiquidationError::DatabaseError(e.to_string()))?;

        // If insurance fund couldn't cover the full loss, trigger ADL
        if covered < loss {
            let uncovered = loss - covered;
            warn!(
                "Insurance fund insufficient. Uncovered loss: {}. Triggering ADL.",
                uncovered
            );
            // ADL would be triggered here (not implemented yet)
        }

        // Create margin history entry
        let margin_history = MarginHistory::new(
            position.portfolio_id.clone(),
            Some(position.id.clone()),
            MarginChangeType::Liquidation,
            position.margin_level(),
            0.0, // Position closed
            portfolio.margin_used,
            portfolio.margin_used - position.margin_used,
            Some(format!("Liquidation at price {}", liquidation_price)),
        );

        self.store
            .create_margin_history(&margin_history)
            .map_err(|e| LiquidationError::DatabaseError(e.to_string()))?;

        // Broadcast liquidation alert via WebSocket
        let alert_data = LiquidationAlertData {
            portfolio_id: position.portfolio_id.clone(),
            position_id: position.id.clone(),
            symbol: position.symbol.clone(),
            liquidation_price: liquidation.liquidation_price,
            loss_amount: liquidation.loss,
            timestamp: chrono::Utc::now().timestamp_millis(),
        };

        let message = ServerMessage::LiquidationAlert { data: alert_data };
        if let Ok(json) = serde_json::to_string(&message) {
            self.room_manager.broadcast_trading(&position.portfolio_id, &json);
        }

        // Create a closed position for the update
        let mut closed_position = position.clone();
        closed_position.quantity = 0.0;
        closed_position.unrealized_pnl = 0.0;
        closed_position.unrealized_pnl_pct = 0.0;
        closed_position.realized_pnl = -liquidation.loss;
        closed_position.margin_used = 0.0;

        // Broadcast position update (closed due to liquidation)
        let position_update = PositionUpdateData {
            position: closed_position,
            update_type: PositionUpdateType::Liquidated,
            timestamp: chrono::Utc::now().timestamp_millis(),
        };

        let pos_message = ServerMessage::PositionUpdate { data: position_update };
        if let Ok(json) = serde_json::to_string(&pos_message) {
            self.room_manager.broadcast_trading(&position.portfolio_id, &json);
        }

        // Clear warning state
        self.warning_state.remove(&position.id);

        info!(
            "Liquidated position {} at price {} (loss: {}, fee: {})",
            position.id, liquidation_price, liquidation.loss, liquidation.liquidation_fee
        );

        Ok(liquidation)
    }

    /// Execute partial liquidation to reduce position size and bring margin level back to safe.
    pub fn execute_partial_liquidation(
        &self,
        position: &Position,
        reduce_by_percent: f64,
    ) -> Result<Liquidation, LiquidationError> {
        let quantity = position.quantity * reduce_by_percent;
        let remaining_quantity = Some(position.quantity - quantity);

        let mark_price = position.current_price;
        let liquidation_price = position.liquidation_price.unwrap_or(position.current_price);

        let liquidation = Liquidation::new(
            position.id.clone(),
            position.portfolio_id.clone(),
            position.symbol.clone(),
            quantity,
            liquidation_price,
            mark_price,
            position.entry_price,
            position.side,
            true,
            remaining_quantity,
        );

        self.store
            .create_liquidation(&liquidation)
            .map_err(|e| LiquidationError::DatabaseError(e.to_string()))?;

        self.store
            .add_insurance_contribution(liquidation.liquidation_fee)
            .map_err(|e| LiquidationError::DatabaseError(e.to_string()))?;

        let covered = self
            .store
            .cover_loss_from_insurance(liquidation.loss)
            .map_err(|e| LiquidationError::DatabaseError(e.to_string()))?;

        if covered < liquidation.loss {
            warn!(
                "Insurance fund partially covered loss. Covered: {}, Total: {}",
                covered, liquidation.loss
            );
        }

        info!(
            "Partial liquidation of position {}: reduced {} ({:.1}%)",
            position.id, quantity, reduce_by_percent * 100.0
        );

        Ok(liquidation)
    }

    // ========== ADL (Auto-Deleverage) ==========

    /// Get ADL priority queue for a symbol and side.
    /// Returns positions sorted by ADL score (highest first).
    pub fn get_adl_queue(&self, _symbol: &str, side: PositionSide) -> Vec<AdlEntry> {
        // Get all positions for this symbol with opposite side
        // (if we're liquidating a long, we need to deleverage shorts, and vice versa)
        let _opposite_side = match side {
            PositionSide::Long => PositionSide::Short,
            PositionSide::Short => PositionSide::Long,
        };

        // This would need to query all portfolios - for now, return empty
        // In a real implementation, we'd have a global position index
        Vec::new()
    }

    /// Execute ADL to cover losses when insurance fund is insufficient.
    pub fn execute_adl(
        &self,
        _symbol: &str,
        _side: PositionSide,
        _amount_to_cover: f64,
    ) -> Result<Vec<AdlEntry>, LiquidationError> {
        // ADL implementation would:
        // 1. Get the ADL queue for the symbol/side
        // 2. Starting from highest score, reduce positions to cover the loss
        // 3. Compensate deleveraged users at mark price
        // This is a complex feature - placeholder for now
        warn!("ADL not implemented - loss may not be fully covered");
        Ok(Vec::new())
    }

    // ========== Funding Rate Management ==========

    /// Update funding rate for a symbol.
    pub fn update_funding_rate(&self, funding_rate: FundingRate) {
        debug!(
            "Updated funding rate for {}: {:.6}%",
            funding_rate.symbol,
            funding_rate.rate * 100.0
        );
        self.funding_rates.insert(funding_rate.symbol.clone(), funding_rate);
    }

    /// Get current funding rate for a symbol.
    pub fn get_funding_rate(&self, symbol: &str) -> Option<FundingRate> {
        self.funding_rates.get(symbol).map(|r| r.clone())
    }

    /// Apply funding payments to all perp positions for a symbol.
    /// This should be called at funding intervals (every 8 hours).
    pub fn apply_funding_payments(&self, symbol: &str) -> Vec<FundingPayment> {
        let funding_rate = match self.get_funding_rate(symbol) {
            Some(rate) => rate,
            None => {
                debug!("No funding rate for symbol {}", symbol);
                return Vec::new();
            }
        };

        if !funding_rate.should_apply_funding() {
            return Vec::new();
        }

        let payments = Vec::new();

        // Get all open perp positions for this symbol
        // This would need to iterate through all portfolios
        // For now, this is a placeholder that returns empty
        // In production, we'd have an index of positions by symbol

        debug!(
            "Applied funding payments for {}: {} payments",
            symbol,
            payments.len()
        );
        payments
    }

    /// Apply funding to a specific position.
    pub fn apply_funding_to_position(
        &self,
        position: &mut Position,
    ) -> Result<FundingPayment, LiquidationError> {
        if position.asset_class != AssetClass::Perp {
            return Err(LiquidationError::PositionNotFound(
                "Not a perp position".to_string(),
            ));
        }

        let funding_rate = self
            .get_funding_rate(&position.symbol)
            .ok_or_else(|| LiquidationError::PositionNotFound(format!("No funding rate for {}", position.symbol)))?;

        let payment = FundingPayment::new(
            position.id.clone(),
            position.portfolio_id.clone(),
            position.symbol.clone(),
            position.notional_value(),
            position.side,
            funding_rate.rate,
        );

        // Apply to position
        position.apply_funding(payment.payment);

        // Save payment to database
        self.store
            .create_funding_payment(&payment)
            .map_err(|e| LiquidationError::DatabaseError(e.to_string()))?;

        debug!(
            "Applied funding payment {} to position {}: {}",
            payment.id, position.id, payment.payment
        );

        Ok(payment)
    }

    // ========== Insurance Fund ==========

    /// Get current insurance fund state.
    pub fn get_insurance_fund(&self) -> InsuranceFund {
        self.store.get_insurance_fund()
    }

    /// Check if insurance fund can cover a potential loss.
    pub fn can_cover_loss(&self, loss: f64) -> bool {
        self.store.get_insurance_fund().can_cover(loss)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_engine() -> LiquidationEngine {
        let store = Arc::new(SqliteStore::new_in_memory().unwrap());
        let room_manager = RoomManager::new(); // Already returns Arc<RoomManager>
        LiquidationEngine::new(store, room_manager)
    }

    #[test]
    fn test_engine_creation() {
        let engine = setup_test_engine();
        assert!(engine.funding_rates.is_empty());
        assert!(engine.warning_state.is_empty());
    }

    #[test]
    fn test_funding_rate_update() {
        let engine = setup_test_engine();
        let rate = FundingRate::new("BTC-PERP".to_string(), 0.0001, 50000.0, 50010.0);

        engine.update_funding_rate(rate.clone());

        let retrieved = engine.get_funding_rate("BTC-PERP");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().rate, 0.0001);
    }

    #[test]
    fn test_insurance_fund_operations() {
        let engine = setup_test_engine();

        // Initially empty
        let fund = engine.get_insurance_fund();
        assert_eq!(fund.balance, 0.0);

        // Add contribution
        engine.store.add_insurance_contribution(1000.0).unwrap();
        let fund = engine.get_insurance_fund();
        assert_eq!(fund.balance, 1000.0);

        // Check can cover
        assert!(engine.can_cover_loss(500.0));
        assert!(!engine.can_cover_loss(1500.0));
    }
}
