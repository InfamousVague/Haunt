//! Database activity view - Monitor database operations.

use crate::AppState;
use crossterm::event::KeyEvent;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
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
    let text = vec![
        Line::from(""),
        Line::from(Span::styled("No database operations tracked yet.", theme.muted())),
        Line::from(Span::styled("Wire a DB operation feed to populate this view.", theme.muted())),
    ];

    let block = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("📝 Recent Operations")
                .border_style(theme.border()),
        )
        .centered();

    frame.render_widget(block, area);
}

/// Handle keyboard events for database view.
pub fn handle_event(_key: &KeyEvent) {
    // No interactive elements yet
}
