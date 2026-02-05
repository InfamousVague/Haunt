//! Database activity view - Monitor database operations.

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

/// Render the database view.
pub fn render(frame: &mut Frame, area: Rect, app_state: &Arc<AppState>, theme: &Theme) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),  // DB status
            Constraint::Min(0),     // Recent operations
        ])
        .split(area);

    render_db_status(frame, chunks[0], app_state, theme);
    render_recent_operations(frame, chunks[1], app_state, theme);
}

/// Render database status.
fn render_db_status(frame: &mut Frame, area: Rect, app_state: &Arc<AppState>, theme: &Theme) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // SQLite info
    let sqlite_info = vec![
        Line::from(vec![
            Span::styled("Store: ", theme.muted()),
            Span::styled("● SQLite", theme.success()),
        ]),
        Line::from(vec![
            Span::styled("Path: ", theme.muted()),
            Span::raw(&app_state.sqlite_store.db_path),
        ]),
        Line::from(vec![
            Span::styled("Status: ", theme.muted()),
            Span::styled("Connected", theme.success()),
        ]),
    ];

    let sqlite_block = Paragraph::new(sqlite_info)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("🗄️  SQLite")
                .border_style(theme.border()),
        );

    frame.render_widget(sqlite_block, chunks[0]);

    // Redis info (if available)
    let redis_info = if app_state.config.redis_url.is_some() {
        vec![
            Line::from(vec![
                Span::styled("Cache: ", theme.muted()),
                Span::styled("● Redis", theme.success()),
            ]),
            Line::from(vec![
                Span::styled("Charts: ", theme.muted()),
                Span::styled("Synced", theme.info()),
            ]),
            Line::from(vec![
                Span::styled("Prices: ", theme.muted()),
                Span::styled("Cached", theme.info()),
            ]),
        ]
    } else {
        vec![
            Line::from(vec![
                Span::styled("Cache: ", theme.muted()),
                Span::styled("○ Disabled", theme.muted()),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Status: ", theme.muted()),
                Span::raw("In-memory only"),
            ]),
        ]
    };

    let redis_block = Paragraph::new(redis_info)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("⚡ Redis")
                .border_style(theme.border()),
        );

    frame.render_widget(redis_block, chunks[1]);
}

/// Render recent database operations.
fn render_recent_operations(frame: &mut Frame, area: Rect, _app_state: &Arc<AppState>, theme: &Theme) {
    // Mock recent operations for now
    // In a real implementation, you'd track these in the app state
    let operations = vec![
        ("WRITE", "signals", "bullish_signal_BTC", "2ms", "success"),
        ("READ", "prices", "ETH_price_cache", "1ms", "success"),
        ("WRITE", "trades", "paper_trade_001", "3ms", "success"),
        ("READ", "orderbook", "BTC-USD", "1ms", "success"),
        ("WRITE", "sync", "peer_update_osaka", "5ms", "success"),
        ("READ", "historical", "BTC_1h_candles", "12ms", "success"),
        ("WRITE", "chart_store", "sparkline_ETH", "2ms", "success"),
        ("READ", "signals", "latest_signals", "1ms", "success"),
        ("WRITE", "metrics", "node_metrics", "1ms", "success"),
        ("READ", "auth", "api_key_verify", "1ms", "success"),
    ];

    let rows: Vec<Row> = operations
        .iter()
        .map(|(op, table, key, time, status)| {
            let op_style = if *op == "WRITE" {
                theme.warning()
            } else {
                theme.info()
            };

            let status_style = if *status == "success" {
                theme.success()
            } else {
                theme.error()
            };

            Row::new(vec![
                Span::styled(*op, op_style).to_string(),
                table.to_string(),
                key.to_string(),
                time.to_string(),
                Span::styled(*status, status_style).to_string(),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(6),
            Constraint::Length(15),
            Constraint::Min(20),
            Constraint::Length(8),
            Constraint::Length(10),
        ],
    )
    .header(
        Row::new(vec!["Op", "Table", "Key", "Time", "Status"])
            .style(theme.header())
            .bottom_margin(1),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("📝 Recent Operations")
            .border_style(theme.border()),
    );

    frame.render_widget(table, area);
}

/// Handle keyboard events for database view.
pub fn handle_event(_key: &KeyEvent) {
    // No interactive elements yet
}
