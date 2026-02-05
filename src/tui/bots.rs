//! Bot activity view - Monitor trading bots.

use crate::AppState;
use crossterm::event::KeyEvent;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table},
    Frame,
};
use std::sync::Arc;

use super::{Theme, TuiState};

/// Render the bots view.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    app_state: &Arc<AppState>,
    theme: &Theme,
    tui_state: &Arc<TuiState>,
) {
    if app_state.bot_runner.is_none() {
        render_bots_disabled(frame, area, theme);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10), // Bot status table
            Constraint::Min(0),     // Recent trades
        ])
        .split(area);

    render_bot_status(frame, chunks[0], app_state, theme);
    render_recent_trades(frame, chunks[1], app_state, theme, tui_state);
}

/// Render message when bots are disabled.
fn render_bots_disabled(frame: &mut Frame, area: Rect, theme: &Theme) {
    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "🤖 Trading Bots Disabled",
            theme.title(),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("Trading bots are not currently running.", theme.muted()),
        ]),
        Line::from(vec![
            Span::styled(
                "Enable them in the configuration to see activity here.",
                theme.muted(),
            ),
        ]),
    ];

    let block = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Bot Activity")
                .border_style(theme.border()),
        )
        .centered();

    frame.render_widget(block, area);
}

/// Render bot status.
fn render_bot_status(frame: &mut Frame, area: Rect, app_state: &Arc<AppState>, theme: &Theme) {
    let Some(runner) = app_state.bot_runner.as_ref() else {
        return;
    };

    let mut statuses = runner.get_all_statuses();
    statuses.sort_by(|a, b| a.name.cmp(&b.name));

    let rows: Vec<Row> = statuses
        .iter()
        .map(|status| {
            let running_style = if status.running {
                theme.success()
            } else {
                theme.warning()
            };

            let win_rate = if status.total_trades > 0 {
                (status.winning_trades as f64 / status.total_trades as f64) * 100.0
            } else {
                0.0
            };

            let win_rate_style = if win_rate >= 70.0 {
                theme.success()
            } else if win_rate >= 50.0 {
                theme.warning()
            } else {
                theme.error()
            };

            let pnl_style = if status.total_pnl >= 0.0 {
                theme.success()
            } else {
                theme.error()
            };

            Row::new(vec![
                format!("{}", status.name),
                Span::styled(
                    if status.running { "Active" } else { "Paused" },
                    running_style,
                )
                .to_string(),
                status.total_trades.to_string(),
                status.winning_trades.to_string(),
                Span::styled(format!("{:.1}%", win_rate), win_rate_style).to_string(),
                Span::styled(format!("{:+.2}", status.total_pnl), pnl_style).to_string(),
                format!("{:.2}", status.portfolio_value),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(14),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(9),
            Constraint::Length(12),
            Constraint::Length(14),
        ],
    )
    .header(
        Row::new(vec!["Bot", "Status", "Trades", "Wins", "WinRate", "Total PnL", "Portfolio"])
            .style(theme.header())
            .bottom_margin(1),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("🤖 Bot Performance")
            .border_style(theme.border()),
    );

    frame.render_widget(table, area);
}

/// Render recent trades.
fn render_recent_trades(
    frame: &mut Frame,
    area: Rect,
    app_state: &Arc<AppState>,
    theme: &Theme,
    tui_state: &Arc<TuiState>,
) {
    let trades = tui_state.recent_trades(20);
    if trades.is_empty() {
        let text = vec![
            Line::from(""),
            Line::from(Span::styled("No trades yet.", theme.muted())),
        ];

        let block = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("💸 Recent Trades")
                    .border_style(theme.border()),
            )
            .centered();

        frame.render_widget(block, area);
        return;
    }

    let mut portfolio_to_bot = std::collections::HashMap::new();
    if let Some(runner) = app_state.bot_runner.as_ref() {
        for status in runner.get_all_statuses() {
            if let Some(portfolio_id) = status.portfolio_id.clone() {
                portfolio_to_bot.insert(portfolio_id, status.name);
            }
        }
    }

    let rows: Vec<Row> = trades
        .iter()
        .rev()
        .map(|trade| {
            let side_style = if trade.side == crate::types::OrderSide::Buy {
                theme.success()
            } else {
                theme.error()
            };

            let pnl_value = trade.realized_pnl.unwrap_or(0.0);
            let pnl_style = if pnl_value >= 0.0 {
                theme.success()
            } else {
                theme.error()
            };

            let bot_name = portfolio_to_bot
                .get(&trade.portfolio_id)
                .cloned()
                .unwrap_or_else(|| trade.portfolio_id.clone());

            Row::new(vec![
                bot_name,
                trade.symbol.clone(),
                Span::styled(trade.side.to_string(), side_style).to_string(),
                format!("{:.2}", trade.price),
                format!("{:.4}", trade.quantity),
                Span::styled(format!("{:+.2}", pnl_value), pnl_style).to_string(),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(14),
            Constraint::Length(10),
            Constraint::Length(6),
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Length(12),
        ],
    )
    .header(
        Row::new(vec!["Bot", "Pair", "Side", "Price", "Amount", "P&L"])
            .style(theme.header())
            .bottom_margin(1),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("💸 Recent Trades")
            .border_style(theme.border()),
    );

    frame.render_widget(table, area);
}

/// Handle keyboard events for bots view.
pub fn handle_event(_key: &KeyEvent) {
    // No interactive elements yet
}
