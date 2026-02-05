//! Logs view - System logs and events.

use crate::AppState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use std::sync::Arc;

use super::{Theme, TuiState};

/// Render the logs view.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    _app_state: &Arc<AppState>,
    theme: &Theme,
    tui_state: &Arc<TuiState>,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),  // Log controls
            Constraint::Min(0),     // Log output
        ])
        .split(area);

    render_log_controls(frame, chunks[0], theme);
    render_log_output(frame, chunks[1], theme, tui_state);
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
fn render_log_output(frame: &mut Frame, area: Rect, theme: &Theme, tui_state: &Arc<TuiState>) {
    let lines = tui_state.log_buffer().recent(200);
    if lines.is_empty() {
        let text = vec![
            Line::from(""),
            Line::from(Span::styled("No logs yet.", theme.muted())),
        ];

        let block = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("📝 System Logs (Live)")
                    .border_style(theme.border()),
            )
            .centered();

        frame.render_widget(block, area);
        return;
    }

    let items: Vec<ListItem> = lines
        .iter()
        .rev()
        .map(|line| {
            let style = if line.contains("ERROR") {
                theme.error()
            } else if line.contains("WARN") {
                theme.warning()
            } else if line.contains("INFO") {
                theme.success()
            } else if line.contains("DEBUG") {
                theme.muted()
            } else {
                theme.info()
            };

            ListItem::new(Line::from(Span::styled(line.clone(), style)))
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
