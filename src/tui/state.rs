//! Shared TUI state and log buffering.

use crate::types::{SyncMessage, Trade};
use std::collections::VecDeque;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use tracing_subscriber::fmt::MakeWriter;

/// In-memory log buffer for the TUI.
pub struct LogBuffer {
    lines: Mutex<VecDeque<String>>,
    capacity: usize,
}

impl LogBuffer {
    /// Create a new log buffer with a fixed capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            lines: Mutex::new(VecDeque::with_capacity(capacity)),
            capacity,
        }
    }

    /// Push a line into the buffer.
    pub fn push_line(&self, line: String) {
        let mut lines = self.lines.lock().unwrap();
        lines.push_back(line);
        while lines.len() > self.capacity {
            lines.pop_front();
        }
    }

    /// Get the most recent lines, up to limit.
    pub fn recent(&self, limit: usize) -> Vec<String> {
        let lines = self.lines.lock().unwrap();
        let start = lines.len().saturating_sub(limit);
        lines.iter().skip(start).cloned().collect()
    }
}

/// Writer that buffers log lines for the TUI.
pub struct LogWriter {
    buffer: Arc<LogBuffer>,
    line: Vec<u8>,
}

impl Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        for &b in buf {
            if b == b'\n' {
                if !self.line.is_empty() {
                    let line = String::from_utf8_lossy(&self.line).to_string();
                    self.buffer.push_line(line);
                    self.line.clear();
                }
            } else {
                self.line.push(b);
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if !self.line.is_empty() {
            let line = String::from_utf8_lossy(&self.line).to_string();
            self.buffer.push_line(line);
            self.line.clear();
        }
        Ok(())
    }
}

/// MakeWriter for tracing subscriber that writes into LogBuffer.
pub struct LogMakeWriter {
    buffer: Arc<LogBuffer>,
}

impl LogMakeWriter {
    /// Create a new writer factory for the given buffer.
    pub fn new(buffer: Arc<LogBuffer>) -> Self {
        Self { buffer }
    }
}

impl<'a> MakeWriter<'a> for LogMakeWriter {
    type Writer = LogWriter;

    fn make_writer(&'a self) -> Self::Writer {
        LogWriter {
            buffer: self.buffer.clone(),
            line: Vec::new(),
        }
    }
}

/// Shared TUI state for real-time feeds.
pub struct TuiState {
    log_buffer: Arc<LogBuffer>,
    trades: Mutex<VecDeque<Trade>>,
    sync_events: Mutex<VecDeque<SyncMessage>>,
    trade_capacity: usize,
    sync_capacity: usize,
}

impl TuiState {
    /// Create a new TUI state container.
    pub fn new(log_buffer: Arc<LogBuffer>, trade_capacity: usize, sync_capacity: usize) -> Self {
        Self {
            log_buffer,
            trades: Mutex::new(VecDeque::with_capacity(trade_capacity)),
            sync_events: Mutex::new(VecDeque::with_capacity(sync_capacity)),
            trade_capacity,
            sync_capacity,
        }
    }

    /// Access the log buffer.
    pub fn log_buffer(&self) -> Arc<LogBuffer> {
        self.log_buffer.clone()
    }

    /// Push a trade into the buffer.
    pub fn push_trade(&self, trade: Trade) {
        let mut trades = self.trades.lock().unwrap();
        trades.push_back(trade);
        while trades.len() > self.trade_capacity {
            trades.pop_front();
        }
    }

    /// Push a sync event into the buffer.
    pub fn push_sync(&self, msg: SyncMessage) {
        let mut events = self.sync_events.lock().unwrap();
        events.push_back(msg);
        while events.len() > self.sync_capacity {
            events.pop_front();
        }
    }

    /// Get recent trades, newest last.
    pub fn recent_trades(&self, limit: usize) -> Vec<Trade> {
        let trades = self.trades.lock().unwrap();
        let start = trades.len().saturating_sub(limit);
        trades.iter().skip(start).cloned().collect()
    }

    /// Get recent sync events, newest last.
    pub fn recent_sync_events(&self, limit: usize) -> Vec<SyncMessage> {
        let events = self.sync_events.lock().unwrap();
        let start = events.len().saturating_sub(limit);
        events.iter().skip(start).cloned().collect()
    }
}
