//! Bot API endpoints
//!
//! Endpoints for managing and monitoring AI trading bots.

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::services::paperbot::{BotPersonality, BotStatus};
use crate::AppState;

/// Response for listing all bots
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BotsListResponse {
    pub bots: Vec<BotStatus>,
    pub total: usize,
}

/// Response for a single bot
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BotResponse {
    pub bot: BotStatus,
}

/// Bot performance metrics
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BotPerformance {
    pub bot_id: String,
    pub name: String,
    pub personality: BotPersonality,
    pub total_trades: u64,
    pub winning_trades: u64,
    pub win_rate: f64,
    pub total_pnl: f64,
    pub portfolio_value: f64,
    pub return_pct: f64,
    pub sharpe_ratio: Option<f64>,
    pub max_drawdown: Option<f64>,
}

/// Request to follow/unfollow a bot
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowBotRequest {
    pub user_id: String,
    pub auto_copy: Option<bool>,
}

/// Response for follow action
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowResponse {
    pub success: bool,
    pub message: String,
}

/// Create bot API routes
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_bots))
        .route("/:bot_id", get(get_bot))
        .route("/:bot_id/performance", get(get_bot_performance))
        .route("/:bot_id/trades", get(get_bot_trades))
        .route("/:bot_id/follow", post(follow_bot))
        .route("/:bot_id/unfollow", post(unfollow_bot))
}

/// List all bots
///
/// GET /api/bots
async fn list_bots(State(state): State<AppState>) -> Result<Json<BotsListResponse>, AppError> {
    let bots = if let Some(ref runner) = state.bot_runner {
        runner.get_all_statuses()
    } else {
        vec![]
    };

    let total = bots.len();
    Ok(Json(BotsListResponse { bots, total }))
}

/// Get a specific bot
///
/// GET /api/bots/:bot_id
async fn get_bot(
    State(state): State<AppState>,
    Path(bot_id): Path<String>,
) -> Result<Json<BotResponse>, AppError> {
    let runner = state
        .bot_runner
        .as_ref()
        .ok_or_else(|| AppError::Internal("Bot runner not initialized".to_string()))?;

    let bot = runner
        .get_status(&bot_id)
        .ok_or_else(|| AppError::NotFound(format!("Bot not found: {}", bot_id)))?;

    Ok(Json(BotResponse { bot }))
}

/// Get bot performance metrics
///
/// GET /api/bots/:bot_id/performance
async fn get_bot_performance(
    State(state): State<AppState>,
    Path(bot_id): Path<String>,
) -> Result<Json<BotPerformance>, AppError> {
    let runner = state
        .bot_runner
        .as_ref()
        .ok_or_else(|| AppError::Internal("Bot runner not initialized".to_string()))?;

    let status = runner
        .get_status(&bot_id)
        .ok_or_else(|| AppError::NotFound(format!("Bot not found: {}", bot_id)))?;

    // Calculate performance metrics
    let win_rate = if status.total_trades > 0 {
        status.winning_trades as f64 / status.total_trades as f64
    } else {
        0.0
    };

    let initial_capital = 100_000.0; // From config
    let return_pct = if initial_capital > 0.0 {
        ((status.portfolio_value - initial_capital) / initial_capital) * 100.0
    } else {
        0.0
    };

    Ok(Json(BotPerformance {
        bot_id: status.id,
        name: status.name,
        personality: status.personality,
        total_trades: status.total_trades,
        winning_trades: status.winning_trades,
        win_rate,
        total_pnl: status.total_pnl,
        portfolio_value: status.portfolio_value,
        return_pct,
        sharpe_ratio: None, // TODO: Calculate from trade history
        max_drawdown: None, // TODO: Calculate from equity curve
    }))
}

/// Get bot's trade history
///
/// GET /api/bots/:bot_id/trades
async fn get_bot_trades(
    State(state): State<AppState>,
    Path(bot_id): Path<String>,
) -> Result<Json<Vec<crate::types::Trade>>, AppError> {
    let runner = state
        .bot_runner
        .as_ref()
        .ok_or_else(|| AppError::Internal("Bot runner not initialized".to_string()))?;

    // Verify bot exists
    let _status = runner
        .get_status(&bot_id)
        .ok_or_else(|| AppError::NotFound(format!("Bot not found: {}", bot_id)))?;

    // Get trades from the bot's portfolio
    let portfolio_id = format!("bot_{}", bot_id);
    let trades = state.trading_service.get_trades(&portfolio_id, 100);

    Ok(Json(trades))
}

/// Follow a bot's trades
///
/// POST /api/bots/:bot_id/follow
async fn follow_bot(
    State(state): State<AppState>,
    Path(bot_id): Path<String>,
    Json(request): Json<FollowBotRequest>,
) -> Result<Json<FollowResponse>, AppError> {
    let runner = state
        .bot_runner
        .as_ref()
        .ok_or_else(|| AppError::Internal("Bot runner not initialized".to_string()))?;

    // Verify bot exists
    let status = runner
        .get_status(&bot_id)
        .ok_or_else(|| AppError::NotFound(format!("Bot not found: {}", bot_id)))?;

    // TODO: Store follow relationship in database
    // For now, just return success
    let message = if request.auto_copy.unwrap_or(false) {
        format!(
            "Now following {} with auto-copy enabled. Your trades will mirror the bot's trades.",
            status.name
        )
    } else {
        format!(
            "Now following {}. You'll receive notifications when the bot trades.",
            status.name
        )
    };

    Ok(Json(FollowResponse {
        success: true,
        message,
    }))
}

/// Unfollow a bot
///
/// POST /api/bots/:bot_id/unfollow
async fn unfollow_bot(
    State(state): State<AppState>,
    Path(bot_id): Path<String>,
    Json(_request): Json<FollowBotRequest>,
) -> Result<Json<FollowResponse>, AppError> {
    let runner = state
        .bot_runner
        .as_ref()
        .ok_or_else(|| AppError::Internal("Bot runner not initialized".to_string()))?;

    // Verify bot exists
    let status = runner
        .get_status(&bot_id)
        .ok_or_else(|| AppError::NotFound(format!("Bot not found: {}", bot_id)))?;

    // TODO: Remove follow relationship from database

    Ok(Json(FollowResponse {
        success: true,
        message: format!("Unfollowed {}", status.name),
    }))
}
