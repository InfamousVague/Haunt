//! Peer network view - Monitor mesh network.

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

/// Render the network view.
pub fn render(frame: &mut Frame, area: Rect, app_state: &Arc<AppState>, theme: &Theme) {
    if app_state.peer_mesh.is_none() {
        render_network_disabled(frame, area, theme);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),  // Network status
            Constraint::Min(0),     // Peer list and sync status
        ])
        .split(area);

    render_network_status(frame, chunks[0], app_state, theme);

    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    render_peer_list(frame, bottom_chunks[0], app_state, theme);
    render_sync_activity(frame, bottom_chunks[1], app_state, theme);
}

/// Render message when network is disabled.
fn render_network_disabled(frame: &mut Frame, area: Rect, theme: &Theme) {
    let text = vec![
        Line::from(""),
        Line::from(Span::styled("🌐 Peer Network Offline", theme.title())),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Peer mesh network is not configured.",
            theme.muted(),
        )]),
        Line::from(vec![Span::styled(
            "Enable mesh networking to see peer activity.",
            theme.muted(),
        )]),
    ];

    let block = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Peer Network")
                .border_style(theme.border()),
        )
        .centered();

    frame.render_widget(block, area);
}

/// Render network status.
fn render_network_status(
    frame: &mut Frame,
    area: Rect,
    app_state: &Arc<AppState>,
    theme: &Theme,
) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(area);

    // Node info
    let is_primary = app_state
        .sync_service
        .as_ref()
        .map(|s| s.is_primary())
        .unwrap_or(false);

    let node_info = vec![
        Line::from(vec![
            Span::styled("Role: ", theme.muted()),
            Span::styled(
                if is_primary { "Primary (Osaka)" } else { "Secondary" },
                if is_primary { theme.warning() } else { theme.info() },
            ),
        ]),
        Line::from(vec![
            Span::styled("Status: ", theme.muted()),
            Span::styled("● Connected", theme.success()),
        ]),
        Line::from(vec![
            Span::styled("Uptime: ", theme.muted()),
            Span::raw("12h 34m"),
        ]),
    ];

    let node_block = Paragraph::new(node_info)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("🖥  This Node")
                .border_style(theme.border()),
        );

    frame.render_widget(node_block, chunks[0]);

    // Mesh info
    let mesh_info = vec![
        Line::from(vec![
            Span::styled("Peers: ", theme.muted()),
            Span::styled("3 connected", theme.success()),
        ]),
        Line::from(vec![
            Span::styled("Latency: ", theme.muted()),
            Span::styled("~45ms avg", theme.info()),
        ]),
        Line::from(vec![
            Span::styled("Health: ", theme.muted()),
            Span::styled("● Healthy", theme.success()),
        ]),
    ];

    let mesh_block = Paragraph::new(mesh_info)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("🌐 Mesh")
                .border_style(theme.border()),
        );

    frame.render_widget(mesh_block, chunks[1]);

    // Sync info
    let sync_info = vec![
        Line::from(vec![
            Span::styled("Status: ", theme.muted()),
            Span::styled("● Syncing", theme.success()),
        ]),
        Line::from(vec![
            Span::styled("Queue: ", theme.muted()),
            Span::raw("0 pending"),
        ]),
        Line::from(vec![
            Span::styled("Last Sync: ", theme.muted()),
            Span::raw("2s ago"),
        ]),
    ];

    let sync_block = Paragraph::new(sync_info)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("🔄 Sync")
                .border_style(theme.border()),
        );

    frame.render_widget(sync_block, chunks[2]);
}

/// Render peer list.
fn render_peer_list(frame: &mut Frame, area: Rect, _app_state: &Arc<AppState>, theme: &Theme) {
    // Mock peer data
    let peers = vec![
        ("osaka-primary", "192.168.1.100", "● Online", "Primary", "28ms"),
        ("sapporo-1", "192.168.1.101", "● Online", "Secondary", "45ms"),
        ("fukuoka-2", "192.168.1.102", "● Online", "Secondary", "52ms"),
        ("tokyo-backup", "192.168.1.103", "○ Offline", "Secondary", "-"),
    ];

    let rows: Vec<Row> = peers
        .iter()
        .map(|(name, ip, status, role, latency)| {
            let status_style = if status.contains("Online") {
                theme.success()
            } else {
                theme.muted()
            };

            let role_style = if *role == "Primary" {
                theme.warning()
            } else {
                theme.info()
            };

            Row::new(vec![
                name.to_string(),
                ip.to_string(),
                Span::styled(*status, status_style).to_string(),
                Span::styled(*role, role_style).to_string(),
                latency.to_string(),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(18),
            Constraint::Length(18),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(8),
        ],
    )
    .header(
        Row::new(vec!["Node", "IP", "Status", "Role", "Latency"])
            .style(theme.header())
            .bottom_margin(1),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("👥 Connected Peers")
            .border_style(theme.border()),
    );

    frame.render_widget(table, area);
}

/// Render sync activity.
fn render_sync_activity(frame: &mut Frame, area: Rect, _app_state: &Arc<AppState>, theme: &Theme) {
    // Mock sync activity
    let activities = vec![
        ("signals", "osaka → sapporo", "2s ago", "✓"),
        ("trades", "fukuoka → osaka", "5s ago", "✓"),
        ("prices", "osaka → all", "8s ago", "✓"),
        ("orderbook", "sapporo → osaka", "12s ago", "✓"),
        ("metrics", "osaka → fukuoka", "15s ago", "✓"),
        ("signals", "osaka → sapporo", "18s ago", "✓"),
        ("chart_data", "osaka → all", "22s ago", "✓"),
        ("predictions", "fukuoka → osaka", "25s ago", "✓"),
        ("trades", "osaka → sapporo", "30s ago", "✓"),
        ("prices", "osaka → all", "35s ago", "✓"),
    ];

    let items: Vec<ListItem> = activities
        .iter()
        .map(|(entity, route, time, status)| {
            let status_style = if *status == "✓" {
                theme.success()
            } else {
                theme.error()
            };

            ListItem::new(Line::from(vec![
                Span::styled(format!("{:12}", entity), theme.info()),
                Span::raw("  "),
                Span::styled(format!("{:20}", route), theme.muted()),
                Span::raw("  "),
                Span::raw(format!("{:8}", time)),
                Span::raw("  "),
                Span::styled(*status, status_style),
            ]))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("🔄 Sync Activity")
            .border_style(theme.border()),
    );

    frame.render_widget(list, area);
}

/// Handle keyboard events for network view.
pub fn handle_event(_key: &KeyEvent) {
    // No interactive elements yet
}
