//! Main TUI application logic.

use super::{bots, dashboard, database, events, logs, network, Route, Theme};
use crate::AppState;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Tabs},
    Frame, Terminal,
};
use std::{
    io::{self, Stdout},
    sync::Arc,
    time::Duration,
};

/// Main TUI application.
pub struct App {
    /// Current route/view.
    current_route: Route,
    /// Application state.
    app_state: Arc<AppState>,
    /// Theme.
    theme: Theme,
    /// Should quit.
    should_quit: bool,
}

impl App {
    /// Create a new TUI application.
    pub fn new(app_state: Arc<AppState>) -> Self {
        Self {
            current_route: Route::Dashboard,
            app_state,
            theme: Theme::default(),
            should_quit: false,
        }
    }

    /// Handle an event.
    pub fn handle_event(&mut self, event: events::Event) {
        match event {
            events::Event::Key(key) => {
                if events::is_quit(&key) {
                    self.should_quit = true;
                    return;
                }

                // Route navigation
                for route in Route::all() {
                    if events::is_key(&key, crossterm::event::KeyCode::Char(route.key())) {
                        self.current_route = route;
                        return;
                    }
                }

                // Pass to current view
                match self.current_route {
                    Route::Dashboard => dashboard::handle_event(&key),
                    Route::Database => database::handle_event(&key),
                    Route::Bots => bots::handle_event(&key),
                    Route::Network => network::handle_event(&key),
                    Route::Logs => logs::handle_event(&key),
                }
            }
            events::Event::Tick => {
                // Periodic update handled by render
            }
            events::Event::Resize(_, _) => {
                // Terminal will handle resize automatically
            }
        }
    }

    /// Check if the app should quit.
    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    /// Render the TUI.
    pub fn render(&self, frame: &mut Frame) {
        let area = frame.size();

        // Create main layout: tabs at top, content below, status bar at bottom
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Tabs
                Constraint::Min(0),    // Content
                Constraint::Length(3), // Status bar
            ])
            .split(area);

        // Render tabs
        self.render_tabs(frame, chunks[0]);

        // Render current view
        match self.current_route {
            Route::Dashboard => dashboard::render(frame, chunks[1], &self.app_state, &self.theme),
            Route::Database => database::render(frame, chunks[1], &self.app_state, &self.theme),
            Route::Bots => bots::render(frame, chunks[1], &self.app_state, &self.theme),
            Route::Network => network::render(frame, chunks[1], &self.app_state, &self.theme),
            Route::Logs => logs::render(frame, chunks[1], &self.app_state, &self.theme),
        }

        // Render status bar
        self.render_status_bar(frame, chunks[2]);
    }

    /// Render the tabs.
    fn render_tabs(&self, frame: &mut Frame, area: Rect) {
        let routes = Route::all();
        let titles: Vec<Line> = routes
            .iter()
            .map(|r| {
                let key = r.key();
                Line::from(vec![
                    Span::styled(
                        format!("[{}] ", key),
                        self.theme.muted(),
                    ),
                    Span::raw(r.name()),
                ])
            })
            .collect();

        let selected = routes
            .iter()
            .position(|r| *r == self.current_route)
            .unwrap_or(0);

        let tabs = Tabs::new(titles)
            .block(Block::default().borders(Borders::ALL).title("Navigation"))
            .select(selected)
            .style(self.theme.tab_inactive())
            .highlight_style(self.theme.tab_active());

        frame.render_widget(tabs, area);
    }

    /// Render status bar.
    fn render_status_bar(&self, frame: &mut Frame, area: Rect) {
        let text = Line::from(vec![
            Span::styled("Haunt TUI", self.theme.title()),
            Span::raw(" | "),
            Span::styled("q", self.theme.muted()),
            Span::raw(" or "),
            Span::styled("Ctrl+C", self.theme.muted()),
            Span::raw(" to quit | "),
            Span::styled("1-5", self.theme.muted()),
            Span::raw(" to switch views"),
        ]);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.theme.border());

        frame.render_widget(block, area);
        
        let inner = Rect {
            x: area.x + 2,
            y: area.y + 1,
            width: area.width.saturating_sub(4),
            height: 1,
        };
        
        frame.render_widget(text, inner);
    }
}

/// Run the TUI application.
pub async fn run_tui(app_state: Arc<AppState>) -> io::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and event handler
    let mut app = App::new(app_state);
    let mut event_handler = events::EventHandler::new(Duration::from_millis(250));

    // Main loop
    loop {
        // Render
        terminal.draw(|f| app.render(f))?;

        // Handle events
        if let Some(event) = event_handler.next().await {
            app.handle_event(event);
        }

        // Check if should quit
        if app.should_quit() {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
