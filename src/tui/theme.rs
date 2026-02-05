//! Theme and color definitions for the TUI.

use ratatui::style::{Color, Modifier, Style};

/// Theme for the TUI with consistent color scheme.
#[derive(Debug, Clone)]
pub struct Theme {
    pub primary: Color,
    pub secondary: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
    pub info: Color,
    pub muted: Color,
    pub background: Color,
    pub foreground: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            primary: Color::Cyan,
            secondary: Color::Magenta,
            success: Color::Green,
            warning: Color::Yellow,
            danger: Color::Red,
            info: Color::Blue,
            muted: Color::DarkGray,
            background: Color::Reset,
            foreground: Color::Reset,
        }
    }
}

impl Theme {
    /// Get style for titles.
    pub fn title(&self) -> Style {
        Style::default()
            .fg(self.primary)
            .add_modifier(Modifier::BOLD)
    }

    /// Get style for headers.
    pub fn header(&self) -> Style {
        Style::default()
            .fg(self.secondary)
            .add_modifier(Modifier::BOLD)
    }

    /// Get style for success messages.
    pub fn success(&self) -> Style {
        Style::default().fg(self.success)
    }

    /// Get style for warnings.
    pub fn warning(&self) -> Style {
        Style::default().fg(self.warning)
    }

    /// Get style for errors.
    pub fn error(&self) -> Style {
        Style::default().fg(self.danger)
    }

    /// Get style for info messages.
    pub fn info(&self) -> Style {
        Style::default().fg(self.info)
    }

    /// Get style for muted text.
    pub fn muted(&self) -> Style {
        Style::default().fg(self.muted)
    }

    /// Get style for selected items.
    pub fn selected(&self) -> Style {
        Style::default()
            .fg(Color::Black)
            .bg(self.primary)
            .add_modifier(Modifier::BOLD)
    }

    /// Get style for borders.
    pub fn border(&self) -> Style {
        Style::default().fg(self.primary)
    }

    /// Get style for tabs (active).
    pub fn tab_active(&self) -> Style {
        Style::default()
            .fg(Color::Black)
            .bg(self.primary)
            .add_modifier(Modifier::BOLD)
    }

    /// Get style for tabs (inactive).
    pub fn tab_inactive(&self) -> Style {
        Style::default().fg(self.muted)
    }
}
