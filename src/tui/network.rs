//! Peer network view - Monitor mesh network.

use crate::AppState;
use crossterm::event::KeyEvent;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Row, Table},
    Frame,
};
use std::sync::Arc;

use super::{Theme, TuiState};

/// Render the network view.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    app_state: &Arc<AppState>,
    theme: &Theme,
    tui_state: &Arc<TuiState>,
) {
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
    render_sync_activity(frame, bottom_chunks[1], theme, tui_state);
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

    let node_id = app_state
        .peer_mesh
        .as_ref()
        .map(|m| m.server_id().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let node_region = app_state
        .peer_mesh
        .as_ref()
        .map(|m| m.server_region().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let node_info = vec![
        Line::from(vec![
            Span::styled("Role: ", theme.muted()),
            Span::styled(
                if is_primary { "Primary (Osaka)" } else { "Secondary" },
                if is_primary { theme.warning() } else { theme.info() },
            ),
        ]),
        Line::from(vec![
            Span::styled("Node: ", theme.muted()),
            Span::raw(node_id),
        ]),
        Line::from(vec![
            Span::styled("Region: ", theme.muted()),
            Span::raw(node_region),
        ]),
        Line::from(vec![
            Span::styled("Status: ", theme.muted()),
            Span::styled("● Connected", theme.success()),
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
    let statuses = app_state
        .peer_mesh
        .as_ref()
        .map(|m| m.get_all_statuses())
        .unwrap_or_default();

    let connected_count = statuses.len();
    let avg_latency = if connected_count > 0 {
        let sum: f64 = statuses
            .iter()
            .filter_map(|s| s.avg_latency_ms.or(s.latency_ms))
            .sum();
        let denom = statuses
            .iter()
            .filter(|s| s.avg_latency_ms.or(s.latency_ms).is_some())
            .count();
        if denom > 0 {
            Some(sum / denom as f64)
        } else {
            None
        }
    } else {
        None
    };

    let mesh_info = vec![
        Line::from(vec![
            Span::styled("Peers: ", theme.muted()),
            Span::raw(format!("{} connected", connected_count)),
        ]),
        Line::from(vec![
            Span::styled("Latency: ", theme.muted()),
            Span::raw(match avg_latency {
                Some(val) => format!("{:.0}ms avg", val),
                None => "n/a".to_string(),
            }),
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
            Span::raw(if app_state.sync_service.is_some() {
                "Enabled"
            } else {
                "Disabled"
            }),
        ]),
        Line::from(vec![
            Span::styled("Queue: ", theme.muted()),
            Span::raw("n/a"),
        ]),
        Line::from(vec![
            Span::styled("Last Sync: ", theme.muted()),
            Span::raw("n/a"),
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
fn render_peer_list(frame: &mut Frame, area: Rect, app_state: &Arc<AppState>, theme: &Theme) {
    let Some(mesh) = app_state.peer_mesh.as_ref() else {
        return;
    };

    let statuses = mesh.get_all_statuses();
    if statuses.is_empty() {
        let text = vec![
            Line::from(""),
            Line::from(Span::styled("No peer statuses available yet.", theme.muted())),
        ];

        let block = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("👥 Connected Peers")
                    .border_style(theme.border()),
            )
            .centered();

        frame.render_widget(block, area);
        return;
    }

    let rows: Vec<Row> = statuses
        .iter()
        .map(|status| {
            let status_style = match status.status {
                crate::types::PeerConnectionStatus::Connected => theme.success(),
                crate::types::PeerConnectionStatus::Connecting => theme.warning(),
                _ => theme.muted(),
            };

            let latency = status
                .avg_latency_ms
                .or(status.latency_ms)
                .map(|v| format!("{:.0}ms", v))
                .unwrap_or_else(|| "n/a".to_string());

            Row::new(vec![
                status.id.clone(),
                status.region.clone(),
                Span::styled(format!("{:?}", status.status), status_style).to_string(),
                latency,
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(18),
            Constraint::Length(12),
            Constraint::Length(14),
            Constraint::Length(10),
        ],
    )
    .header(
        Row::new(vec!["Node", "Region", "Status", "Latency"])
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
fn render_sync_activity(frame: &mut Frame, area: Rect, theme: &Theme, tui_state: &Arc<TuiState>) {
    let events = tui_state.recent_sync_events(20);
    if events.is_empty() {
        let text = vec![
            Line::from(""),
            Line::from(Span::styled("No sync activity yet.", theme.muted())),
        ];

        let block = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("🔄 Sync Activity")
                    .border_style(theme.border()),
            )
            .centered();

        frame.render_widget(block, area);
        return;
    }

    let items: Vec<ListItem> = events
        .iter()
        .rev()
        .map(|msg| {
            let line = match msg {
                crate::types::SyncMessage::DataUpdate {
                    entity_type,
                    entity_id,
                    version,
                    node_id,
                    ..
                } => format!(
                    "DataUpdate {:?} {} v{} from {}",
                    entity_type,
                    entity_id,
                    version,
                    node_id
                ),
                crate::types::SyncMessage::DataRequest {
                    entity_type,
                    entity_id,
                    ..
                } => format!("DataRequest {:?} {}", entity_type, entity_id),
                crate::types::SyncMessage::DataResponse {
                    entity_type,
                    entity_id,
                    version,
                    ..
                } => format!("DataResponse {:?} {} v{}", entity_type, entity_id, version),
                crate::types::SyncMessage::BulkSync {
                    entity_type,
                    page,
                    total_pages,
                    ..
                } => format!("BulkSync {:?} page {}/{}", entity_type, page, total_pages),
                crate::types::SyncMessage::ConflictDetected {
                    entity_type,
                    entity_id,
                    ..
                } => format!("ConflictDetected {:?} {}", entity_type, entity_id),
                crate::types::SyncMessage::ConflictResolution {
                    entity_type,
                    entity_id,
                    winner_node,
                    ..
                } => format!(
                    "ConflictResolution {:?} {} winner {}",
                    entity_type, entity_id, winner_node
                ),
                crate::types::SyncMessage::SyncHealthCheck {
                    node_id,
                    sync_lag_ms,
                    pending_syncs,
                    error_count,
                } => format!(
                    "HealthCheck {} lag {}ms pending {} errors {}",
                    node_id, sync_lag_ms, pending_syncs, error_count
                ),
                crate::types::SyncMessage::ReconcileRequest { entity_type, .. } => {
                    format!("ReconcileRequest {:?}", entity_type)
                }
                crate::types::SyncMessage::ChecksumRequest { entity_type, entity_id } => {
                    format!("ChecksumRequest {:?} {}", entity_type, entity_id)
                }
                crate::types::SyncMessage::ChecksumResponse { entity_type, entity_id, version, .. } => {
                    format!("ChecksumResponse {:?} {} v{}", entity_type, entity_id, version)
                }
                crate::types::SyncMessage::BatchUpdate { updates, compression } => {
                    let compressed = if compression.is_some() { " (compressed)" } else { "" };
                    format!("BatchUpdate {} entities{}", updates.len(), compressed)
                }
                crate::types::SyncMessage::DeltaUpdate {
                    entity_type,
                    entity_id,
                    version,
                    changes,
                    ..
                } => format!(
                    "DeltaUpdate {:?} {} v{} ({} fields)",
                    entity_type, entity_id, version, changes.len()
                ),
            };

            ListItem::new(Line::from(vec![Span::raw(line)]))
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
