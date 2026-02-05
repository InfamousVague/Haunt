//! Bot activity view - Monitor trading bots.

use crate::AppState;
use crossterm::event::KeyEvent;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Stylize,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Row, Table},
    Frame,
};
use std::sync::Arc;

use super::Theme;

/// Render the bots view.
pub fn render(frame: &mut Frame, area: Rect, app_state: &Arc<AppState>, theme: &Theme) {
    if app_state.bot_runner.is_none() {
        render_bots_disabled(frame, area, theme);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),  // Bot status grid
            Constraint::Min(0),     // Recent trades
        ])
        .split(area);

    render_bot_status(frame, chunks[0], app_state, theme);
    render_recent_trades(frame, chunks[1], app_state, theme);
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
fn render_bot_status(frame: &mut Frame, area: Rect, _app_state: &Arc<AppState>, theme: &Theme) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(area);

    // Mock bot data (in real implementation, get from bot_runner)
    let bots = vec![
        ("Scalper", "Active", 12, 8, 67.0),
        ("Crypto Bro", "Active", 5, 3, 60.0),
        ("Grandma", "Paused", 3, 2, 67.0),
        ("Quant", "Active", 8, 7, 88.0),
    ];

    for (i, (name, status, trades, wins, win_rate)) in bots.iter().enumerate() {
        let status_color = if *status == "Active" {
            theme.success()
        } else {
            theme.warning()
        };

        let win_rate_color = if *win_rate >= 70.0 {
            theme.success()
        } else if *win_rate >= 50.0 {
            theme.warning()
        } else {
            theme.error()
        };

        let info = vec![
            Line::from(vec![
                Span::styled(format!("🤖 {}", name), theme.title()),
            ]),
            Line::from(vec![
                Span::styled("● ", status_color),
                Span::styled(*status, status_color),
            ]),
            Line::from(vec![
                Span::styled("Trades: ", theme.muted()),
                Span::raw(trades.to_string()),
            ]),
            Line::from(vec![
                Span::styled("Wins: ", theme.muted()),
                Span::raw(wins.to_string()),
            ]),
            Line::from(vec![
                Span::styled("Rate: ", theme.muted()),
                Span::styled(format!("{:.1}%", win_rate), win_rate_color),
            ]),
        ];

        let block = Paragraph::new(info)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme.border()),
            );

        frame.render_widget(block, chunks[i]);
    }
}

/// Render recent trades.
fn render_recent_trades(frame: &mut Frame, area: Rect, _app_state: &Arc<AppState>, theme: &Theme) {
    // Mock trade data
    let trades = vec![
        ("Scalper", "BTC-USD", "BUY", "42150.00", "0.025", "+125.50", "Win"),
        ("Quant", "ETH-USD", "SELL", "2245.30", "0.500", "+89.20", "Win"),
        ("CryptoBro", "BTC-USD", "BUY", "42080.00", "0.010", "-35.00", "Loss"),
        ("Scalper", "ETH-USD", "SELL", "2250.00", "0.250", "+67.80", "Win"),
        ("Quant", "SOL-USD", "BUY", "98.50", "5.000", "+45.00", "Win"),
        ("Grandma", "BTC-USD", "BUY", "41900.00", "0.050", "+250.00", "Win"),
        ("Scalper", "BTC-USD", "SELL", "42200.00", "0.030", "+78.90", "Win"),
        ("Quant", "ETH-USD", "BUY", "2240.00", "0.400", "-12.00", "Loss"),
        ("CryptoBro", "SOL-USD", "SELL", "99.20", "10.00", "+120.00", "Win"),
        ("Scalper", "BTC-USD", "BUY", "42100.00", "0.020", "+56.40", "Win"),
    ];

    let rows: Vec<Row> = trades
        .iter()
        .map(|(bot, pair, side, price, amount, pnl, result)| {
            let side_style = if *side == "BUY" {
                theme.success()
            } else {
                theme.error()
            };

            let pnl_style = if result == &"Win" {
                theme.success()
            } else {
                theme.error()
            };

            Row::new(vec![
                bot.to_string(),
                pair.to_string(),
                Span::styled(*side, side_style).to_string(),
                format!("${}", price),
                amount.to_string(),
                Span::styled(*pnl, pnl_style).to_string(),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
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
