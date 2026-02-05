//! Event handling for the TUI.

use crossterm::event::{self, Event as CrosstermEvent, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;
use tokio::sync::mpsc;

/// Events that can occur in the TUI.
#[derive(Debug, Clone)]
pub enum Event {
    /// Terminal event (keyboard input).
    Key(KeyEvent),
    /// Tick event for periodic updates.
    Tick,
    /// Resize event.
    Resize(u16, u16),
}

/// Event handler that sends events over a channel.
pub struct EventHandler {
    _tx: mpsc::UnboundedSender<Event>,
    rx: mpsc::UnboundedReceiver<Event>,
}

impl EventHandler {
    /// Create a new event handler.
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let event_tx = tx.clone();

        // Spawn event listener
        tokio::spawn(async move {
            loop {
                // Check for crossterm events with timeout
                if event::poll(tick_rate).unwrap_or(false) {
                    match event::read() {
                        Ok(CrosstermEvent::Key(key)) => {
                            if event_tx.send(Event::Key(key)).is_err() {
                                break;
                            }
                        }
                        Ok(CrosstermEvent::Resize(w, h)) => {
                            if event_tx.send(Event::Resize(w, h)).is_err() {
                                break;
                            }
                        }
                        _ => {}
                    }
                } else {
                    // Send tick event
                    if event_tx.send(Event::Tick).is_err() {
                        break;
                    }
                }
            }
        });

        Self { _tx: tx, rx }
    }

    /// Receive the next event.
    pub async fn next(&mut self) -> Option<Event> {
        self.rx.recv().await
    }
}

/// Check if a key event matches a specific key code.
pub fn is_key(event: &KeyEvent, code: KeyCode) -> bool {
    event.code == code && event.modifiers == KeyModifiers::NONE
}

/// Check if a key event is Ctrl+C.
pub fn is_quit(event: &KeyEvent) -> bool {
    event.code == KeyCode::Char('c') && event.modifiers == KeyModifiers::CONTROL
        || event.code == KeyCode::Char('q')
}
