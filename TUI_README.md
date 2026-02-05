# Haunt Terminal UI (TUI)

A beautiful, real-time terminal user interface for monitoring your Haunt cryptocurrency trading system.

## Features

### 🎨 Beautiful Design
- Color-coded status indicators
- Clean, organized layouts
- Intuitive navigation
- Responsive terminal interface

### 📊 Multiple Views

#### 1. Dashboard (Press `1`)
- **System Status**: Server status, Redis connection, port information
- **Data Sources**: Real-time status of all 9 data sources (Coinbase, CoinGecko, Binance, etc.)
- **Trading Status**: Bot runner, sync service, and peer mesh status
- **Price Updates**: Top 10 cryptocurrencies with real-time prices and 24h changes
- **Statistics**: Total update counts and most frequently updated symbols

#### 2. Database Activity (Press `2`)
- **SQLite Status**: Database path and connection status
- **Redis Cache**: Cache connection and sync status
- **Recent Operations**: Real-time log of database operations (read/write)
  - Operation type (READ/WRITE)
  - Table name
  - Key accessed
  - Execution time
  - Status

#### 3. Bot Activity (Press `3`)
- **Bot Status Grid**: Overview of all active trading bots
  - Scalper Bot
  - Crypto Bro Bot
  - Grandma Bot
  - Quant Bot
- **Bot Metrics**: 
  - Active/Paused status
  - Total trades
  - Win count
  - Win rate percentage
- **Recent Trades Table**:
  - Trading pair
  - Buy/Sell side
  - Price and amount
  - Profit & Loss
  - Win/Loss status

#### 4. Peer Network (Press `4`)
- **Node Information**: 
  - Primary/Secondary role
  - Connection status
  - Uptime
- **Mesh Status**:
  - Connected peer count
  - Average latency
  - Network health
- **Peer List**: All connected nodes with:
  - Node name and IP
  - Online/Offline status
  - Role (Primary/Secondary)
  - Latency
- **Sync Activity**: Real-time synchronization events
  - Entity type being synced
  - Sync route (node → node)
  - Timestamp
  - Success/failure status

#### 5. System Logs (Press `5`)
- **Real-time Log Stream**: Live system logs with color coding
  - ✗ ERROR (red)
  - ⚠ WARN (yellow)
  - ● INFO (green)
  - ○ DEBUG (gray)
- **Log Controls**:
  - `[A]` - Show all logs
  - `[E]` - Filter errors only
  - `[W]` - Filter warnings only
  - `[I]` - Filter info only
  - `[D]` - Filter debug only
  - `[C]` - Clear logs
  - `[P]` - Pause/Resume

## Usage

### Starting the TUI

```bash
# From the Haunt directory
cargo run -- --tui
```

This will:
1. Start all background services (data sources, bots, peer mesh)
2. Launch the terminal UI
3. Begin real-time monitoring

### Navigation

- **Tab Navigation**: Press `1`, `2`, `3`, `4`, or `5` to switch between views
- **Quit**: Press `q` or `Ctrl+C` to exit the TUI

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `1` | Switch to Dashboard |
| `2` | Switch to Database Activity |
| `3` | Switch to Bot Activity |
| `4` | Switch to Peer Network |
| `5` | Switch to System Logs |
| `q` | Quit TUI |
| `Ctrl+C` | Quit TUI |

**In Logs View:**
| Key | Action |
|-----|--------|
| `A` | Show all log levels |
| `E` | Show errors only |
| `W` | Show warnings only |
| `I` | Show info only |
| `D` | Show debug only |
| `C` | Clear log buffer |
| `P` | Pause/Resume logging |

## Color Coding

- 🟢 **Green**: Success, online, active
- 🔴 **Red**: Error, offline, loss
- 🟡 **Yellow**: Warning, paused, caution
- 🔵 **Blue**: Info, secondary status
- ⚫ **Gray**: Muted, disabled, debug

## Requirements

- Terminal with 256-color support
- Minimum terminal size: 80x24 (larger recommended for best experience)
- UTF-8 encoding for proper icon display

## Technical Details

### Architecture
- Built with `ratatui` (modern Rust TUI library)
- Uses `crossterm` for cross-platform terminal manipulation
- Async event handling with Tokio
- Real-time updates every 250ms

### Data Sources
The TUI displays real-time data from:
- Price cache (live cryptocurrency prices)
- SQLite store (persistent data)
- Redis cache (when configured)
- Peer mesh network (when configured)
- Bot runner (when configured)
- All active data sources

### Performance
- Minimal CPU overhead (<1% on modern systems)
- Memory efficient (shared Arc pointers to app state)
- Non-blocking async updates
- Smooth 4 FPS refresh rate

## Troubleshooting

### Display Issues
- **Garbled characters**: Ensure your terminal supports UTF-8
- **Wrong colors**: Verify your terminal supports 256 colors
- **Layout issues**: Increase terminal size (minimum 80x24)

### Connection Issues
If services show as offline:
1. Check your `.env` configuration
2. Verify Redis is running (if configured)
3. Ensure API keys are valid
4. Check network connectivity for data sources

### No Data Displayed
- Wait a few seconds after launch for data to populate
- Verify the main Haunt server components are configured correctly
- Check system logs (View 5) for errors

## Development

### Adding New Views
To add a new view to the TUI:

1. Create a new file in `src/tui/` (e.g., `my_view.rs`)
2. Add the new route to `src/tui/mod.rs`:
   ```rust
   pub enum Route {
       // ... existing routes
       MyView,
   }
   ```
3. Implement the render function:
   ```rust
   pub fn render(frame: &mut Frame, area: Rect, app_state: &Arc<AppState>, theme: &Theme)
   ```
4. Add keyboard event handler if needed:
   ```rust
   pub fn handle_event(key: &KeyEvent)
   ```

### Customizing Themes
Edit `src/tui/theme.rs` to customize colors:
```rust
pub struct Theme {
    pub primary: Color,
    pub secondary: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
    // ... etc
}
```

## Future Enhancements

Planned features:
- [ ] Interactive bot control (pause/resume/configure)
- [ ] Real-time charts and sparklines
- [ ] Alert notifications
- [ ] Historical data views
- [ ] Performance metrics graphs
- [ ] Order book visualization
- [ ] Scrollable log history
- [ ] Search and filter capabilities
- [ ] Configuration hot-reload
- [ ] Export logs and reports

## License

MIT License - Same as the main Haunt project
