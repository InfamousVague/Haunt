//! Logs view - System logs and events.

use crate::AppState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Stylize,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use std::sync::Arc;

use super::{events, Theme};

/// Render the logs view.
pub fn render(frame: &mut Frame, area: Rect, _app_state: &Arc<AppState>, theme: &Theme) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),  // Log controls
            Constraint::Min(0),     // Log output
        ])
        .split(area);

    render_log_controls(frame, chunks[0], theme);
    render_log_output(frame, chunks[1], theme);
}

/// Render log controls.
fn render_log_controls(frame: &mut Frame, area: Rect, theme: &Theme) {
    let text = vec![
        Line::from(vec![
            Span::styled("Filter: ", theme.muted()),
            Span::styled("[A]", theme.info()),
            Span::raw(" All  "),
            Span::styled("[E]", theme.error()),
            Span::raw(" Error  "),
            Span::styled("[W]", theme.warning()),
            Span::raw(" Warn  "),
            Span::styled("[I]", theme.info()),
            Span::raw(" Info  "),
            Span::styled("[D]", theme.muted()),
            Span::raw(" Debug"),
        ]),
        Line::from(vec![
            Span::styled("Actions: ", theme.muted()),
            Span::styled("[C]", theme.info()),
            Span::raw(" Clear  "),
            Span::styled("[P]", theme.info()),
            Span::raw(" Pause/Resume"),
        ]),
    ];

    let block = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("📋 Log Controls")
                .border_style(theme.border()),
        );

    frame.render_widget(block, area);
}

/// Render log output.
fn render_log_output(frame: &mut Frame, area: Rect, theme: &Theme) {
    // Mock log entries
    let logs = vec![
        ("INFO", "haunt::api", "Server started on 0.0.0.0:3000"),
        ("DEBUG", "haunt::sources", "Connected to CoinGecko WebSocket"),
        ("INFO", "haunt::services", "Price cache loaded 1,234 symbols from Redis"),
        ("DEBUG", "haunt::websocket", "Client connected: room=BTC-USD"),
        ("INFO", "haunt::services", "Sync service initialized"),
        ("WARN", "haunt::sources", "CoinCap rate limit exceeded, backing off"),
        ("DEBUG", "haunt::api", "GET /api/prices/BTC - 200 OK (2ms)"),
        ("INFO", "haunt::services", "Bot runner started with 4 strategies"),
        ("DEBUG", "haunt::services", "Scalper bot opened position: BTC-USD"),
        ("INFO", "haunt::services", "Chart store synced to Redis (234 sparklines)"),
        ("DEBUG", "haunt::websocket", "Broadcasting price update to 5 clients"),
        ("ERROR", "haunt::sources", "Failed to connect to Finnhub: connection timeout"),
        ("INFO", "haunt::services", "Peer mesh connected to 3 nodes"),
        ("DEBUG", "haunt::services", "Sync: replicated 12 entities to sapporo"),
        ("WARN", "haunt::services", "Order book depth below threshold for ETH-USD"),
        ("INFO", "haunt::api", "POST /api/signals - Signal saved successfully"),
        ("DEBUG", "haunt::services", "Grandma bot: analyzing 1h candles"),
        ("INFO", "haunt::services", "Historical data updated: 500 new candles"),
        ("DEBUG", "haunt::websocket", "Client disconnected: session_id=abc123"),
        ("INFO", "haunt::services", "Database checkpoint completed (2.1MB)"),
    ];

    let items: Vec<ListItem> = logs
        .iter()
        .rev()
        .map(|(level, module, message)| {
            let (level_style, level_icon) = match *level {
                "ERROR" => (theme.error(), "✗"),
                "WARN" => (theme.warning(), "⚠"),
                "INFO" => (theme.success(), "●"),
                "DEBUG" => (theme.muted(), "○"),
                _ => (theme.info(), "·"),
            };

            ListItem::new(Line::from(vec![
                Span::styled(level_icon, level_style),
                Span::raw(" "),
                Span::styled(format!("{:5}", level), level_style),
                Span::raw(" "),
                Span::styled(format!("{:30}", module), theme.info()),
                Span::raw(" "),
                Span::raw(*message),
            ]))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("📝 System Logs (Live)")
            .border_style(theme.border()),
    );

    frame.render_widget(list, area);
}

/// Handle keyboard events for logs view.
pub fn handle_event(key: &KeyEvent) {
    // Handle log filtering and controls
    match key.code {
        KeyCode::Char('a') | KeyCode::Char('A') => {
            // Show all logs
        }
        KeyCode::Char('e') | KeyCode::Char('E') => {
            // Show only errors
        }
        KeyCode::Char('w') | KeyCode::Char('W') => {
            // Show only warnings
        }
        KeyCode::Char('i') | KeyCode::Char('I') => {
            // Show only info
        }
        KeyCode::Char('d') | KeyCode::Char('D') => {
            // Show only debug
        }
        KeyCode::Char('c') | KeyCode::Char('C') => {
            // Clear logs
        }
        KeyCode::Char('p') | KeyCode::Char('P') => {
            // Pause/resume
        }
        _ => {}
    }
}
