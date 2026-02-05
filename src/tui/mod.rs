//! Terminal UI module for monitoring Haunt in real-time.

mod app;
mod dashboard;
mod database;
mod bots;
mod network;
mod logs;
mod theme;
mod events;

pub use app::{App, run_tui};
pub use theme::Theme;

use crate::AppState;

/// Route/View enum for navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Dashboard,
    Database,
    Bots,
    Network,
    Logs,
}

impl Route {
    /// Get all available routes.
    pub fn all() -> Vec<Self> {
        vec![
            Self::Dashboard,
            Self::Database,
            Self::Bots,
            Self::Network,
            Self::Logs,
        ]
    }

    /// Get the route name.
    pub fn name(&self) -> &str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Database => "Database",
            Self::Bots => "Bots",
            Self::Network => "Network",
            Self::Logs => "Logs",
        }
    }

    /// Get the route shortcut key.
    pub fn key(&self) -> char {
        match self {
            Self::Dashboard => '1',
            Self::Database => '2',
            Self::Bots => '3',
            Self::Network => '4',
            Self::Logs => '5',
        }
    }
}
