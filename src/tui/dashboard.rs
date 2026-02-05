//! Dashboard view - System overview.

use crate::AppState;
use crossterm::event::KeyEvent;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Stylize,
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph},
    Frame,
};
use std::sync::Arc;

use super::Theme;

/// Render the dashboard view.
pub fn render(frame: &mut Frame, area: Rect, app_state: &Arc<AppState>, theme: &Theme) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10), // System info
            Constraint::Min(0),      // Price updates & stats
        ])
        .split(area);

    // Top section: System info
    render_system_info(frame, chunks[0], app_state, theme);

    // Bottom section: Split into prices and statistics
    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    render_price_updates(frame, bottom_chunks[0], app_state, theme);
    render_statistics(frame, bottom_chunks[1], app_state, theme);
}

/// Render system information.
fn render_system_info(frame: &mut Frame, area: Rect, app_state: &Arc<AppState>, theme: &Theme) {
    let inner_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(area);

    // Server status
    let server_info = vec![
        Line::from(vec![
            Span::styled("Status: ", theme.muted()),
            Span::styled("● Online", theme.success()),
        ]),
        Line::from(vec![
            Span::styled("Port: ", theme.muted()),
            Span::raw(app_state.config.port.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Redis: ", theme.muted()),
            Span::styled(
                if app_state.config.redis_url.is_some() {
                    "● Connected"
                } else {
                    "○ Disabled"
                },
                if app_state.config.redis_url.is_some() {
                    theme.success()
                } else {
                    theme.muted()
                },
            ),
        ]),
    ];

    let server_block = Paragraph::new(server_info)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("🖥  Server")
                .border_style(theme.border()),
        )
        .style(theme.info());

    frame.render_widget(server_block, inner_chunks[0]);

    // Sources status
    let sources = app_state.coordinator.source_status();
    let sources_text: Vec<Line> = sources
        .iter()
        .take(5)
        .map(|(name, active)| {
            Line::from(vec![
                Span::styled(
                    if *active { "● " } else { "○ " },
                    if *active { theme.success() } else { theme.muted() },
                ),
                Span::raw(*name),
            ])
        })
        .collect();

    let sources_block = Paragraph::new(sources_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("📡 Data Sources")
                .border_style(theme.border()),
        );

    frame.render_widget(sources_block, inner_chunks[1]);

    // Trading status
    let trading_info = vec![
        Line::from(vec![
            Span::styled("Paper Trading: ", theme.muted()),
            Span::styled(
                if app_state.bot_runner.is_some() {
                    "● Active"
                } else {
                    "○ Inactive"
                },
                if app_state.bot_runner.is_some() {
                    theme.success()
                } else {
                    theme.muted()
                },
            ),
        ]),
        Line::from(vec![
            Span::styled("Sync Service: ", theme.muted()),
            Span::styled(
                if app_state.sync_service.is_some() {
                    "● Enabled"
                } else {
                    "○ Disabled"
                },
                if app_state.sync_service.is_some() {
                    theme.success()
                } else {
                    theme.muted()
                },
            ),
        ]),
        Line::from(vec![
            Span::styled("Peer Mesh: ", theme.muted()),
            Span::styled(
                if app_state.peer_mesh.is_some() {
                    "● Connected"
                } else {
                    "○ Offline"
                },
                if app_state.peer_mesh.is_some() {
                    theme.success()
                } else {
                    theme.muted()
                },
            ),
        ]),
    ];

    let trading_block = Paragraph::new(trading_info)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("🤖 Trading")
                .border_style(theme.border()),
        );

    frame.render_widget(trading_block, inner_chunks[2]);
}

/// Render recent price updates.
fn render_price_updates(frame: &mut Frame, area: Rect, app_state: &Arc<AppState>, theme: &Theme) {
    let prices = app_state.price_cache.get_top_prices(10);
    
    let items: Vec<ListItem> = prices
        .iter()
        .map(|(symbol, price, change)| {
            let change_color = if *change >= 0.0 {
                theme.success()
            } else {
                theme.error()
            };
            
            let change_symbol = if *change >= 0.0 { "▲" } else { "▼" };
            
            ListItem::new(Line::from(vec![
                Span::styled(format!("{:>8}", symbol), theme.title()),
                Span::raw("  "),
                Span::styled(format!("${:>10.2}", price), theme.info()),
                Span::raw("  "),
                Span::styled(
                    format!("{} {:>6.2}%", change_symbol, change.abs()),
                    change_color,
                ),
            ]))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("💰 Price Updates")
            .border_style(theme.border()),
    );

    frame.render_widget(list, area);
}

/// Render statistics.
fn render_statistics(frame: &mut Frame, area: Rect, app_state: &Arc<AppState>, theme: &Theme) {
    let stats = app_state.price_cache.get_update_counts();
    let total: u64 = stats.values().sum();
    
    let lines = vec![
        Line::from(vec![
            Span::styled("Total Updates: ", theme.muted()),
            Span::styled(format!("{}", total), theme.title()),
        ]),
        Line::from(""),
        Line::from(Span::styled("Top Updated:", theme.header())),
    ];

    let mut all_lines = lines;
    
    let mut sorted_stats: Vec<_> = stats.iter().collect();
    sorted_stats.sort_by(|a, b| b.1.cmp(a.1));
    
    for (symbol, count) in sorted_stats.iter().take(8) {
        all_lines.push(Line::from(vec![
            Span::raw(format!("  {:>8}", symbol)),
            Span::styled(format!(" {:>8}", count), theme.success()),
        ]));
    }

    let block = Paragraph::new(all_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("📊 Statistics")
                .border_style(theme.border()),
        );

    frame.render_widget(block, area);
}

/// Handle keyboard events for dashboard.
pub fn handle_event(_key: &KeyEvent) {
    // Dashboard doesn't have interactive elements yet
}
