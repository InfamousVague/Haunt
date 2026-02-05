//! SQLite persistence layer for long-term profile and prediction storage.
//!
//! SQLite is used for data that should survive Redis restarts:
//! - User profiles (persist forever)
//! - Prediction history archive (historical accuracy tracking)
//!
//! Redis is still used for:
//! - Sessions (24-hour TTL, ephemeral)
//! - Recent predictions (7-day TTL, quick access)

use crate::types::{
    AssetClass, BracketRole, CostBasisMethod, EquityPoint, Fill, FundingPayment, Greeks,
    InsuranceFund, Liquidation, MarginChangeType, MarginHistory, MarginMode, OptionPosition,
    OptionStyle, OptionType, Order, OrderSide, OrderStatus, OrderType, Position, PositionSide,
    Portfolio, PredictionOutcome, Profile, ProfileSettings, RiskSettings, SignalPrediction,
    StrategyStatus, TimeInForce, Trade, TradingRule, TradingStrategy,
};
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;
use tracing::{debug, error, info};
use uuid::Uuid;

/// SQLite store for persistent profile and prediction data.
pub struct SqliteStore {
    conn: Mutex<Connection>,
    pub db_path: String,
}

impl SqliteStore {
    /// Create a new SQLite store at the given path.
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, rusqlite::Error> {
        let db_path = path.as_ref().to_string_lossy().to_string();
        let conn = Connection::open(path)?;
        let store = Self {
            conn: Mutex::new(conn),
            db_path,
        };
        store.init_schema()?;
        info!("SQLite store initialized");
        Ok(store)
    }

    /// Create an in-memory SQLite store (for testing).
    pub fn new_in_memory() -> Result<Self, rusqlite::Error> {
        let conn = Connection::open_in_memory()?;
        let store = Self {
            conn: Mutex::new(conn),
            db_path: ":memory:".to_string(),
        };
        store.init_schema()?;
        debug!("In-memory SQLite store initialized");
        Ok(store)
    }

    /// Initialize database schema.
    fn init_schema(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        // Profiles table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS profiles (
                id TEXT PRIMARY KEY,
                public_key TEXT UNIQUE NOT NULL,
                username TEXT NOT NULL DEFAULT '',
                created_at INTEGER NOT NULL,
                last_seen INTEGER NOT NULL,
                show_on_leaderboard INTEGER NOT NULL DEFAULT 0,
                leaderboard_signature TEXT,
                leaderboard_consent_at INTEGER,
                settings_json TEXT DEFAULT '{}'
            )",
            [],
        )?;

        // Migrate existing profiles table to add new columns
        let _ = conn.execute(
            "ALTER TABLE profiles ADD COLUMN username TEXT NOT NULL DEFAULT ''",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE profiles ADD COLUMN show_on_leaderboard INTEGER NOT NULL DEFAULT 0",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE profiles ADD COLUMN leaderboard_signature TEXT",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE profiles ADD COLUMN leaderboard_consent_at INTEGER",
            [],
        );

        // Index on public_key for faster lookups
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_profiles_public_key ON profiles(public_key)",
            [],
        )?;

        // Prediction history table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS prediction_history (
                id TEXT PRIMARY KEY,
                symbol TEXT NOT NULL,
                indicator TEXT NOT NULL,
                direction TEXT NOT NULL,
                score INTEGER NOT NULL,
                price_at_prediction REAL NOT NULL,
                timestamp INTEGER NOT NULL,
                price_after_5m REAL,
                price_after_1h REAL,
                price_after_4h REAL,
                price_after_24h REAL,
                outcome_5m TEXT,
                outcome_1h TEXT,
                outcome_4h TEXT,
                outcome_24h TEXT
            )",
            [],
        )?;

        // Indexes for prediction queries
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_predictions_symbol ON prediction_history(symbol)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_predictions_timestamp ON prediction_history(timestamp DESC)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_predictions_symbol_indicator
             ON prediction_history(symbol, indicator)",
            [],
        )?;

        // ========== Trading Tables ==========

        // Portfolios table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS portfolios (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT,
                base_currency TEXT NOT NULL DEFAULT 'USD',
                starting_balance REAL NOT NULL,
                cash_balance REAL NOT NULL,
                margin_used REAL NOT NULL DEFAULT 0,
                margin_available REAL NOT NULL,
                unrealized_pnl REAL NOT NULL DEFAULT 0,
                realized_pnl REAL NOT NULL DEFAULT 0,
                total_value REAL NOT NULL,
                cost_basis_method TEXT NOT NULL DEFAULT 'fifo',
                risk_settings_json TEXT DEFAULT '{}',
                is_competition INTEGER NOT NULL DEFAULT 0,
                competition_id TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                total_trades INTEGER NOT NULL DEFAULT 0,
                winning_trades INTEGER NOT NULL DEFAULT 0
            )",
            [],
        )?;

        // Add new columns for existing databases (migrations)
        let _ = conn.execute("ALTER TABLE portfolios ADD COLUMN total_trades INTEGER NOT NULL DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE portfolios ADD COLUMN winning_trades INTEGER NOT NULL DEFAULT 0", []);

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_portfolios_user_id ON portfolios(user_id)",
            [],
        )?;

        // Orders table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS orders (
                id TEXT PRIMARY KEY,
                portfolio_id TEXT NOT NULL,
                symbol TEXT NOT NULL,
                asset_class TEXT NOT NULL,
                side TEXT NOT NULL,
                order_type TEXT NOT NULL,
                quantity REAL NOT NULL,
                filled_quantity REAL NOT NULL DEFAULT 0,
                price REAL,
                stop_price REAL,
                trail_amount REAL,
                trail_percent REAL,
                time_in_force TEXT NOT NULL DEFAULT 'gtc',
                status TEXT NOT NULL,
                linked_order_id TEXT,
                bracket_id TEXT,
                leverage REAL NOT NULL DEFAULT 1.0,
                fills_json TEXT DEFAULT '[]',
                avg_fill_price REAL,
                total_fees REAL NOT NULL DEFAULT 0,
                client_order_id TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                expires_at INTEGER,
                trail_high_price REAL,
                trail_low_price REAL,
                bracket_role TEXT,
                FOREIGN KEY (portfolio_id) REFERENCES portfolios(id)
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_orders_portfolio_id ON orders(portfolio_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_orders_status ON orders(status)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_orders_symbol ON orders(symbol)",
            [],
        )?;

        // Positions table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS positions (
                id TEXT PRIMARY KEY,
                portfolio_id TEXT NOT NULL,
                symbol TEXT NOT NULL,
                asset_class TEXT NOT NULL,
                side TEXT NOT NULL,
                quantity REAL NOT NULL,
                entry_price REAL NOT NULL,
                current_price REAL NOT NULL,
                unrealized_pnl REAL NOT NULL DEFAULT 0,
                unrealized_pnl_pct REAL NOT NULL DEFAULT 0,
                realized_pnl REAL NOT NULL DEFAULT 0,
                margin_used REAL NOT NULL,
                leverage REAL NOT NULL DEFAULT 1.0,
                margin_mode TEXT NOT NULL DEFAULT 'isolated',
                liquidation_price REAL,
                stop_loss REAL,
                take_profit REAL,
                cost_basis_json TEXT DEFAULT '[]',
                funding_payments REAL NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                closed_at INTEGER,
                FOREIGN KEY (portfolio_id) REFERENCES portfolios(id)
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_positions_portfolio_id ON positions(portfolio_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_positions_symbol ON positions(symbol)",
            [],
        )?;

        // Trades table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS trades (
                id TEXT PRIMARY KEY,
                order_id TEXT NOT NULL,
                portfolio_id TEXT NOT NULL,
                position_id TEXT,
                symbol TEXT NOT NULL,
                asset_class TEXT NOT NULL,
                side TEXT NOT NULL,
                quantity REAL NOT NULL,
                price REAL NOT NULL,
                fee REAL NOT NULL DEFAULT 0,
                slippage REAL NOT NULL DEFAULT 0,
                realized_pnl REAL,
                executed_at INTEGER NOT NULL,
                FOREIGN KEY (order_id) REFERENCES orders(id),
                FOREIGN KEY (portfolio_id) REFERENCES portfolios(id),
                FOREIGN KEY (position_id) REFERENCES positions(id)
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_trades_portfolio_id ON trades(portfolio_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_trades_order_id ON trades(order_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_trades_executed_at ON trades(executed_at DESC)",
            [],
        )?;

        // ========== Perpetual Futures Tables ==========

        // Funding payments table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS funding_payments (
                id TEXT PRIMARY KEY,
                position_id TEXT NOT NULL,
                portfolio_id TEXT NOT NULL,
                symbol TEXT NOT NULL,
                position_size REAL NOT NULL,
                side TEXT NOT NULL,
                funding_rate REAL NOT NULL,
                payment REAL NOT NULL,
                paid_at INTEGER NOT NULL,
                FOREIGN KEY (position_id) REFERENCES positions(id),
                FOREIGN KEY (portfolio_id) REFERENCES portfolios(id)
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_funding_payments_position ON funding_payments(position_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_funding_payments_portfolio ON funding_payments(portfolio_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_funding_payments_paid_at ON funding_payments(paid_at DESC)",
            [],
        )?;

        // Liquidations table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS liquidations (
                id TEXT PRIMARY KEY,
                position_id TEXT NOT NULL,
                portfolio_id TEXT NOT NULL,
                symbol TEXT NOT NULL,
                quantity REAL NOT NULL,
                liquidation_price REAL NOT NULL,
                mark_price REAL NOT NULL,
                loss REAL NOT NULL,
                liquidation_fee REAL NOT NULL,
                is_partial INTEGER NOT NULL DEFAULT 0,
                remaining_quantity REAL,
                liquidated_at INTEGER NOT NULL,
                FOREIGN KEY (position_id) REFERENCES positions(id),
                FOREIGN KEY (portfolio_id) REFERENCES portfolios(id)
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_liquidations_portfolio ON liquidations(portfolio_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_liquidations_liquidated_at ON liquidations(liquidated_at DESC)",
            [],
        )?;

        // Margin history table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS margin_history (
                id TEXT PRIMARY KEY,
                portfolio_id TEXT NOT NULL,
                position_id TEXT,
                change_type TEXT NOT NULL,
                previous_margin_level REAL NOT NULL,
                new_margin_level REAL NOT NULL,
                previous_margin_used REAL NOT NULL,
                new_margin_used REAL NOT NULL,
                amount_changed REAL NOT NULL,
                reason TEXT,
                timestamp INTEGER NOT NULL,
                FOREIGN KEY (portfolio_id) REFERENCES portfolios(id),
                FOREIGN KEY (position_id) REFERENCES positions(id)
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_margin_history_portfolio ON margin_history(portfolio_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_margin_history_timestamp ON margin_history(timestamp DESC)",
            [],
        )?;

        // Portfolio snapshots table (for equity curve charting)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS portfolio_snapshots (
                id TEXT PRIMARY KEY,
                portfolio_id TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                equity REAL NOT NULL,
                cash REAL NOT NULL,
                positions_value REAL NOT NULL,
                realized_pnl REAL NOT NULL,
                unrealized_pnl REAL NOT NULL,
                drawdown_pct REAL NOT NULL,
                peak_equity REAL NOT NULL,
                FOREIGN KEY (portfolio_id) REFERENCES portfolios(id)
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_portfolio_snapshots_portfolio ON portfolio_snapshots(portfolio_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_portfolio_snapshots_timestamp ON portfolio_snapshots(portfolio_id, timestamp DESC)",
            [],
        )?;

        // Insurance fund table (single row, global state)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS insurance_fund (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                balance REAL NOT NULL DEFAULT 0,
                total_contributions REAL NOT NULL DEFAULT 0,
                total_payouts REAL NOT NULL DEFAULT 0,
                liquidations_covered INTEGER NOT NULL DEFAULT 0,
                updated_at INTEGER NOT NULL
            )",
            [],
        )?;

        // Initialize insurance fund if not exists
        conn.execute(
            "INSERT OR IGNORE INTO insurance_fund (id, balance, total_contributions, total_payouts, liquidations_covered, updated_at)
             VALUES (1, 0, 0, 0, 0, ?1)",
            params![chrono::Utc::now().timestamp_millis()],
        )?;

        // ========== Options Trading Tables ==========

        // Options positions table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS options_positions (
                id TEXT PRIMARY KEY,
                portfolio_id TEXT NOT NULL,
                contract_symbol TEXT NOT NULL,
                underlying_symbol TEXT NOT NULL,
                option_type TEXT NOT NULL,
                strike REAL NOT NULL,
                expiration INTEGER NOT NULL,
                style TEXT NOT NULL,
                contracts INTEGER NOT NULL,
                multiplier INTEGER NOT NULL DEFAULT 100,
                entry_premium REAL NOT NULL,
                current_premium REAL NOT NULL,
                underlying_price REAL NOT NULL,
                unrealized_pnl REAL NOT NULL DEFAULT 0,
                realized_pnl REAL NOT NULL DEFAULT 0,
                greeks_json TEXT DEFAULT '{}',
                entry_iv REAL NOT NULL,
                current_iv REAL NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                closed_at INTEGER,
                FOREIGN KEY (portfolio_id) REFERENCES portfolios(id)
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_options_positions_portfolio ON options_positions(portfolio_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_options_positions_underlying ON options_positions(underlying_symbol)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_options_positions_expiration ON options_positions(expiration)",
            [],
        )?;

        // ========== Auto-Trading Strategy Tables ==========

        // Strategies table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS strategies (
                id TEXT PRIMARY KEY,
                portfolio_id TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT,
                symbols_json TEXT NOT NULL DEFAULT '[]',
                asset_class TEXT,
                rules_json TEXT NOT NULL DEFAULT '[]',
                status TEXT NOT NULL DEFAULT 'paused',
                cooldown_seconds INTEGER NOT NULL DEFAULT 3600,
                max_positions INTEGER NOT NULL DEFAULT 3,
                max_position_size_pct REAL NOT NULL DEFAULT 0.10,
                last_trade_at INTEGER,
                total_trades INTEGER NOT NULL DEFAULT 0,
                winning_trades INTEGER NOT NULL DEFAULT 0,
                losing_trades INTEGER NOT NULL DEFAULT 0,
                realized_pnl REAL NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                FOREIGN KEY (portfolio_id) REFERENCES portfolios(id)
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_strategies_portfolio ON strategies(portfolio_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_strategies_status ON strategies(status)",
            [],
        )?;

        // ========== Data Sync Tables ==========

        // Sync versions tracking table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sync_versions (
                entity_type TEXT NOT NULL,
                entity_id TEXT NOT NULL,
                node_id TEXT NOT NULL,
                version INTEGER NOT NULL,
                timestamp INTEGER NOT NULL,
                checksum TEXT NOT NULL,
                PRIMARY KEY (entity_type, entity_id, node_id)
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_sync_versions_entity ON sync_versions(entity_type, entity_id)",
            [],
        )?;

        // Sync state table (single row)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sync_state (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                last_full_sync_at INTEGER NOT NULL DEFAULT 0,
                last_incremental_sync_at INTEGER NOT NULL DEFAULT 0,
                sync_cursor_position INTEGER NOT NULL DEFAULT 0,
                pending_sync_count INTEGER NOT NULL DEFAULT 0,
                failed_sync_count INTEGER NOT NULL DEFAULT 0,
                total_synced_entities INTEGER NOT NULL DEFAULT 0,
                sync_enabled INTEGER NOT NULL DEFAULT 1
            )",
            [],
        )?;

        // Initialize sync state if not exists
        conn.execute(
            "INSERT OR IGNORE INTO sync_state (id, last_full_sync_at, last_incremental_sync_at)
             VALUES (1, 0, 0)",
            [],
        )?;

        // Sync queue table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sync_queue (
                id TEXT PRIMARY KEY,
                entity_type TEXT NOT NULL,
                entity_id TEXT NOT NULL,
                operation TEXT NOT NULL,
                priority INTEGER NOT NULL DEFAULT 5,
                target_nodes TEXT,
                retry_count INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL,
                scheduled_at INTEGER NOT NULL,
                attempted_at INTEGER,
                completed_at INTEGER,
                error TEXT
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_sync_queue_scheduled ON sync_queue(scheduled_at)
             WHERE completed_at IS NULL",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_sync_queue_priority ON sync_queue(priority DESC, scheduled_at)
             WHERE completed_at IS NULL",
            [],
        )?;

        // Sync conflicts table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sync_conflicts (
                id TEXT PRIMARY KEY,
                entity_type TEXT NOT NULL,
                entity_id TEXT NOT NULL,
                node_a TEXT NOT NULL,
                version_a INTEGER NOT NULL,
                data_a BLOB NOT NULL,
                timestamp_a INTEGER NOT NULL,
                node_b TEXT NOT NULL,
                version_b INTEGER NOT NULL,
                data_b BLOB NOT NULL,
                timestamp_b INTEGER NOT NULL,
                detected_at INTEGER NOT NULL,
                resolved_at INTEGER,
                resolution_strategy TEXT,
                winner_node TEXT
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_sync_conflicts_entity ON sync_conflicts(entity_type, entity_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_sync_conflicts_unresolved ON sync_conflicts(detected_at)
             WHERE resolved_at IS NULL",
            [],
        )?;

        // Node metrics table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS node_metrics (
                id TEXT PRIMARY KEY,
                node_id TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                sync_lag_ms INTEGER NOT NULL,
                pending_sync_count INTEGER NOT NULL,
                synced_entities_1m INTEGER NOT NULL,
                sync_errors_1m INTEGER NOT NULL,
                sync_throughput_mbps REAL NOT NULL,
                db_size_mb REAL NOT NULL,
                db_row_count INTEGER NOT NULL,
                db_write_rate REAL NOT NULL,
                db_read_rate REAL NOT NULL,
                cpu_usage_pct REAL,
                memory_usage_mb REAL,
                disk_usage_pct REAL,
                network_rx_mbps REAL,
                network_tx_mbps REAL,
                active_users INTEGER NOT NULL,
                active_portfolios INTEGER NOT NULL,
                open_orders INTEGER NOT NULL,
                open_positions INTEGER NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_node_metrics_node_time ON node_metrics(node_id, timestamp DESC)",
            [],
        )?;

        // ========== Add Sync Columns to Existing Tables ==========

        // Add version tracking columns (migrations for existing databases)
        // These may fail if columns already exist, which is fine
        
        let _ = conn.execute("ALTER TABLE profiles ADD COLUMN version INTEGER NOT NULL DEFAULT 1", []);
        let _ = conn.execute("ALTER TABLE profiles ADD COLUMN last_modified_at INTEGER NOT NULL DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE profiles ADD COLUMN last_modified_by TEXT NOT NULL DEFAULT ''", []);

        let _ = conn.execute("ALTER TABLE portfolios ADD COLUMN version INTEGER NOT NULL DEFAULT 1", []);
        let _ = conn.execute("ALTER TABLE portfolios ADD COLUMN last_modified_at INTEGER NOT NULL DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE portfolios ADD COLUMN last_modified_by TEXT NOT NULL DEFAULT ''", []);

        let _ = conn.execute("ALTER TABLE orders ADD COLUMN version INTEGER NOT NULL DEFAULT 1", []);
        let _ = conn.execute("ALTER TABLE orders ADD COLUMN last_modified_at INTEGER NOT NULL DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE orders ADD COLUMN last_modified_by TEXT NOT NULL DEFAULT ''", []);

        let _ = conn.execute("ALTER TABLE positions ADD COLUMN version INTEGER NOT NULL DEFAULT 1", []);
        let _ = conn.execute("ALTER TABLE positions ADD COLUMN last_modified_at INTEGER NOT NULL DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE positions ADD COLUMN last_modified_by TEXT NOT NULL DEFAULT ''", []);

        let _ = conn.execute("ALTER TABLE trades ADD COLUMN version INTEGER NOT NULL DEFAULT 1", []);
        let _ = conn.execute("ALTER TABLE trades ADD COLUMN last_modified_at INTEGER NOT NULL DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE trades ADD COLUMN last_modified_by TEXT NOT NULL DEFAULT ''", []);

        let _ = conn.execute("ALTER TABLE options_positions ADD COLUMN version INTEGER NOT NULL DEFAULT 1", []);
        let _ = conn.execute("ALTER TABLE options_positions ADD COLUMN last_modified_at INTEGER NOT NULL DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE options_positions ADD COLUMN last_modified_by TEXT NOT NULL DEFAULT ''", []);

        let _ = conn.execute("ALTER TABLE strategies ADD COLUMN version INTEGER NOT NULL DEFAULT 1", []);
        let _ = conn.execute("ALTER TABLE strategies ADD COLUMN last_modified_at INTEGER NOT NULL DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE strategies ADD COLUMN last_modified_by TEXT NOT NULL DEFAULT ''", []);

        let _ = conn.execute("ALTER TABLE funding_payments ADD COLUMN version INTEGER NOT NULL DEFAULT 1", []);
        let _ = conn.execute("ALTER TABLE funding_payments ADD COLUMN last_modified_at INTEGER NOT NULL DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE funding_payments ADD COLUMN last_modified_by TEXT NOT NULL DEFAULT ''", []);

        let _ = conn.execute("ALTER TABLE liquidations ADD COLUMN version INTEGER NOT NULL DEFAULT 1", []);
        let _ = conn.execute("ALTER TABLE liquidations ADD COLUMN last_modified_at INTEGER NOT NULL DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE liquidations ADD COLUMN last_modified_by TEXT NOT NULL DEFAULT ''", []);

        let _ = conn.execute("ALTER TABLE margin_history ADD COLUMN version INTEGER NOT NULL DEFAULT 1", []);
        let _ = conn.execute("ALTER TABLE margin_history ADD COLUMN last_modified_at INTEGER NOT NULL DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE margin_history ADD COLUMN last_modified_by TEXT NOT NULL DEFAULT ''", []);

        let _ = conn.execute("ALTER TABLE portfolio_snapshots ADD COLUMN version INTEGER NOT NULL DEFAULT 1", []);
        let _ = conn.execute("ALTER TABLE portfolio_snapshots ADD COLUMN last_modified_at INTEGER NOT NULL DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE portfolio_snapshots ADD COLUMN last_modified_by TEXT NOT NULL DEFAULT ''", []);

        let _ = conn.execute("ALTER TABLE insurance_fund ADD COLUMN version INTEGER NOT NULL DEFAULT 1", []);
        let _ = conn.execute("ALTER TABLE insurance_fund ADD COLUMN last_modified_at INTEGER NOT NULL DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE insurance_fund ADD COLUMN last_modified_by TEXT NOT NULL DEFAULT 'osaka'", []);

        let _ = conn.execute("ALTER TABLE prediction_history ADD COLUMN version INTEGER NOT NULL DEFAULT 1", []);
        let _ = conn.execute("ALTER TABLE prediction_history ADD COLUMN last_modified_at INTEGER NOT NULL DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE prediction_history ADD COLUMN last_modified_by TEXT NOT NULL DEFAULT ''", []);

        info!("SQLite schema initialized with sync tables");
        Ok(())
    }

    // ========== Profile Methods ==========

    /// Get a profile by public key.
    pub fn get_profile(&self, public_key: &str) -> Option<Profile> {
        let conn = self.conn.lock().unwrap();

        let result = conn.query_row(
            "SELECT id, public_key, username, created_at, last_seen,
                    show_on_leaderboard, leaderboard_signature, leaderboard_consent_at, settings_json
             FROM profiles WHERE public_key = ?1",
            params![public_key],
            |row| {
                let settings_json: String = row.get(8)?;
                let settings: ProfileSettings =
                    serde_json::from_str(&settings_json).unwrap_or_default();

                Ok(Profile {
                    id: row.get(0)?,
                    public_key: row.get(1)?,
                    username: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    created_at: row.get(3)?,
                    last_seen: row.get(4)?,
                    show_on_leaderboard: row.get::<_, i64>(5)? != 0,
                    leaderboard_signature: row.get(6)?,
                    leaderboard_consent_at: row.get(7)?,
                    settings,
                })
            },
        );

        match result {
            Ok(profile) => Some(profile),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => {
                error!("Error fetching profile: {}", e);
                None
            }
        }
    }

    /// Save or update a profile.
    pub fn save_profile(&self, profile: &Profile) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let settings_json = serde_json::to_string(&profile.settings).unwrap_or_default();

        conn.execute(
            "INSERT INTO profiles (id, public_key, username, created_at, last_seen,
                                   show_on_leaderboard, leaderboard_signature, leaderboard_consent_at, settings_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(public_key) DO UPDATE SET
                username = excluded.username,
                last_seen = excluded.last_seen,
                show_on_leaderboard = excluded.show_on_leaderboard,
                leaderboard_signature = excluded.leaderboard_signature,
                leaderboard_consent_at = excluded.leaderboard_consent_at,
                settings_json = excluded.settings_json",
            params![
                profile.id,
                profile.public_key,
                profile.username,
                profile.created_at,
                profile.last_seen,
                profile.show_on_leaderboard as i64,
                profile.leaderboard_signature,
                profile.leaderboard_consent_at,
                settings_json,
            ],
        )?;

        debug!(
            "Saved profile for {}",
            &profile.public_key[..16.min(profile.public_key.len())]
        );
        Ok(())
    }

    /// Update profile's last_seen timestamp.
    pub fn update_last_seen(&self, public_key: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().timestamp_millis();

        conn.execute(
            "UPDATE profiles SET last_seen = ?1 WHERE public_key = ?2",
            params![now, public_key],
        )?;

        Ok(())
    }

    /// Delete a profile.
    pub fn delete_profile(&self, public_key: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM profiles WHERE public_key = ?1",
            params![public_key],
        )?;
        Ok(())
    }

    /// Get total profile count.
    pub fn profile_count(&self) -> usize {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM profiles", [], |row| row.get(0))
            .unwrap_or(0)
    }

    // ========== Prediction History Methods ==========

    /// Archive a prediction to SQLite.
    pub fn archive_prediction(&self, prediction: &SignalPrediction) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "INSERT INTO prediction_history
             (id, symbol, indicator, direction, score, price_at_prediction, timestamp,
              price_after_5m, price_after_1h, price_after_4h, price_after_24h,
              outcome_5m, outcome_1h, outcome_4h, outcome_24h)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
             ON CONFLICT(id) DO UPDATE SET
                price_after_5m = COALESCE(excluded.price_after_5m, price_after_5m),
                price_after_1h = COALESCE(excluded.price_after_1h, price_after_1h),
                price_after_4h = COALESCE(excluded.price_after_4h, price_after_4h),
                price_after_24h = COALESCE(excluded.price_after_24h, price_after_24h),
                outcome_5m = COALESCE(excluded.outcome_5m, outcome_5m),
                outcome_1h = COALESCE(excluded.outcome_1h, outcome_1h),
                outcome_4h = COALESCE(excluded.outcome_4h, outcome_4h),
                outcome_24h = COALESCE(excluded.outcome_24h, outcome_24h)",
            params![
                prediction.id.to_string(),
                prediction.symbol.to_lowercase(),
                prediction.indicator,
                format!("{:?}", prediction.direction).to_lowercase(),
                prediction.score,
                prediction.price_at_prediction,
                prediction.timestamp,
                prediction.price_after_5m,
                prediction.price_after_1h,
                prediction.price_after_4h,
                prediction.price_after_24h,
                prediction
                    .outcome_5m
                    .as_ref()
                    .map(|o| format!("{:?}", o).to_lowercase()),
                prediction
                    .outcome_1h
                    .as_ref()
                    .map(|o| format!("{:?}", o).to_lowercase()),
                prediction
                    .outcome_4h
                    .as_ref()
                    .map(|o| format!("{:?}", o).to_lowercase()),
                prediction
                    .outcome_24h
                    .as_ref()
                    .map(|o| format!("{:?}", o).to_lowercase()),
            ],
        )?;

        debug!(
            "Archived prediction {} for {}",
            prediction.id, prediction.symbol
        );
        Ok(())
    }

    /// Get predictions for a symbol with optional filtering.
    pub fn get_predictions(
        &self,
        symbol: &str,
        status: Option<&str>,
        limit: usize,
    ) -> Vec<SignalPrediction> {
        let conn = self.conn.lock().unwrap();
        let symbol_lower = symbol.to_lowercase();

        let query = match status {
            Some("validated") => {
                // Return predictions with ANY validated outcome (5m, 1h, 4h, or 24h)
                "SELECT id, symbol, indicator, direction, score, price_at_prediction, timestamp,
                        price_after_5m, price_after_1h, price_after_4h, price_after_24h,
                        outcome_5m, outcome_1h, outcome_4h, outcome_24h
                 FROM prediction_history
                 WHERE symbol = ?1 AND (outcome_5m IS NOT NULL OR outcome_1h IS NOT NULL OR outcome_4h IS NOT NULL OR outcome_24h IS NOT NULL)
                 ORDER BY timestamp DESC
                 LIMIT ?2"
            }
            Some("pending") => {
                // Return predictions with NO validated outcomes yet
                "SELECT id, symbol, indicator, direction, score, price_at_prediction, timestamp,
                        price_after_5m, price_after_1h, price_after_4h, price_after_24h,
                        outcome_5m, outcome_1h, outcome_4h, outcome_24h
                 FROM prediction_history
                 WHERE symbol = ?1 AND outcome_5m IS NULL AND outcome_1h IS NULL AND outcome_4h IS NULL AND outcome_24h IS NULL
                 ORDER BY timestamp DESC
                 LIMIT ?2"
            }
            _ => {
                "SELECT id, symbol, indicator, direction, score, price_at_prediction, timestamp,
                        price_after_5m, price_after_1h, price_after_4h, price_after_24h,
                        outcome_5m, outcome_1h, outcome_4h, outcome_24h
                 FROM prediction_history
                 WHERE symbol = ?1
                 ORDER BY timestamp DESC
                 LIMIT ?2"
            }
        };

        let mut stmt = match conn.prepare(query) {
            Ok(stmt) => stmt,
            Err(e) => {
                error!("Error preparing prediction query: {}", e);
                return Vec::new();
            }
        };

        let predictions = stmt
            .query_map(params![symbol_lower, limit as i64], |row| {
                let id_str: String = row.get(0)?;
                Ok(SignalPrediction {
                    id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
                    symbol: row.get(1)?,
                    indicator: row.get(2)?,
                    direction: parse_direction(&row.get::<_, String>(3)?),
                    score: row.get(4)?,
                    price_at_prediction: row.get(5)?,
                    timestamp: row.get(6)?,
                    validated: row.get::<_, Option<String>>(14)?.is_some(),
                    price_after_5m: row.get(7)?,
                    price_after_1h: row.get(8)?,
                    price_after_4h: row.get(9)?,
                    price_after_24h: row.get(10)?,
                    outcome_5m: row.get::<_, Option<String>>(11)?.map(|s| parse_outcome(&s)),
                    outcome_1h: row.get::<_, Option<String>>(12)?.map(|s| parse_outcome(&s)),
                    outcome_4h: row.get::<_, Option<String>>(13)?.map(|s| parse_outcome(&s)),
                    outcome_24h: row.get::<_, Option<String>>(14)?.map(|s| parse_outcome(&s)),
                })
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default();

        predictions
    }

    /// Get all predictions across all symbols (for loading on startup).
    pub fn get_all_predictions(&self, limit: usize) -> Vec<SignalPrediction> {
        let conn = self.conn.lock().unwrap();

        let query =
            "SELECT id, symbol, indicator, direction, score, price_at_prediction, timestamp,
                            price_after_5m, price_after_1h, price_after_4h, price_after_24h,
                            outcome_5m, outcome_1h, outcome_4h, outcome_24h
                     FROM prediction_history
                     ORDER BY timestamp DESC
                     LIMIT ?1";

        let mut stmt = match conn.prepare(query) {
            Ok(stmt) => stmt,
            Err(e) => {
                error!("Error preparing all predictions query: {}", e);
                return Vec::new();
            }
        };

        let predictions = stmt
            .query_map(params![limit as i64], |row| {
                let id_str: String = row.get(0)?;
                Ok(SignalPrediction {
                    id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
                    symbol: row.get(1)?,
                    indicator: row.get(2)?,
                    direction: parse_direction(&row.get::<_, String>(3)?),
                    score: row.get(4)?,
                    price_at_prediction: row.get(5)?,
                    timestamp: row.get(6)?,
                    validated: row.get::<_, Option<String>>(14)?.is_some(),
                    price_after_5m: row.get(7)?,
                    price_after_1h: row.get(8)?,
                    price_after_4h: row.get(9)?,
                    price_after_24h: row.get(10)?,
                    outcome_5m: row.get::<_, Option<String>>(11)?.map(|s| parse_outcome(&s)),
                    outcome_1h: row.get::<_, Option<String>>(12)?.map(|s| parse_outcome(&s)),
                    outcome_4h: row.get::<_, Option<String>>(13)?.map(|s| parse_outcome(&s)),
                    outcome_24h: row.get::<_, Option<String>>(14)?.map(|s| parse_outcome(&s)),
                })
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default();

        predictions
    }

    /// Check if connection is available (used by other services).
    pub fn get_connection(&self) -> Option<()> {
        // Just verify the mutex can be locked
        self.conn.lock().ok().map(|_| ())
    }

    /// Get accuracy statistics for a symbol.
    pub fn get_accuracy_stats(&self, symbol: &str, timeframe: &str) -> AccuracyStats {
        let conn = self.conn.lock().unwrap();
        let symbol_lower = symbol.to_lowercase();

        let outcome_col = match timeframe {
            "5m" => "outcome_5m",
            "1h" => "outcome_1h",
            "4h" => "outcome_4h",
            "24h" => "outcome_24h",
            _ => "outcome_1h",
        };

        let query = format!(
            "SELECT
                COUNT(*) as total,
                SUM(CASE WHEN {} = 'correct' THEN 1 ELSE 0 END) as correct,
                SUM(CASE WHEN {} = 'incorrect' THEN 1 ELSE 0 END) as incorrect,
                SUM(CASE WHEN {} = 'neutral' THEN 1 ELSE 0 END) as neutral
             FROM prediction_history
             WHERE symbol = ?1 AND {} IS NOT NULL",
            outcome_col, outcome_col, outcome_col, outcome_col
        );

        let result = conn.query_row(&query, params![symbol_lower], |row| {
            Ok(AccuracyStats {
                total: row.get(0)?,
                correct: row.get(1)?,
                incorrect: row.get(2)?,
                neutral: row.get(3)?,
            })
        });

        result.unwrap_or_default()
    }

    /// Get overall accuracy across all symbols.
    pub fn get_global_accuracy(&self, timeframe: &str) -> AccuracyStats {
        let conn = self.conn.lock().unwrap();

        let outcome_col = match timeframe {
            "5m" => "outcome_5m",
            "1h" => "outcome_1h",
            "4h" => "outcome_4h",
            "24h" => "outcome_24h",
            _ => "outcome_1h",
        };

        let query = format!(
            "SELECT
                COUNT(*) as total,
                SUM(CASE WHEN {} = 'correct' THEN 1 ELSE 0 END) as correct,
                SUM(CASE WHEN {} = 'incorrect' THEN 1 ELSE 0 END) as incorrect,
                SUM(CASE WHEN {} = 'neutral' THEN 1 ELSE 0 END) as neutral
             FROM prediction_history
             WHERE {} IS NOT NULL",
            outcome_col, outcome_col, outcome_col, outcome_col
        );

        let result = conn.query_row(&query, [], |row| {
            Ok(AccuracyStats {
                total: row.get(0)?,
                correct: row.get(1)?,
                incorrect: row.get(2)?,
                neutral: row.get(3)?,
            })
        });

        result.unwrap_or_default()
    }

    /// Get prediction count for a symbol.
    pub fn prediction_count(&self, symbol: &str) -> usize {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM prediction_history WHERE symbol = ?1",
            params![symbol.to_lowercase()],
            |row| row.get(0),
        )
        .unwrap_or(0)
    }

    /// Clean up old predictions (older than days_to_keep).
    pub fn cleanup_old_predictions(&self, days_to_keep: i64) -> Result<usize, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let cutoff = chrono::Utc::now().timestamp_millis() - (days_to_keep * 24 * 60 * 60 * 1000);

        let count = conn.execute(
            "DELETE FROM prediction_history WHERE timestamp < ?1",
            params![cutoff],
        )?;

        if count > 0 {
            info!("Cleaned up {} old predictions", count);
        }

        Ok(count)
    }

    // ========== Portfolio Methods ==========

    /// Create a new portfolio.
    pub fn create_portfolio(&self, portfolio: &Portfolio) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let risk_settings_json = serde_json::to_string(&portfolio.risk_settings).unwrap_or_default();

        conn.execute(
            "INSERT INTO portfolios (
                id, user_id, name, description, base_currency, starting_balance,
                cash_balance, margin_used, margin_available, unrealized_pnl, realized_pnl,
                total_value, cost_basis_method, risk_settings_json, is_competition,
                competition_id, created_at, updated_at, total_trades, winning_trades
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
            params![
                portfolio.id,
                portfolio.user_id,
                portfolio.name,
                portfolio.description,
                portfolio.base_currency,
                portfolio.starting_balance,
                portfolio.cash_balance,
                portfolio.margin_used,
                portfolio.margin_available,
                portfolio.unrealized_pnl,
                portfolio.realized_pnl,
                portfolio.total_value,
                portfolio.cost_basis_method.to_string(),
                risk_settings_json,
                portfolio.is_competition as i32,
                portfolio.competition_id,
                portfolio.created_at,
                portfolio.updated_at,
                portfolio.total_trades,
                portfolio.winning_trades,
            ],
        )?;

        debug!("Created portfolio {} for user {}", portfolio.id, portfolio.user_id);
        Ok(())
    }

    /// Get a portfolio by ID.
    pub fn get_portfolio(&self, id: &str) -> Option<Portfolio> {
        let conn = self.conn.lock().unwrap();

        let result = conn.query_row(
            "SELECT id, user_id, name, description, base_currency, starting_balance,
                    cash_balance, margin_used, margin_available, unrealized_pnl, realized_pnl,
                    total_value, cost_basis_method, risk_settings_json, is_competition,
                    competition_id, created_at, updated_at, total_trades, winning_trades
             FROM portfolios WHERE id = ?1",
            params![id],
            |row| Self::row_to_portfolio(row),
        );

        match result {
            Ok(portfolio) => Some(portfolio),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => {
                error!("Error fetching portfolio: {}", e);
                None
            }
        }
    }

    /// Get all portfolios for a user.
    pub fn get_user_portfolios(&self, user_id: &str) -> Vec<Portfolio> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = match conn.prepare(
            "SELECT id, user_id, name, description, base_currency, starting_balance,
                    cash_balance, margin_used, margin_available, unrealized_pnl, realized_pnl,
                    total_value, cost_basis_method, risk_settings_json, is_competition,
                    competition_id, created_at, updated_at, total_trades, winning_trades
             FROM portfolios WHERE user_id = ?1 ORDER BY created_at DESC",
        ) {
            Ok(stmt) => stmt,
            Err(e) => {
                error!("Error preparing portfolio query: {}", e);
                return Vec::new();
            }
        };

        stmt.query_map(params![user_id], |row| Self::row_to_portfolio(row))
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }

    /// Update a portfolio.
    pub fn update_portfolio(&self, portfolio: &Portfolio) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let risk_settings_json = serde_json::to_string(&portfolio.risk_settings).unwrap_or_default();

        conn.execute(
            "UPDATE portfolios SET
                name = ?1, description = ?2, cash_balance = ?3, margin_used = ?4,
                margin_available = ?5, unrealized_pnl = ?6, realized_pnl = ?7,
                total_value = ?8, cost_basis_method = ?9, risk_settings_json = ?10,
                updated_at = ?11, total_trades = ?12, winning_trades = ?13
             WHERE id = ?14",
            params![
                portfolio.name,
                portfolio.description,
                portfolio.cash_balance,
                portfolio.margin_used,
                portfolio.margin_available,
                portfolio.unrealized_pnl,
                portfolio.realized_pnl,
                portfolio.total_value,
                portfolio.cost_basis_method.to_string(),
                risk_settings_json,
                portfolio.updated_at,
                portfolio.total_trades,
                portfolio.winning_trades,
                portfolio.id,
            ],
        )?;

        Ok(())
    }

    /// Delete a portfolio (and all associated data).
    pub fn delete_portfolio(&self, id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        // Delete in order due to foreign keys
        conn.execute("DELETE FROM trades WHERE portfolio_id = ?1", params![id])?;
        conn.execute("DELETE FROM positions WHERE portfolio_id = ?1", params![id])?;
        conn.execute("DELETE FROM orders WHERE portfolio_id = ?1", params![id])?;
        conn.execute("DELETE FROM portfolios WHERE id = ?1", params![id])?;

        info!("Deleted portfolio {}", id);
        Ok(())
    }

    /// Helper to convert a row to a Portfolio.
    fn row_to_portfolio(row: &rusqlite::Row) -> Result<Portfolio, rusqlite::Error> {
        let risk_settings_json: String = row.get(13)?;
        let risk_settings: RiskSettings =
            serde_json::from_str(&risk_settings_json).unwrap_or_default();
        let cost_basis_str: String = row.get(12)?;

        Ok(Portfolio {
            id: row.get(0)?,
            user_id: row.get(1)?,
            name: row.get(2)?,
            description: row.get(3)?,
            base_currency: row.get(4)?,
            starting_balance: row.get(5)?,
            cash_balance: row.get(6)?,
            margin_used: row.get(7)?,
            margin_available: row.get(8)?,
            unrealized_pnl: row.get(9)?,
            realized_pnl: row.get(10)?,
            total_value: row.get(11)?,
            total_trades: row.get(18).unwrap_or(0),
            winning_trades: row.get(19).unwrap_or(0),
            cost_basis_method: parse_cost_basis_method(&cost_basis_str),
            risk_settings,
            is_competition: row.get::<_, i32>(14)? != 0,
            competition_id: row.get(15)?,
            created_at: row.get(16)?,
            updated_at: row.get(17)?,
        })
    }

    // ========== Order Methods ==========

    /// Create a new order.
    pub fn create_order(&self, order: &Order) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let fills_json = serde_json::to_string(&order.fills).unwrap_or_default();

        conn.execute(
            "INSERT INTO orders (
                id, portfolio_id, symbol, asset_class, side, order_type, quantity,
                filled_quantity, price, stop_price, trail_amount, trail_percent,
                time_in_force, status, linked_order_id, bracket_id, leverage,
                fills_json, avg_fill_price, total_fees, client_order_id,
                created_at, updated_at, expires_at, trail_high_price, trail_low_price, bracket_role
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27)",
            params![
                order.id,
                order.portfolio_id,
                order.symbol,
                order.asset_class.to_string(),
                order.side.to_string(),
                order.order_type.to_string(),
                order.quantity,
                order.filled_quantity,
                order.price,
                order.stop_price,
                order.trail_amount,
                order.trail_percent,
                order.time_in_force.to_string(),
                order.status.to_string(),
                order.linked_order_id,
                order.bracket_id,
                order.leverage,
                fills_json,
                order.avg_fill_price,
                order.total_fees,
                order.client_order_id,
                order.created_at,
                order.updated_at,
                order.expires_at,
                order.trail_high_price,
                order.trail_low_price,
                order.bracket_role.as_ref().map(|r| r.to_string()),
            ],
        )?;

        debug!("Created order {} for symbol {}", order.id, order.symbol);
        Ok(())
    }

    /// Get an order by ID.
    pub fn get_order(&self, id: &str) -> Option<Order> {
        let conn = self.conn.lock().unwrap();

        let result = conn.query_row(
            "SELECT id, portfolio_id, symbol, asset_class, side, order_type, quantity,
                    filled_quantity, price, stop_price, trail_amount, trail_percent,
                    time_in_force, status, linked_order_id, bracket_id, leverage,
                    fills_json, avg_fill_price, total_fees, client_order_id,
                    created_at, updated_at, expires_at, trail_high_price, trail_low_price, bracket_role
             FROM orders WHERE id = ?1",
            params![id],
            |row| Self::row_to_order(row),
        );

        match result {
            Ok(order) => Some(order),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => {
                error!("Error fetching order: {}", e);
                None
            }
        }
    }

    /// Get orders for a portfolio with optional status filter.
    pub fn get_portfolio_orders(
        &self,
        portfolio_id: &str,
        status: Option<OrderStatus>,
        limit: usize,
    ) -> Vec<Order> {
        let conn = self.conn.lock().unwrap();

        if let Some(s) = status {
            let mut stmt = match conn.prepare(
                "SELECT id, portfolio_id, symbol, asset_class, side, order_type, quantity,
                        filled_quantity, price, stop_price, trail_amount, trail_percent,
                        time_in_force, status, linked_order_id, bracket_id, leverage,
                        fills_json, avg_fill_price, total_fees, client_order_id,
                        created_at, updated_at, expires_at, trail_high_price, trail_low_price, bracket_role
                 FROM orders WHERE portfolio_id = ?1 AND status = ?2
                 ORDER BY created_at DESC LIMIT ?3",
            ) {
                Ok(stmt) => stmt,
                Err(e) => {
                    error!("Error preparing orders query: {}", e);
                    return Vec::new();
                }
            };

            stmt.query_map(
                params![portfolio_id, s.to_string(), limit as i64],
                |row| Self::row_to_order(row),
            )
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
        } else {
            let mut stmt = match conn.prepare(
                "SELECT id, portfolio_id, symbol, asset_class, side, order_type, quantity,
                        filled_quantity, price, stop_price, trail_amount, trail_percent,
                        time_in_force, status, linked_order_id, bracket_id, leverage,
                        fills_json, avg_fill_price, total_fees, client_order_id,
                        created_at, updated_at, expires_at, trail_high_price, trail_low_price, bracket_role
                 FROM orders WHERE portfolio_id = ?1
                 ORDER BY created_at DESC LIMIT ?2",
            ) {
                Ok(stmt) => stmt,
                Err(e) => {
                    error!("Error preparing orders query: {}", e);
                    return Vec::new();
                }
            };

            stmt.query_map(params![portfolio_id, limit as i64], |row| {
                Self::row_to_order(row)
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
        }
    }

    /// Get open orders (pending, open, partially_filled).
    pub fn get_open_orders(&self, portfolio_id: &str) -> Vec<Order> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = match conn.prepare(
            "SELECT id, portfolio_id, symbol, asset_class, side, order_type, quantity,
                    filled_quantity, price, stop_price, trail_amount, trail_percent,
                    time_in_force, status, linked_order_id, bracket_id, leverage,
                    fills_json, avg_fill_price, total_fees, client_order_id,
                    created_at, updated_at, expires_at, trail_high_price, trail_low_price, bracket_role
             FROM orders WHERE portfolio_id = ?1
             AND status IN ('pending', 'open', 'partially_filled')
             ORDER BY created_at DESC",
        ) {
            Ok(stmt) => stmt,
            Err(e) => {
                error!("Error preparing open orders query: {}", e);
                return Vec::new();
            }
        };

        stmt.query_map(params![portfolio_id], |row| Self::row_to_order(row))
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }

    /// Get ALL open orders across all portfolios.
    /// Used by the market simulation engine to check for triggered orders.
    pub fn get_all_open_orders(&self) -> Vec<Order> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = match conn.prepare(
            "SELECT id, portfolio_id, symbol, asset_class, side, order_type, quantity,
                    filled_quantity, price, stop_price, trail_amount, trail_percent,
                    time_in_force, status, linked_order_id, bracket_id, leverage,
                    fills_json, avg_fill_price, total_fees, client_order_id,
                    created_at, updated_at, expires_at, trail_high_price, trail_low_price, bracket_role
             FROM orders
             WHERE status IN ('pending', 'open', 'partially_filled')
             ORDER BY created_at ASC",
        ) {
            Ok(stmt) => stmt,
            Err(e) => {
                error!("Error preparing all open orders query: {}", e);
                return Vec::new();
            }
        };

        stmt.query_map([], |row| Self::row_to_order(row))
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }

    /// Get all unique symbols that have open positions.
    /// Used by the market simulation engine to update position prices.
    pub fn get_symbols_with_positions(&self) -> Vec<String> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = match conn.prepare(
            "SELECT DISTINCT symbol FROM positions WHERE closed_at IS NULL",
        ) {
            Ok(stmt) => stmt,
            Err(e) => {
                error!("Error preparing symbols query: {}", e);
                return Vec::new();
            }
        };

        stmt.query_map([], |row| row.get(0))
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }

    /// Get all unique symbols that have open orders.
    pub fn get_symbols_with_orders(&self) -> Vec<String> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = match conn.prepare(
            "SELECT DISTINCT symbol FROM orders WHERE status IN ('pending', 'open', 'partially_filled')",
        ) {
            Ok(stmt) => stmt,
            Err(e) => {
                error!("Error preparing order symbols query: {}", e);
                return Vec::new();
            }
        };

        stmt.query_map([], |row| row.get(0))
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }

    /// Update an order.
    pub fn update_order(&self, order: &Order) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let fills_json = serde_json::to_string(&order.fills).unwrap_or_default();

        conn.execute(
            "UPDATE orders SET
                filled_quantity = ?1, status = ?2, fills_json = ?3,
                avg_fill_price = ?4, total_fees = ?5, updated_at = ?6,
                stop_price = ?7, trail_high_price = ?8, trail_low_price = ?9,
                linked_order_id = ?10
             WHERE id = ?11",
            params![
                order.filled_quantity,
                order.status.to_string(),
                fills_json,
                order.avg_fill_price,
                order.total_fees,
                order.updated_at,
                order.stop_price,
                order.trail_high_price,
                order.trail_low_price,
                order.linked_order_id,
                order.id,
            ],
        )?;

        Ok(())
    }

    /// Helper to convert a row to an Order.
    fn row_to_order(row: &rusqlite::Row) -> Result<Order, rusqlite::Error> {
        let fills_json: String = row.get(17)?;
        let fills: Vec<Fill> = serde_json::from_str(&fills_json).unwrap_or_default();

        Ok(Order {
            id: row.get(0)?,
            portfolio_id: row.get(1)?,
            symbol: row.get(2)?,
            asset_class: parse_asset_class(&row.get::<_, String>(3)?),
            side: parse_order_side(&row.get::<_, String>(4)?),
            order_type: parse_order_type(&row.get::<_, String>(5)?),
            quantity: row.get(6)?,
            filled_quantity: row.get(7)?,
            price: row.get(8)?,
            stop_price: row.get(9)?,
            trail_amount: row.get(10)?,
            trail_percent: row.get(11)?,
            time_in_force: parse_time_in_force(&row.get::<_, String>(12)?),
            status: parse_order_status(&row.get::<_, String>(13)?),
            linked_order_id: row.get(14)?,
            bracket_id: row.get(15)?,
            leverage: row.get(16)?,
            fills,
            avg_fill_price: row.get(18)?,
            total_fees: row.get(19)?,
            client_order_id: row.get(20)?,
            created_at: row.get(21)?,
            updated_at: row.get(22)?,
            expires_at: row.get(23)?,
            trail_high_price: row.get(24).ok(),
            trail_low_price: row.get(25).ok(),
            bracket_role: row.get::<_, Option<String>>(26)?.map(|s| parse_bracket_role(&s)),
        })
    }

    // ========== Position Methods ==========

    /// Create a new position.
    pub fn create_position(&self, position: &Position) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let cost_basis_json = serde_json::to_string(&position.cost_basis).unwrap_or_default();

        conn.execute(
            "INSERT INTO positions (
                id, portfolio_id, symbol, asset_class, side, quantity, entry_price,
                current_price, unrealized_pnl, unrealized_pnl_pct, realized_pnl,
                margin_used, leverage, margin_mode, liquidation_price, stop_loss,
                take_profit, cost_basis_json, funding_payments, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
            params![
                position.id,
                position.portfolio_id,
                position.symbol,
                position.asset_class.to_string(),
                position.side.to_string(),
                position.quantity,
                position.entry_price,
                position.current_price,
                position.unrealized_pnl,
                position.unrealized_pnl_pct,
                position.realized_pnl,
                position.margin_used,
                position.leverage,
                position.margin_mode.to_string(),
                position.liquidation_price,
                position.stop_loss,
                position.take_profit,
                cost_basis_json,
                position.funding_payments,
                position.created_at,
                position.updated_at,
            ],
        )?;

        debug!("Created position {} for symbol {}", position.id, position.symbol);
        Ok(())
    }

    /// Get a position by ID.
    pub fn get_position(&self, id: &str) -> Option<Position> {
        let conn = self.conn.lock().unwrap();

        let result = conn.query_row(
            "SELECT id, portfolio_id, symbol, asset_class, side, quantity, entry_price,
                    current_price, unrealized_pnl, unrealized_pnl_pct, realized_pnl,
                    margin_used, leverage, margin_mode, liquidation_price, stop_loss,
                    take_profit, cost_basis_json, funding_payments, created_at, updated_at
             FROM positions WHERE id = ?1 AND closed_at IS NULL",
            params![id],
            |row| Self::row_to_position(row),
        );

        match result {
            Ok(position) => Some(position),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => {
                error!("Error fetching position: {}", e);
                None
            }
        }
    }

    /// Get open positions for a portfolio.
    pub fn get_portfolio_positions(&self, portfolio_id: &str) -> Vec<Position> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = match conn.prepare(
            "SELECT id, portfolio_id, symbol, asset_class, side, quantity, entry_price,
                    current_price, unrealized_pnl, unrealized_pnl_pct, realized_pnl,
                    margin_used, leverage, margin_mode, liquidation_price, stop_loss,
                    take_profit, cost_basis_json, funding_payments, created_at, updated_at
             FROM positions WHERE portfolio_id = ?1 AND closed_at IS NULL
             ORDER BY created_at DESC",
        ) {
            Ok(stmt) => stmt,
            Err(e) => {
                error!("Error preparing positions query: {}", e);
                return Vec::new();
            }
        };

        stmt.query_map(params![portfolio_id], |row| Self::row_to_position(row))
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }

    /// Get position for a specific symbol in a portfolio.
    pub fn get_position_by_symbol(
        &self,
        portfolio_id: &str,
        symbol: &str,
        side: PositionSide,
    ) -> Option<Position> {
        let conn = self.conn.lock().unwrap();

        let result = conn.query_row(
            "SELECT id, portfolio_id, symbol, asset_class, side, quantity, entry_price,
                    current_price, unrealized_pnl, unrealized_pnl_pct, realized_pnl,
                    margin_used, leverage, margin_mode, liquidation_price, stop_loss,
                    take_profit, cost_basis_json, funding_payments, created_at, updated_at
             FROM positions
             WHERE portfolio_id = ?1 AND symbol = ?2 AND side = ?3 AND closed_at IS NULL",
            params![portfolio_id, symbol, side.to_string()],
            |row| Self::row_to_position(row),
        );

        match result {
            Ok(position) => Some(position),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => {
                error!("Error fetching position by symbol: {}", e);
                None
            }
        }
    }

    /// Update a position.
    pub fn update_position(&self, position: &Position) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let cost_basis_json = serde_json::to_string(&position.cost_basis).unwrap_or_default();

        conn.execute(
            "UPDATE positions SET
                quantity = ?1, current_price = ?2, unrealized_pnl = ?3,
                unrealized_pnl_pct = ?4, realized_pnl = ?5, margin_used = ?6,
                stop_loss = ?7, take_profit = ?8, cost_basis_json = ?9,
                funding_payments = ?10, updated_at = ?11
             WHERE id = ?12",
            params![
                position.quantity,
                position.current_price,
                position.unrealized_pnl,
                position.unrealized_pnl_pct,
                position.realized_pnl,
                position.margin_used,
                position.stop_loss,
                position.take_profit,
                cost_basis_json,
                position.funding_payments,
                position.updated_at,
                position.id,
            ],
        )?;

        Ok(())
    }

    /// Close a position.
    pub fn close_position(&self, position_id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().timestamp_millis();

        conn.execute(
            "UPDATE positions SET closed_at = ?1, updated_at = ?1 WHERE id = ?2",
            params![now, position_id],
        )?;

        debug!("Closed position {}", position_id);
        Ok(())
    }

    /// Helper to convert a row to a Position.
    fn row_to_position(row: &rusqlite::Row) -> Result<Position, rusqlite::Error> {
        let cost_basis_json: String = row.get(17)?;
        let cost_basis = serde_json::from_str(&cost_basis_json).unwrap_or_default();

        Ok(Position {
            id: row.get(0)?,
            portfolio_id: row.get(1)?,
            symbol: row.get(2)?,
            asset_class: parse_asset_class(&row.get::<_, String>(3)?),
            side: parse_position_side(&row.get::<_, String>(4)?),
            quantity: row.get(5)?,
            entry_price: row.get(6)?,
            current_price: row.get(7)?,
            unrealized_pnl: row.get(8)?,
            unrealized_pnl_pct: row.get(9)?,
            realized_pnl: row.get(10)?,
            margin_used: row.get(11)?,
            leverage: row.get(12)?,
            margin_mode: parse_margin_mode(&row.get::<_, String>(13)?),
            liquidation_price: row.get(14)?,
            stop_loss: row.get(15)?,
            take_profit: row.get(16)?,
            cost_basis,
            funding_payments: row.get(18)?,
            created_at: row.get(19)?,
            updated_at: row.get(20)?,
        })
    }

    // ========== Trade Methods ==========

    /// Create a new trade record.
    pub fn create_trade(&self, trade: &Trade) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "INSERT INTO trades (
                id, order_id, portfolio_id, position_id, symbol, asset_class,
                side, quantity, price, fee, slippage, realized_pnl, executed_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                trade.id,
                trade.order_id,
                trade.portfolio_id,
                trade.position_id,
                trade.symbol,
                trade.asset_class.to_string(),
                trade.side.to_string(),
                trade.quantity,
                trade.price,
                trade.fee,
                trade.slippage,
                trade.realized_pnl,
                trade.executed_at,
            ],
        )?;

        debug!("Created trade {} for order {}", trade.id, trade.order_id);
        Ok(())
    }

    /// Get trades for a portfolio.
    pub fn get_portfolio_trades(&self, portfolio_id: &str, limit: usize) -> Vec<Trade> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = match conn.prepare(
            "SELECT id, order_id, portfolio_id, position_id, symbol, asset_class,
                    side, quantity, price, fee, slippage, realized_pnl, executed_at
             FROM trades WHERE portfolio_id = ?1
             ORDER BY executed_at DESC LIMIT ?2",
        ) {
            Ok(stmt) => stmt,
            Err(e) => {
                error!("Error preparing trades query: {}", e);
                return Vec::new();
            }
        };

        stmt.query_map(params![portfolio_id, limit as i64], |row| {
            Self::row_to_trade(row)
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    /// Get trades for an order.
    pub fn get_order_trades(&self, order_id: &str) -> Vec<Trade> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = match conn.prepare(
            "SELECT id, order_id, portfolio_id, position_id, symbol, asset_class,
                    side, quantity, price, fee, slippage, realized_pnl, executed_at
             FROM trades WHERE order_id = ?1 ORDER BY executed_at ASC",
        ) {
            Ok(stmt) => stmt,
            Err(e) => {
                error!("Error preparing order trades query: {}", e);
                return Vec::new();
            }
        };

        stmt.query_map(params![order_id], |row| Self::row_to_trade(row))
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }

    /// Get a single trade by ID.
    pub fn get_trade(&self, id: &str) -> Option<Trade> {
        let conn = self.conn.lock().unwrap();

        conn.query_row(
            "SELECT id, order_id, portfolio_id, position_id, symbol, asset_class,
                    side, quantity, price, fee, slippage, realized_pnl, executed_at
             FROM trades WHERE id = ?1",
            params![id],
            Self::row_to_trade,
        ).ok()
    }

    /// Helper to convert a row to a Trade.
    fn row_to_trade(row: &rusqlite::Row) -> Result<Trade, rusqlite::Error> {
        Ok(Trade {
            id: row.get(0)?,
            order_id: row.get(1)?,
            portfolio_id: row.get(2)?,
            position_id: row.get(3)?,
            symbol: row.get(4)?,
            asset_class: parse_asset_class(&row.get::<_, String>(5)?),
            side: parse_order_side(&row.get::<_, String>(6)?),
            quantity: row.get(7)?,
            price: row.get(8)?,
            fee: row.get(9)?,
            slippage: row.get(10)?,
            realized_pnl: row.get(11)?,
            executed_at: row.get(12)?,
        })
    }

    /// Get total number of open positions for a portfolio.
    pub fn position_count(&self, portfolio_id: &str) -> usize {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM positions WHERE portfolio_id = ?1 AND closed_at IS NULL",
            params![portfolio_id],
            |row| row.get(0),
        )
        .unwrap_or(0)
    }

    /// Get total number of open orders for a portfolio.
    pub fn open_order_count(&self, portfolio_id: &str) -> usize {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM orders WHERE portfolio_id = ?1
             AND status IN ('pending', 'open', 'partially_filled')",
            params![portfolio_id],
            |row| row.get(0),
        )
        .unwrap_or(0)
    }

    // ========== Funding Payment Methods ==========

    /// Create a funding payment record.
    pub fn create_funding_payment(&self, payment: &FundingPayment) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "INSERT INTO funding_payments (
                id, position_id, portfolio_id, symbol, position_size,
                side, funding_rate, payment, paid_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                payment.id,
                payment.position_id,
                payment.portfolio_id,
                payment.symbol,
                payment.position_size,
                payment.side.to_string(),
                payment.funding_rate,
                payment.payment,
                payment.paid_at,
            ],
        )?;

        debug!(
            "Created funding payment {} for position {}",
            payment.id, payment.position_id
        );
        Ok(())
    }

    /// Get funding payments for a position.
    pub fn get_position_funding_payments(&self, position_id: &str) -> Vec<FundingPayment> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = match conn.prepare(
            "SELECT id, position_id, portfolio_id, symbol, position_size,
                    side, funding_rate, payment, paid_at
             FROM funding_payments WHERE position_id = ?1
             ORDER BY paid_at DESC",
        ) {
            Ok(stmt) => stmt,
            Err(e) => {
                error!("Error preparing funding payments query: {}", e);
                return Vec::new();
            }
        };

        stmt.query_map(params![position_id], |row| Self::row_to_funding_payment(row))
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }

    /// Get funding payments for a portfolio.
    pub fn get_portfolio_funding_payments(
        &self,
        portfolio_id: &str,
        limit: usize,
    ) -> Vec<FundingPayment> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = match conn.prepare(
            "SELECT id, position_id, portfolio_id, symbol, position_size,
                    side, funding_rate, payment, paid_at
             FROM funding_payments WHERE portfolio_id = ?1
             ORDER BY paid_at DESC LIMIT ?2",
        ) {
            Ok(stmt) => stmt,
            Err(e) => {
                error!("Error preparing portfolio funding payments query: {}", e);
                return Vec::new();
            }
        };

        stmt.query_map(params![portfolio_id, limit as i64], |row| {
            Self::row_to_funding_payment(row)
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    /// Helper to convert a row to a FundingPayment.
    fn row_to_funding_payment(row: &rusqlite::Row) -> Result<FundingPayment, rusqlite::Error> {
        Ok(FundingPayment {
            id: row.get(0)?,
            position_id: row.get(1)?,
            portfolio_id: row.get(2)?,
            symbol: row.get(3)?,
            position_size: row.get(4)?,
            side: parse_position_side(&row.get::<_, String>(5)?),
            funding_rate: row.get(6)?,
            payment: row.get(7)?,
            paid_at: row.get(8)?,
        })
    }

    // ========== Liquidation Methods ==========

    /// Create a liquidation record.
    pub fn create_liquidation(&self, liquidation: &Liquidation) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "INSERT INTO liquidations (
                id, position_id, portfolio_id, symbol, quantity,
                liquidation_price, mark_price, loss, liquidation_fee,
                is_partial, remaining_quantity, liquidated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                liquidation.id,
                liquidation.position_id,
                liquidation.portfolio_id,
                liquidation.symbol,
                liquidation.quantity,
                liquidation.liquidation_price,
                liquidation.mark_price,
                liquidation.loss,
                liquidation.liquidation_fee,
                liquidation.is_partial as i32,
                liquidation.remaining_quantity,
                liquidation.liquidated_at,
            ],
        )?;

        debug!(
            "Created liquidation {} for position {}",
            liquidation.id, liquidation.position_id
        );
        Ok(())
    }

    /// Get liquidations for a portfolio.
    pub fn get_portfolio_liquidations(&self, portfolio_id: &str, limit: usize) -> Vec<Liquidation> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = match conn.prepare(
            "SELECT id, position_id, portfolio_id, symbol, quantity,
                    liquidation_price, mark_price, loss, liquidation_fee,
                    is_partial, remaining_quantity, liquidated_at
             FROM liquidations WHERE portfolio_id = ?1
             ORDER BY liquidated_at DESC LIMIT ?2",
        ) {
            Ok(stmt) => stmt,
            Err(e) => {
                error!("Error preparing liquidations query: {}", e);
                return Vec::new();
            }
        };

        stmt.query_map(params![portfolio_id, limit as i64], |row| {
            Self::row_to_liquidation(row)
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    /// Get a liquidation by ID.
    pub fn get_liquidation(&self, id: &str) -> Option<Liquidation> {
        let conn = self.conn.lock().unwrap();

        let result = conn.query_row(
            "SELECT id, position_id, portfolio_id, symbol, quantity,
                    liquidation_price, mark_price, loss, liquidation_fee,
                    is_partial, remaining_quantity, liquidated_at
             FROM liquidations WHERE id = ?1",
            params![id],
            |row| Self::row_to_liquidation(row),
        );

        match result {
            Ok(liq) => Some(liq),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => {
                error!("Error fetching liquidation: {}", e);
                None
            }
        }
    }

    /// Helper to convert a row to a Liquidation.
    fn row_to_liquidation(row: &rusqlite::Row) -> Result<Liquidation, rusqlite::Error> {
        Ok(Liquidation {
            id: row.get(0)?,
            position_id: row.get(1)?,
            portfolio_id: row.get(2)?,
            symbol: row.get(3)?,
            quantity: row.get(4)?,
            liquidation_price: row.get(5)?,
            mark_price: row.get(6)?,
            loss: row.get(7)?,
            liquidation_fee: row.get(8)?,
            is_partial: row.get::<_, i32>(9)? != 0,
            remaining_quantity: row.get(10)?,
            liquidated_at: row.get(11)?,
        })
    }

    // ========== Margin History Methods ==========

    /// Create a margin history entry.
    pub fn create_margin_history(&self, entry: &MarginHistory) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "INSERT INTO margin_history (
                id, portfolio_id, position_id, change_type,
                previous_margin_level, new_margin_level,
                previous_margin_used, new_margin_used,
                amount_changed, reason, timestamp
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                entry.id,
                entry.portfolio_id,
                entry.position_id,
                entry.change_type.to_string(),
                entry.previous_margin_level,
                entry.new_margin_level,
                entry.previous_margin_used,
                entry.new_margin_used,
                entry.amount_changed,
                entry.reason,
                entry.timestamp,
            ],
        )?;

        debug!("Created margin history entry {} for portfolio {}", entry.id, entry.portfolio_id);
        Ok(())
    }

    /// Get margin history for a portfolio.
    pub fn get_portfolio_margin_history(
        &self,
        portfolio_id: &str,
        limit: usize,
    ) -> Vec<MarginHistory> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = match conn.prepare(
            "SELECT id, portfolio_id, position_id, change_type,
                    previous_margin_level, new_margin_level,
                    previous_margin_used, new_margin_used,
                    amount_changed, reason, timestamp
             FROM margin_history WHERE portfolio_id = ?1
             ORDER BY timestamp DESC LIMIT ?2",
        ) {
            Ok(stmt) => stmt,
            Err(e) => {
                error!("Error preparing margin history query: {}", e);
                return Vec::new();
            }
        };

        stmt.query_map(params![portfolio_id, limit as i64], |row| {
            Self::row_to_margin_history(row)
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    /// Helper to convert a row to a MarginHistory.
    fn row_to_margin_history(row: &rusqlite::Row) -> Result<MarginHistory, rusqlite::Error> {
        Ok(MarginHistory {
            id: row.get(0)?,
            portfolio_id: row.get(1)?,
            position_id: row.get(2)?,
            change_type: parse_margin_change_type(&row.get::<_, String>(3)?),
            previous_margin_level: row.get(4)?,
            new_margin_level: row.get(5)?,
            previous_margin_used: row.get(6)?,
            new_margin_used: row.get(7)?,
            amount_changed: row.get(8)?,
            reason: row.get(9)?,
            timestamp: row.get(10)?,
        })
    }

    // ========== Insurance Fund Methods ==========

    /// Get the insurance fund state.
    pub fn get_insurance_fund(&self) -> InsuranceFund {
        let conn = self.conn.lock().unwrap();

        let result = conn.query_row(
            "SELECT balance, total_contributions, total_payouts, liquidations_covered, updated_at
             FROM insurance_fund WHERE id = 1",
            [],
            |row| {
                Ok(InsuranceFund {
                    balance: row.get(0)?,
                    total_contributions: row.get(1)?,
                    total_payouts: row.get(2)?,
                    liquidations_covered: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            },
        );

        match result {
            Ok(fund) => fund,
            Err(e) => {
                error!("Error fetching insurance fund: {}", e);
                InsuranceFund::default()
            }
        }
    }

    /// Update the insurance fund state.
    pub fn update_insurance_fund(&self, fund: &InsuranceFund) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "UPDATE insurance_fund SET
                balance = ?1,
                total_contributions = ?2,
                total_payouts = ?3,
                liquidations_covered = ?4,
                updated_at = ?5
             WHERE id = 1",
            params![
                fund.balance,
                fund.total_contributions,
                fund.total_payouts,
                fund.liquidations_covered,
                fund.updated_at,
            ],
        )?;

        debug!("Updated insurance fund, balance: {}", fund.balance);
        Ok(())
    }

    /// Add a contribution to the insurance fund (from liquidation fee).
    pub fn add_insurance_contribution(&self, amount: f64) -> Result<InsuranceFund, rusqlite::Error> {
        let mut fund = self.get_insurance_fund();
        fund.add_contribution(amount);
        self.update_insurance_fund(&fund)?;
        Ok(fund)
    }

    /// Cover a loss from the insurance fund.
    /// Returns the actual amount covered (may be less if fund is insufficient).
    pub fn cover_loss_from_insurance(&self, loss: f64) -> Result<f64, rusqlite::Error> {
        let mut fund = self.get_insurance_fund();
        let covered = fund.cover_loss(loss);
        self.update_insurance_fund(&fund)?;
        Ok(covered)
    }

    // ========== Options Position Methods ==========

    /// Create a new option position.
    pub fn create_option_position(&self, position: &OptionPosition) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let greeks_json = serde_json::to_string(&position.greeks).unwrap_or_default();

        conn.execute(
            "INSERT INTO options_positions (
                id, portfolio_id, contract_symbol, underlying_symbol, option_type,
                strike, expiration, style, contracts, multiplier, entry_premium,
                current_premium, underlying_price, unrealized_pnl, realized_pnl,
                greeks_json, entry_iv, current_iv, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
            params![
                position.id,
                position.portfolio_id,
                position.contract_symbol,
                position.underlying_symbol,
                position.option_type.to_string(),
                position.strike,
                position.expiration,
                position.style.to_string(),
                position.contracts,
                position.multiplier,
                position.entry_premium,
                position.current_premium,
                position.underlying_price,
                position.unrealized_pnl,
                position.realized_pnl,
                greeks_json,
                position.entry_iv,
                position.current_iv,
                position.created_at,
                position.updated_at,
            ],
        )?;

        debug!(
            "Created option position {} for contract {}",
            position.id, position.contract_symbol
        );
        Ok(())
    }

    /// Get an option position by ID.
    pub fn get_option_position(&self, id: &str) -> Option<OptionPosition> {
        let conn = self.conn.lock().unwrap();

        let result = conn.query_row(
            "SELECT id, portfolio_id, contract_symbol, underlying_symbol, option_type,
                    strike, expiration, style, contracts, multiplier, entry_premium,
                    current_premium, underlying_price, unrealized_pnl, realized_pnl,
                    greeks_json, entry_iv, current_iv, created_at, updated_at
             FROM options_positions WHERE id = ?1 AND closed_at IS NULL",
            params![id],
            |row| Self::row_to_option_position(row),
        );

        match result {
            Ok(position) => Some(position),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => {
                error!("Error fetching option position: {}", e);
                None
            }
        }
    }

    /// Get open option positions for a portfolio.
    pub fn get_portfolio_option_positions(&self, portfolio_id: &str) -> Vec<OptionPosition> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = match conn.prepare(
            "SELECT id, portfolio_id, contract_symbol, underlying_symbol, option_type,
                    strike, expiration, style, contracts, multiplier, entry_premium,
                    current_premium, underlying_price, unrealized_pnl, realized_pnl,
                    greeks_json, entry_iv, current_iv, created_at, updated_at
             FROM options_positions WHERE portfolio_id = ?1 AND closed_at IS NULL
             ORDER BY expiration ASC",
        ) {
            Ok(stmt) => stmt,
            Err(e) => {
                error!("Error preparing option positions query: {}", e);
                return Vec::new();
            }
        };

        stmt.query_map(params![portfolio_id], |row| Self::row_to_option_position(row))
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }

    /// Get option positions for a specific underlying symbol.
    pub fn get_option_positions_by_underlying(
        &self,
        portfolio_id: &str,
        underlying_symbol: &str,
    ) -> Vec<OptionPosition> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = match conn.prepare(
            "SELECT id, portfolio_id, contract_symbol, underlying_symbol, option_type,
                    strike, expiration, style, contracts, multiplier, entry_premium,
                    current_premium, underlying_price, unrealized_pnl, realized_pnl,
                    greeks_json, entry_iv, current_iv, created_at, updated_at
             FROM options_positions
             WHERE portfolio_id = ?1 AND underlying_symbol = ?2 AND closed_at IS NULL
             ORDER BY expiration ASC, strike ASC",
        ) {
            Ok(stmt) => stmt,
            Err(e) => {
                error!("Error preparing option positions by underlying query: {}", e);
                return Vec::new();
            }
        };

        stmt.query_map(params![portfolio_id, underlying_symbol], |row| {
            Self::row_to_option_position(row)
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    /// Get option positions expiring before a given date.
    pub fn get_expiring_option_positions(
        &self,
        portfolio_id: &str,
        before_timestamp: i64,
    ) -> Vec<OptionPosition> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = match conn.prepare(
            "SELECT id, portfolio_id, contract_symbol, underlying_symbol, option_type,
                    strike, expiration, style, contracts, multiplier, entry_premium,
                    current_premium, underlying_price, unrealized_pnl, realized_pnl,
                    greeks_json, entry_iv, current_iv, created_at, updated_at
             FROM options_positions
             WHERE portfolio_id = ?1 AND expiration < ?2 AND closed_at IS NULL
             ORDER BY expiration ASC",
        ) {
            Ok(stmt) => stmt,
            Err(e) => {
                error!("Error preparing expiring options query: {}", e);
                return Vec::new();
            }
        };

        stmt.query_map(params![portfolio_id, before_timestamp], |row| {
            Self::row_to_option_position(row)
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    /// Update an option position.
    pub fn update_option_position(&self, position: &OptionPosition) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let greeks_json = serde_json::to_string(&position.greeks).unwrap_or_default();

        conn.execute(
            "UPDATE options_positions SET
                contracts = ?1, current_premium = ?2, underlying_price = ?3,
                unrealized_pnl = ?4, realized_pnl = ?5, greeks_json = ?6,
                current_iv = ?7, updated_at = ?8
             WHERE id = ?9",
            params![
                position.contracts,
                position.current_premium,
                position.underlying_price,
                position.unrealized_pnl,
                position.realized_pnl,
                greeks_json,
                position.current_iv,
                position.updated_at,
                position.id,
            ],
        )?;

        Ok(())
    }

    /// Close an option position.
    pub fn close_option_position(&self, position_id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().timestamp_millis();

        conn.execute(
            "UPDATE options_positions SET closed_at = ?1, updated_at = ?1 WHERE id = ?2",
            params![now, position_id],
        )?;

        debug!("Closed option position {}", position_id);
        Ok(())
    }

    /// Get count of open option positions for a portfolio.
    pub fn option_position_count(&self, portfolio_id: &str) -> usize {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM options_positions WHERE portfolio_id = ?1 AND closed_at IS NULL",
            params![portfolio_id],
            |row| row.get(0),
        )
        .unwrap_or(0)
    }

    /// Helper to convert a row to an OptionPosition.
    fn row_to_option_position(row: &rusqlite::Row) -> Result<OptionPosition, rusqlite::Error> {
        let greeks_json: String = row.get(15)?;
        let greeks: Greeks = serde_json::from_str(&greeks_json).unwrap_or_default();

        Ok(OptionPosition {
            id: row.get(0)?,
            portfolio_id: row.get(1)?,
            contract_symbol: row.get(2)?,
            underlying_symbol: row.get(3)?,
            option_type: parse_option_type(&row.get::<_, String>(4)?),
            strike: row.get(5)?,
            expiration: row.get(6)?,
            style: parse_option_style(&row.get::<_, String>(7)?),
            contracts: row.get(8)?,
            multiplier: row.get(9)?,
            entry_premium: row.get(10)?,
            current_premium: row.get(11)?,
            underlying_price: row.get(12)?,
            unrealized_pnl: row.get(13)?,
            realized_pnl: row.get(14)?,
            greeks,
            entry_iv: row.get(16)?,
            current_iv: row.get(17)?,
            created_at: row.get(18)?,
            updated_at: row.get(19)?,
        })
    }

    // ========== Strategy Methods ==========

    /// Create a new trading strategy.
    pub fn create_strategy(&self, strategy: &TradingStrategy) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let symbols_json = serde_json::to_string(&strategy.symbols).unwrap_or_default();
        let rules_json = serde_json::to_string(&strategy.rules).unwrap_or_default();

        conn.execute(
            "INSERT INTO strategies (
                id, portfolio_id, name, description, symbols_json, asset_class,
                rules_json, status, cooldown_seconds, max_positions, max_position_size_pct,
                last_trade_at, total_trades, winning_trades, losing_trades, realized_pnl,
                created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            params![
                strategy.id,
                strategy.portfolio_id,
                strategy.name,
                strategy.description,
                symbols_json,
                strategy.asset_class.as_ref().map(|c| c.to_string()),
                rules_json,
                strategy.status.to_string(),
                strategy.cooldown_seconds,
                strategy.max_positions,
                strategy.max_position_size_pct,
                strategy.last_trade_at,
                strategy.total_trades,
                strategy.winning_trades,
                strategy.losing_trades,
                strategy.realized_pnl,
                strategy.created_at,
                strategy.updated_at,
            ],
        )?;

        debug!("Created strategy {} for portfolio {}", strategy.id, strategy.portfolio_id);
        Ok(())
    }

    /// Get a strategy by ID.
    pub fn get_strategy(&self, id: &str) -> Option<TradingStrategy> {
        let conn = self.conn.lock().unwrap();

        let result = conn.query_row(
            "SELECT id, portfolio_id, name, description, symbols_json, asset_class,
                    rules_json, status, cooldown_seconds, max_positions, max_position_size_pct,
                    last_trade_at, total_trades, winning_trades, losing_trades, realized_pnl,
                    created_at, updated_at
             FROM strategies WHERE id = ?1",
            params![id],
            |row| Self::row_to_strategy(row),
        );

        match result {
            Ok(strategy) => Some(strategy),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => {
                error!("Error fetching strategy: {}", e);
                None
            }
        }
    }

    /// Get all strategies for a portfolio.
    pub fn get_portfolio_strategies(&self, portfolio_id: &str) -> Vec<TradingStrategy> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = match conn.prepare(
            "SELECT id, portfolio_id, name, description, symbols_json, asset_class,
                    rules_json, status, cooldown_seconds, max_positions, max_position_size_pct,
                    last_trade_at, total_trades, winning_trades, losing_trades, realized_pnl,
                    created_at, updated_at
             FROM strategies WHERE portfolio_id = ?1 AND status != 'deleted'
             ORDER BY created_at DESC",
        ) {
            Ok(stmt) => stmt,
            Err(e) => {
                error!("Error preparing strategies query: {}", e);
                return Vec::new();
            }
        };

        stmt.query_map(params![portfolio_id], |row| Self::row_to_strategy(row))
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }

    /// Get all active strategies for a portfolio.
    pub fn get_active_strategies(&self, portfolio_id: &str) -> Vec<TradingStrategy> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = match conn.prepare(
            "SELECT id, portfolio_id, name, description, symbols_json, asset_class,
                    rules_json, status, cooldown_seconds, max_positions, max_position_size_pct,
                    last_trade_at, total_trades, winning_trades, losing_trades, realized_pnl,
                    created_at, updated_at
             FROM strategies WHERE portfolio_id = ?1 AND status = 'active'
             ORDER BY created_at DESC",
        ) {
            Ok(stmt) => stmt,
            Err(e) => {
                error!("Error preparing active strategies query: {}", e);
                return Vec::new();
            }
        };

        stmt.query_map(params![portfolio_id], |row| Self::row_to_strategy(row))
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }

    /// Update a strategy.
    pub fn update_strategy(&self, strategy: &TradingStrategy) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let symbols_json = serde_json::to_string(&strategy.symbols).unwrap_or_default();
        let rules_json = serde_json::to_string(&strategy.rules).unwrap_or_default();

        conn.execute(
            "UPDATE strategies SET
                name = ?1, description = ?2, symbols_json = ?3, asset_class = ?4,
                rules_json = ?5, status = ?6, cooldown_seconds = ?7, max_positions = ?8,
                max_position_size_pct = ?9, last_trade_at = ?10, total_trades = ?11,
                winning_trades = ?12, losing_trades = ?13, realized_pnl = ?14, updated_at = ?15
             WHERE id = ?16",
            params![
                strategy.name,
                strategy.description,
                symbols_json,
                strategy.asset_class.as_ref().map(|c| c.to_string()),
                rules_json,
                strategy.status.to_string(),
                strategy.cooldown_seconds,
                strategy.max_positions,
                strategy.max_position_size_pct,
                strategy.last_trade_at,
                strategy.total_trades,
                strategy.winning_trades,
                strategy.losing_trades,
                strategy.realized_pnl,
                strategy.updated_at,
                strategy.id,
            ],
        )?;

        Ok(())
    }

    /// Delete a strategy (soft delete by setting status to deleted).
    pub fn delete_strategy(&self, strategy_id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().timestamp_millis();

        conn.execute(
            "UPDATE strategies SET status = 'deleted', updated_at = ?1 WHERE id = ?2",
            params![now, strategy_id],
        )?;

        debug!("Deleted strategy {}", strategy_id);
        Ok(())
    }

    /// Get count of strategies for a portfolio.
    pub fn strategy_count(&self, portfolio_id: &str) -> usize {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM strategies WHERE portfolio_id = ?1 AND status != 'deleted'",
            params![portfolio_id],
            |row| row.get(0),
        )
        .unwrap_or(0)
    }

    /// Helper to convert a row to a TradingStrategy.
    fn row_to_strategy(row: &rusqlite::Row) -> Result<TradingStrategy, rusqlite::Error> {
        let symbols_json: String = row.get(4)?;
        let symbols: Vec<String> = serde_json::from_str(&symbols_json).unwrap_or_default();
        let rules_json: String = row.get(6)?;
        let rules: Vec<TradingRule> = serde_json::from_str(&rules_json).unwrap_or_default();
        let asset_class_str: Option<String> = row.get(5)?;

        Ok(TradingStrategy {
            id: row.get(0)?,
            portfolio_id: row.get(1)?,
            name: row.get(2)?,
            description: row.get(3)?,
            symbols,
            asset_class: asset_class_str.map(|s| parse_asset_class(&s)),
            rules,
            status: parse_strategy_status(&row.get::<_, String>(7)?),
            cooldown_seconds: row.get(8)?,
            max_positions: row.get(9)?,
            max_position_size_pct: row.get(10)?,
            last_trade_at: row.get(11)?,
            total_trades: row.get(12)?,
            winning_trades: row.get(13)?,
            losing_trades: row.get(14)?,
            realized_pnl: row.get(15)?,
            created_at: row.get(16)?,
            updated_at: row.get(17)?,
        })
    }

    // ========== Portfolio Snapshot Methods ==========

    /// Create a portfolio snapshot for equity curve charting.
    pub fn create_portfolio_snapshot(
        &self,
        portfolio_id: &str,
        equity: f64,
        cash: f64,
        positions_value: f64,
        realized_pnl: f64,
        unrealized_pnl: f64,
        drawdown_pct: f64,
        peak_equity: f64,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let id = Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().timestamp_millis();

        conn.execute(
            "INSERT INTO portfolio_snapshots (
                id, portfolio_id, timestamp, equity, cash, positions_value,
                realized_pnl, unrealized_pnl, drawdown_pct, peak_equity
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                id,
                portfolio_id,
                timestamp,
                equity,
                cash,
                positions_value,
                realized_pnl,
                unrealized_pnl,
                drawdown_pct,
                peak_equity,
            ],
        )?;

        debug!(
            "Created portfolio snapshot for {} - equity: {:.2}",
            portfolio_id, equity
        );
        Ok(())
    }

    /// Create a portfolio snapshot from a Portfolio object.
    pub fn create_snapshot_from_portfolio(
        &self,
        portfolio: &Portfolio,
    ) -> Result<(), rusqlite::Error> {
        let peak_equity = portfolio.starting_balance.max(portfolio.total_value);
        let drawdown_pct = if peak_equity > 0.0 {
            ((peak_equity - portfolio.total_value) / peak_equity) * 100.0
        } else {
            0.0
        };

        self.create_portfolio_snapshot(
            &portfolio.id,
            portfolio.total_value,
            portfolio.cash_balance,
            portfolio.total_value - portfolio.cash_balance,
            portfolio.realized_pnl,
            portfolio.unrealized_pnl,
            drawdown_pct,
            peak_equity,
        )
    }

    /// Get portfolio snapshots for charting (equity curve).
    /// Returns points ordered by timestamp ascending.
    pub fn get_portfolio_snapshots(
        &self,
        portfolio_id: &str,
        since_timestamp: Option<i64>,
        limit: Option<usize>,
    ) -> Vec<EquityPoint> {
        let conn = self.conn.lock().unwrap();

        // Always use the query with since parameter (use 0 if not specified)
        let query = "SELECT timestamp, equity, cash, positions_value, realized_pnl, unrealized_pnl, drawdown_pct
             FROM portfolio_snapshots
             WHERE portfolio_id = ?1 AND timestamp >= ?2
             ORDER BY timestamp ASC
             LIMIT ?3";

        let mut stmt = match conn.prepare(query) {
            Ok(stmt) => stmt,
            Err(e) => {
                error!("Error preparing snapshots query: {}", e);
                return Vec::new();
            }
        };

        let limit_val = limit.unwrap_or(10000) as i64;
        let since = since_timestamp.unwrap_or(0);

        stmt.query_map(params![portfolio_id, since, limit_val], |row| {
            Ok(EquityPoint {
                timestamp: row.get(0)?,
                equity: row.get(1)?,
                cash: row.get(2)?,
                positions_value: row.get(3)?,
                realized_pnl: row.get(4)?,
                unrealized_pnl: row.get(5)?,
                drawdown_pct: row.get(6)?,
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    /// Get the latest snapshot for a portfolio.
    pub fn get_latest_portfolio_snapshot(&self, portfolio_id: &str) -> Option<EquityPoint> {
        let conn = self.conn.lock().unwrap();

        conn.query_row(
            "SELECT timestamp, equity, cash, positions_value, realized_pnl, unrealized_pnl, drawdown_pct
             FROM portfolio_snapshots
             WHERE portfolio_id = ?1
             ORDER BY timestamp DESC
             LIMIT 1",
            params![portfolio_id],
            |row| {
                Ok(EquityPoint {
                    timestamp: row.get(0)?,
                    equity: row.get(1)?,
                    cash: row.get(2)?,
                    positions_value: row.get(3)?,
                    realized_pnl: row.get(4)?,
                    unrealized_pnl: row.get(5)?,
                    drawdown_pct: row.get(6)?,
                })
            },
        )
        .ok()
    }

    /// Delete old snapshots to manage storage (keep last N days).
    pub fn cleanup_old_snapshots(&self, portfolio_id: &str, days_to_keep: i64) -> Result<usize, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let cutoff = chrono::Utc::now().timestamp_millis() - (days_to_keep * 24 * 60 * 60 * 1000);

        let deleted = conn.execute(
            "DELETE FROM portfolio_snapshots WHERE portfolio_id = ?1 AND timestamp < ?2",
            params![portfolio_id, cutoff],
        )?;

        if deleted > 0 {
            debug!(
                "Cleaned up {} old snapshots for portfolio {}",
                deleted, portfolio_id
            );
        }

        Ok(deleted)
    }

    /// Get snapshot count for a portfolio.
    pub fn snapshot_count(&self, portfolio_id: &str) -> usize {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM portfolio_snapshots WHERE portfolio_id = ?1",
            params![portfolio_id],
            |row| row.get(0),
        )
        .unwrap_or(0)
    }

    // ========== Backtest Methods ==========

    /// Create a new backtest result.
    pub fn create_backtest_result(&self, result: &crate::types::BacktestResult) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let config_json = serde_json::to_string(&result.config).unwrap_or_default();
        let metrics_json = serde_json::to_string(&result.metrics).unwrap_or_default();
        let trades_json = serde_json::to_string(&result.trades).unwrap_or_default();
        let equity_json = serde_json::to_string(&result.equity_curve).unwrap_or_default();
        let bnh_json = result.buy_and_hold.as_ref().map(|b| serde_json::to_string(b).unwrap_or_default());
        let mc_json = result.monte_carlo.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default());

        conn.execute(
            "INSERT INTO backtest_results (
                id, strategy_id, status, config_json, metrics_json, trades_json,
                equity_curve_json, buy_and_hold_json, monte_carlo_json, final_balance,
                error_message, created_at, started_at, completed_at, execution_time_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                result.id,
                result.strategy_id,
                format!("{:?}", result.status).to_lowercase(),
                config_json,
                metrics_json,
                trades_json,
                equity_json,
                bnh_json,
                mc_json,
                result.final_balance,
                result.error_message,
                result.created_at,
                result.started_at,
                result.completed_at,
                result.execution_time_ms,
            ],
        )?;

        debug!("Created backtest result {} for strategy {}", result.id, result.strategy_id);
        Ok(())
    }

    /// Get a backtest result by ID.
    pub fn get_backtest_result(&self, id: &str) -> Option<crate::types::BacktestResult> {
        let conn = self.conn.lock().unwrap();

        let result = conn.query_row(
            "SELECT id, strategy_id, status, config_json, metrics_json, trades_json,
                    equity_curve_json, buy_and_hold_json, monte_carlo_json, final_balance,
                    error_message, created_at, started_at, completed_at, execution_time_ms
             FROM backtest_results WHERE id = ?1",
            params![id],
            |row| Self::row_to_backtest_result(row),
        );

        match result {
            Ok(backtest) => Some(backtest),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => {
                error!("Error fetching backtest result: {}", e);
                None
            }
        }
    }

    /// Get backtest results for a strategy.
    pub fn get_strategy_backtests(&self, strategy_id: &str) -> Vec<crate::types::BacktestResult> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = match conn.prepare(
            "SELECT id, strategy_id, status, config_json, metrics_json, trades_json,
                    equity_curve_json, buy_and_hold_json, monte_carlo_json, final_balance,
                    error_message, created_at, started_at, completed_at, execution_time_ms
             FROM backtest_results WHERE strategy_id = ?1
             ORDER BY created_at DESC",
        ) {
            Ok(stmt) => stmt,
            Err(e) => {
                error!("Error preparing backtest results query: {}", e);
                return Vec::new();
            }
        };

        stmt.query_map(params![strategy_id], |row| Self::row_to_backtest_result(row))
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }

    /// Helper to convert a row to a BacktestResult.
    fn row_to_backtest_result(row: &rusqlite::Row) -> Result<crate::types::BacktestResult, rusqlite::Error> {
        let config_json: String = row.get(3)?;
        let metrics_json: String = row.get(4)?;
        let trades_json: String = row.get(5)?;
        let equity_json: String = row.get(6)?;
        let bnh_json: Option<String> = row.get(7)?;
        let mc_json: Option<String> = row.get(8)?;

        Ok(crate::types::BacktestResult {
            id: row.get(0)?,
            strategy_id: row.get(1)?,
            status: parse_backtest_status(&row.get::<_, String>(2)?),
            config: serde_json::from_str(&config_json).unwrap_or_else(|_| {
                crate::types::BacktestConfig::new("".to_string(), 0, 0)
            }),
            metrics: serde_json::from_str(&metrics_json).unwrap_or_default(),
            trades: serde_json::from_str(&trades_json).unwrap_or_default(),
            equity_curve: serde_json::from_str(&equity_json).unwrap_or_default(),
            buy_and_hold: bnh_json.and_then(|j| serde_json::from_str(&j).ok()),
            monte_carlo: mc_json.and_then(|j| serde_json::from_str(&j).ok()),
            final_balance: row.get(9)?,
            error_message: row.get(10)?,
            created_at: row.get(11)?,
            started_at: row.get(12)?,
            completed_at: row.get(13)?,
            execution_time_ms: row.get(14)?,
        })
    }

    // ========== Chart Data Methods ==========

    /// Get chart data (OHLCV candles) for a symbol.
    /// Returns None if no data is available.
    pub fn get_chart_data(
        &self,
        symbol: &str,
        interval: &str,
        start_time: i64,
        end_time: i64,
    ) -> Option<Vec<crate::types::ChartCandle>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = match conn.prepare(
            "SELECT timestamp, open, high, low, close, volume
             FROM chart_candles
             WHERE symbol = ?1 AND interval = ?2 AND timestamp >= ?3 AND timestamp <= ?4
             ORDER BY timestamp ASC",
        ) {
            Ok(stmt) => stmt,
            Err(_) => return None,
        };

        let result: Result<Vec<crate::types::ChartCandle>, _> = stmt.query_map(
            params![symbol, interval, start_time, end_time],
            |row| {
                Ok(crate::types::ChartCandle {
                    timestamp: row.get(0)?,
                    open: row.get(1)?,
                    high: row.get(2)?,
                    low: row.get(3)?,
                    close: row.get(4)?,
                    volume: row.get(5)?,
                })
            },
        ).and_then(|rows| rows.collect());

        match result {
            Ok(candles) if !candles.is_empty() => Some(candles),
            _ => None,
        }
    }

    // ========== Data Sync Methods ==========

    /// Get sync state.
    pub fn get_sync_state(&self) -> Option<crate::types::SyncState> {
        let conn = self.conn.lock().unwrap();
        
        conn.query_row(
            "SELECT last_full_sync_at, last_incremental_sync_at, sync_cursor_position,
                    pending_sync_count, failed_sync_count, total_synced_entities, sync_enabled
             FROM sync_state WHERE id = 1",
            [],
            |row| {
                Ok(crate::types::SyncState {
                    last_full_sync_at: row.get(0)?,
                    last_incremental_sync_at: row.get(1)?,
                    sync_cursor_position: row.get(2)?,
                    pending_sync_count: row.get(3)?,
                    failed_sync_count: row.get(4)?,
                    total_synced_entities: row.get(5)?,
                    sync_enabled: row.get::<_, i64>(6)? != 0,
                })
            },
        ).ok()
    }

    /// Update sync state.
    pub fn update_sync_state(&self, state: &crate::types::SyncState) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute(
            "UPDATE sync_state SET
                last_full_sync_at = ?1,
                last_incremental_sync_at = ?2,
                sync_cursor_position = ?3,
                pending_sync_count = ?4,
                failed_sync_count = ?5,
                total_synced_entities = ?6,
                sync_enabled = ?7
             WHERE id = 1",
            params![
                state.last_full_sync_at,
                state.last_incremental_sync_at,
                state.sync_cursor_position,
                state.pending_sync_count,
                state.failed_sync_count,
                state.total_synced_entities,
                if state.sync_enabled { 1 } else { 0 },
            ],
        )?;
        
        Ok(())
    }

    /// Insert sync queue item.
    pub fn insert_sync_queue_item(&self, item: &crate::types::SyncQueueItem) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        
        let target_nodes_json = item.target_nodes.as_ref()
            .map(|nodes| serde_json::to_string(nodes).unwrap_or_default());
        
        conn.execute(
            "INSERT INTO sync_queue (
                id, entity_type, entity_id, operation, priority, target_nodes,
                retry_count, created_at, scheduled_at, attempted_at, completed_at, error
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                item.id,
                format!("{:?}", item.entity_type).to_lowercase(),
                item.entity_id,
                format!("{:?}", item.operation).to_lowercase(),
                item.priority,
                target_nodes_json,
                item.retry_count,
                item.created_at,
                item.scheduled_at,
                item.attempted_at,
                item.completed_at,
                item.error,
            ],
        )?;
        
        Ok(())
    }

    /// Get pending sync queue items.
    pub fn get_pending_sync_items(&self, limit: u32) -> Result<Vec<crate::types::SyncQueueItem>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        
        let mut stmt = conn.prepare(
            "SELECT id, entity_type, entity_id, operation, priority, target_nodes,
                    retry_count, created_at, scheduled_at, attempted_at, completed_at, error
             FROM sync_queue
             WHERE completed_at IS NULL AND retry_count < 5
             ORDER BY priority ASC, scheduled_at ASC
             LIMIT ?1"
        )?;
        
        let items = stmt.query_map(params![limit], |row| {
            let entity_type_str: String = row.get(1)?;
            let operation_str: String = row.get(3)?;
            let target_nodes_json: Option<String> = row.get(5)?;
            
            let entity_type = parse_entity_type(&entity_type_str);
            let operation = parse_sync_operation(&operation_str);
            let target_nodes = target_nodes_json.and_then(|json| serde_json::from_str(&json).ok());
            
            Ok(crate::types::SyncQueueItem {
                id: row.get(0)?,
                entity_type,
                entity_id: row.get(2)?,
                operation,
                priority: row.get(4)?,
                target_nodes,
                retry_count: row.get(6)?,
                created_at: row.get(7)?,
                scheduled_at: row.get(8)?,
                attempted_at: row.get(9)?,
                completed_at: row.get(10)?,
                error: row.get(11)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        
        Ok(items)
    }

    /// Complete sync queue item.
    pub fn complete_sync_queue_item(&self, id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute(
            "UPDATE sync_queue SET completed_at = ?1 WHERE id = ?2",
            params![chrono::Utc::now().timestamp_millis(), id],
        )?;
        
        Ok(())
    }

    /// Update sync queue item error.
    pub fn update_sync_queue_item_error(&self, id: &str, error: &str, retry_count: u32) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute(
            "UPDATE sync_queue SET error = ?1, retry_count = ?2, attempted_at = ?3 WHERE id = ?4",
            params![error, retry_count, chrono::Utc::now().timestamp_millis(), id],
        )?;
        
        Ok(())
    }

    /// Record sync version.
    pub fn record_sync_version(
        &self,
        entity_type: crate::types::EntityType,
        entity_id: &str,
        node_id: &str,
        version: u64,
        timestamp: i64,
        checksum: &str,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute(
            "INSERT OR REPLACE INTO sync_versions (entity_type, entity_id, node_id, version, timestamp, checksum)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                format!("{:?}", entity_type).to_lowercase(),
                entity_id,
                node_id,
                version as i64,
                timestamp,
                checksum,
            ],
        )?;
        
        Ok(())
    }

    /// Get entity version and timestamp.
    pub fn get_entity_version(&self, entity_type: crate::types::EntityType, entity_id: &str) -> Result<Option<(u64, i64)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let table = entity_type.table_name();
        
        let query = format!("SELECT version, last_modified_at FROM {} WHERE id = ?1", table);
        
        match conn.query_row(&query, params![entity_id], |row| {
            Ok((row.get::<_, i64>(0)? as u64, row.get(1)?))
        }) {
            Ok(version) => Ok(Some(version)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get entity data as JSON bytes.
    pub fn get_entity_data(&self, entity_type: crate::types::EntityType, entity_id: &str) -> Result<Vec<u8>, rusqlite::Error> {
        use crate::types::EntityType;
        
        match entity_type {
            EntityType::Portfolio => {
                if let Some(portfolio) = self.get_portfolio(entity_id) {
                    serde_json::to_vec(&portfolio)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
                } else {
                    Ok(Vec::new())
                }
            }
            EntityType::Order => {
                if let Some(order) = self.get_order(entity_id) {
                    serde_json::to_vec(&order)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
                } else {
                    Ok(Vec::new())
                }
            }
            EntityType::Position => {
                if let Some(position) = self.get_position(entity_id) {
                    serde_json::to_vec(&position)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
                } else {
                    Ok(Vec::new())
                }
            }
            EntityType::Trade => {
                if let Some(trade) = self.get_trade(entity_id) {
                    serde_json::to_vec(&trade)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
                } else {
                    Ok(Vec::new())
                }
            }
            EntityType::Profile => {
                // Get profile by ID (need to query by ID not public key)
                let conn = self.conn.lock().unwrap();
                let result = conn.query_row(
                    "SELECT public_key FROM profiles WHERE id = ?1",
                    params![entity_id],
                    |row| row.get::<_, String>(0),
                );
                
                if let Ok(public_key) = result {
                    if let Some(profile) = self.get_profile(&public_key) {
                        return serde_json::to_vec(&profile)
                            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)));
                    }
                }
                Ok(Vec::new())
            }
            EntityType::Liquidation => {
                if let Some(liquidation) = self.get_liquidation(entity_id) {
                    serde_json::to_vec(&liquidation)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
                } else {
                    Ok(Vec::new())
                }
            }
            EntityType::OptionsPosition => {
                if let Some(position) = self.get_option_position(entity_id) {
                    serde_json::to_vec(&position)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
                } else {
                    Ok(Vec::new())
                }
            }
            EntityType::Strategy => {
                if let Some(strategy) = self.get_strategy(entity_id) {
                    serde_json::to_vec(&strategy)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
                } else {
                    Ok(Vec::new())
                }
            }
            EntityType::InsuranceFund => {
                let fund = self.get_insurance_fund();
                serde_json::to_vec(&fund)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
            }
            // For other entity types, return empty for now
            _ => Ok(Vec::new()),
        }
    }

    /// Update entity from sync data.
    pub fn update_entity_from_sync(
        &self,
        entity_type: crate::types::EntityType,
        entity_id: &str,
        data: &[u8],
        version: u64,
        timestamp: i64,
        node_id: &str,
    ) -> Result<crate::types::SyncUpdateResult, rusqlite::Error> {
        use crate::types::{EntityType, SyncUpdateResult, ConsistencyModel};
        
        let conn = self.conn.lock().unwrap();
        let table = entity_type.table_name();
        let consistency = entity_type.consistency_model();
        
        // Check existing version for conflict detection
        let existing = conn.query_row(
            &format!("SELECT version, last_modified_at, last_modified_by FROM {} WHERE id = ?1", table),
            params![entity_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, String>(2)?))
        ).optional()?;
        
        // Detect conflicts
        if let Some((existing_version, existing_timestamp, existing_node)) = existing {
            let existing_version = existing_version as u64;
            
            // For strong consistency, require version to be sequential
            if consistency == ConsistencyModel::Strong {
                if version <= existing_version {
                    // Version conflict detected
                    return Ok(SyncUpdateResult::Conflict {
                        existing_version,
                        existing_timestamp,
                        existing_node,
                        incoming_version: version,
                        incoming_timestamp: timestamp,
                        incoming_node: node_id.to_string(),
                    });
                }
            } else {
                // For eventual consistency, still detect version conflicts
                if version != existing_version + 1 && version <= existing_version {
                    return Ok(SyncUpdateResult::Conflict {
                        existing_version,
                        existing_timestamp,
                        existing_node,
                        incoming_version: version,
                        incoming_timestamp: timestamp,
                        incoming_node: node_id.to_string(),
                    });
                }
            }
        }
        
        //  Apply update based on entity type
        match entity_type {
            EntityType::Portfolio => {
                let portfolio: crate::types::Portfolio = serde_json::from_slice(data)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Blob, Box::new(e)))?;
                
                // Update portfolio with version tracking
                conn.execute(
                    "INSERT OR REPLACE INTO portfolios (
                        id, user_id, name, description, base_currency,
                        starting_balance, cash_balance, margin_used, margin_available,
                        unrealized_pnl, realized_pnl, total_value,
                        cost_basis_method, risk_settings_json,
                        is_competition, competition_id,
                        created_at, updated_at,
                        total_trades, winning_trades,
                        version, last_modified_at, last_modified_by
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)",
                    params![
                        portfolio.id,
                        portfolio.user_id,
                        portfolio.name,
                        portfolio.description,
                        portfolio.base_currency,
                        portfolio.starting_balance,
                        portfolio.cash_balance,
                        portfolio.margin_used,
                        portfolio.margin_available,
                        portfolio.unrealized_pnl,
                        portfolio.realized_pnl,
                        portfolio.total_value,
                        format!("{:?}", portfolio.cost_basis_method).to_lowercase(),
                        serde_json::to_string(&portfolio.risk_settings).unwrap_or_default(),
                        if portfolio.is_competition { 1 } else { 0 },
                        portfolio.competition_id,
                        portfolio.created_at,
                        portfolio.updated_at,
                        portfolio.total_trades,
                        portfolio.winning_trades,
                        version as i64,
                        timestamp,
                        node_id,
                    ],
                )?;
                Ok(SyncUpdateResult::Applied)
            }
            EntityType::Order => {
                let order: crate::types::Order = serde_json::from_slice(data)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Blob, Box::new(e)))?;
                
                // Update order with version tracking
                conn.execute(
                    "INSERT OR REPLACE INTO orders (
                        id, portfolio_id, symbol, asset_class, side, order_type,
                        quantity, filled_quantity, price, stop_price, trail_amount, trail_percent,
                        time_in_force, status, linked_order_id, bracket_id, leverage,
                        fills_json, avg_fill_price, total_fees, client_order_id,
                        created_at, updated_at, expires_at,
                        trail_high_price, trail_low_price, bracket_role,
                        version, last_modified_at, last_modified_by
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30)",
                    params![
                        order.id,
                        order.portfolio_id,
                        order.symbol,
                        format!("{:?}", order.asset_class).to_lowercase().replace("_", "_"),
                        format!("{:?}", order.side).to_lowercase(),
                        format!("{:?}", order.order_type).to_lowercase().replace("_", "_"),
                        order.quantity,
                        order.filled_quantity,
                        order.price,
                        order.stop_price,
                        order.trail_amount,
                        order.trail_percent,
                        format!("{:?}", order.time_in_force).to_lowercase(),
                        format!("{:?}", order.status).to_lowercase().replace("_", "_"),
                        order.linked_order_id,
                        order.bracket_id,
                        order.leverage,
                        serde_json::to_string(&order.fills).unwrap_or_default(),
                        order.avg_fill_price,
                        order.total_fees,
                        order.client_order_id,
                        order.created_at,
                        order.updated_at,
                        order.expires_at,
                        order.trail_high_price,
                        order.trail_low_price,
                        order.bracket_role.as_ref().map(|r| format!("{:?}", r).to_lowercase().replace("_", "_")),
                        version as i64,
                        timestamp,
                        node_id,
                    ],
                )?;
                Ok(SyncUpdateResult::Applied)
            }
            EntityType::Trade => {
                let trade: crate::types::Trade = serde_json::from_slice(data)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Blob, Box::new(e)))?;
                
                // Insert trade (trades are append-only)
                conn.execute(
                    "INSERT OR IGNORE INTO trades (
                        id, order_id, portfolio_id, position_id,
                        symbol, asset_class, side, quantity, price, fee, slippage,
                        realized_pnl, executed_at,
                        version, last_modified_at, last_modified_by
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
                    params![
                        trade.id,
                        trade.order_id,
                        trade.portfolio_id,
                        trade.position_id,
                        trade.symbol,
                        format!("{:?}", trade.asset_class).to_lowercase().replace("_", "_"),
                        format!("{:?}", trade.side).to_lowercase(),
                        trade.quantity,
                        trade.price,
                        trade.fee,
                        trade.slippage,
                        trade.realized_pnl,
                        trade.executed_at,
                        version as i64,
                        timestamp,
                        node_id,
                    ],
                )?;
                Ok(SyncUpdateResult::Applied)
            }
            EntityType::Position => {
                let position: crate::types::Position = serde_json::from_slice(data)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Blob, Box::new(e)))?;
                
                // Update position with version tracking
                conn.execute(
                    "INSERT OR REPLACE INTO positions (
                        id, portfolio_id, symbol, asset_class, side,
                        quantity, entry_price, current_price,
                        unrealized_pnl, unrealized_pnl_pct, realized_pnl,
                        margin_used, leverage, margin_mode,
                        liquidation_price, stop_loss, take_profit,
                        cost_basis_json, funding_payments,
                        created_at, updated_at,
                        version, last_modified_at, last_modified_by
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24)",
                    params![
                        position.id,
                        position.portfolio_id,
                        position.symbol,
                        format!("{:?}", position.asset_class).to_lowercase().replace("_", "_"),
                        format!("{:?}", position.side).to_lowercase(),
                        position.quantity,
                        position.entry_price,
                        position.current_price,
                        position.unrealized_pnl,
                        position.unrealized_pnl_pct,
                        position.realized_pnl,
                        position.margin_used,
                        position.leverage,
                        format!("{:?}", position.margin_mode).to_lowercase(),
                        position.liquidation_price,
                        position.stop_loss,
                        position.take_profit,
                        serde_json::to_string(&position.cost_basis).unwrap_or_default(),
                        position.funding_payments,
                        position.created_at,
                        position.updated_at,
                        version as i64,
                        timestamp,
                        node_id,
                    ],
                )?;
                Ok(SyncUpdateResult::Applied)
            }
            EntityType::Liquidation => {
                let liquidation: crate::types::Liquidation = serde_json::from_slice(data)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Blob, Box::new(e)))?;
                
                // Insert liquidation (append-only)
                conn.execute(
                    "INSERT OR IGNORE INTO liquidations (
                        id, position_id, portfolio_id, symbol,
                        quantity, liquidation_price, mark_price, loss, liquidation_fee,
                        is_partial, remaining_quantity, liquidated_at,
                        version, last_modified_at, last_modified_by
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                    params![
                        liquidation.id,
                        liquidation.position_id,
                        liquidation.portfolio_id,
                        liquidation.symbol,
                        liquidation.quantity,
                        liquidation.liquidation_price,
                        liquidation.mark_price,
                        liquidation.loss,
                        liquidation.liquidation_fee,
                        if liquidation.is_partial { 1 } else { 0 },
                        liquidation.remaining_quantity,
                        liquidation.liquidated_at,
                        version as i64,
                        timestamp,
                        node_id,
                    ],
                )?;
                Ok(SyncUpdateResult::Applied)
            }
            // For other entity types, do nothing for now
            _ => Ok(SyncUpdateResult::Applied),
        }
    }

    /// Record a sync conflict to the database.
    pub fn insert_sync_conflict(
        &self,
        entity_type: crate::types::EntityType,
        entity_id: &str,
        local_version: u64,
        local_timestamp: i64,
        local_node: &str,
        remote_version: u64,
        remote_timestamp: i64,
        remote_node: &str,
        resolution_strategy: &str,
        resolution_reason: Option<String>,
    ) -> Result<String, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let conflict_id = uuid::Uuid::new_v4().to_string();
        let detected_at = chrono::Utc::now().timestamp_millis();
        
        conn.execute(
            "INSERT INTO sync_conflicts (
                id, entity_type, entity_id,
                local_version, local_timestamp, local_node,
                remote_version, remote_timestamp, remote_node,
                resolution_strategy, resolution_reason,
                detected_at, resolved_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, NULL)",
            params![
                conflict_id,
                format!("{:?}", entity_type).to_lowercase(),
                entity_id,
                local_version as i64,
                local_timestamp,
                local_node,
                remote_version as i64,
                remote_timestamp,
                remote_node,
                resolution_strategy,
                resolution_reason,
                detected_at,
            ],
        )?;
        
        Ok(conflict_id)
    }

    /// Increment entity version (call after any entity modification).
    pub fn increment_entity_version(
        &self,
        entity_type: crate::types::EntityType,
        entity_id: &str,
        node_id: &str,
    ) -> Result<u64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let table = entity_type.table_name();
        let timestamp = chrono::Utc::now().timestamp_millis();
        
        // Get current version
        let query = format!("SELECT version FROM {} WHERE id = ?1", table);
        let current_version: i64 = conn.query_row(&query, params![entity_id], |row| row.get(0))
            .unwrap_or(0);
        
        let new_version = current_version + 1;
        
        // Update version and metadata
        let update_query = format!(
            "UPDATE {} SET version = ?1, last_modified_at = ?2, last_modified_by = ?3 WHERE id = ?4",
            table
        );
        
        conn.execute(&update_query, params![new_version, timestamp, node_id, entity_id])?;
        
        Ok(new_version as u64)
    }

    /// Insert node metrics.
    pub fn insert_node_metrics(&self, metrics: &crate::types::NodeMetrics) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute(
            "INSERT INTO node_metrics (
                id, node_id, timestamp, sync_lag_ms, pending_sync_count,
                synced_entities_1m, sync_errors_1m, sync_throughput_mbps,
                db_size_mb, db_row_count, db_write_rate, db_read_rate,
                cpu_usage_pct, memory_usage_mb, disk_usage_pct,
                network_rx_mbps, network_tx_mbps,
                active_users, active_portfolios, open_orders, open_positions
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
            params![
                metrics.id,
                metrics.node_id,
                metrics.timestamp,
                metrics.sync_lag_ms,
                metrics.pending_sync_count,
                metrics.synced_entities_1m,
                metrics.sync_errors_1m,
                metrics.sync_throughput_mbps,
                metrics.db_size_mb,
                metrics.db_row_count,
                metrics.db_write_rate,
                metrics.db_read_rate,
                metrics.cpu_usage_pct,
                metrics.memory_usage_mb,
                metrics.disk_usage_pct,
                metrics.network_rx_mbps,
                metrics.network_tx_mbps,
                metrics.active_users,
                metrics.active_portfolios,
                metrics.open_orders,
                metrics.open_positions,
            ],
        )?;
        
        Ok(())
    }

    /// Get database stats.
    pub fn get_database_stats(&self) -> Result<(f64, u32), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        
        // Get page count and page size to calculate DB size
        let page_count: i64 = conn.query_row("PRAGMA page_count", [], |row| row.get(0))?;
        let page_size: i64 = conn.query_row("PRAGMA page_size", [], |row| row.get(0))?;
        let db_size_mb = (page_count * page_size) as f64 / (1024.0 * 1024.0);
        
        // Count total rows across all tables
        let mut row_count = 0u32;
        let tables = vec![
            "profiles", "portfolios", "orders", "positions", "trades",
            "options_positions", "strategies", "funding_payments", "liquidations",
            "margin_history", "portfolio_snapshots", "prediction_history"
        ];
        
        for table in tables {
            let count: i64 = conn.query_row(
                &format!("SELECT COUNT(*) FROM {}", table),
                [],
                |row| row.get(0)
            ).unwrap_or(0);
            row_count += count as u32;
        }
        
        Ok((db_size_mb, row_count))
    }

    /// Get active counts (portfolios, orders, positions).
    pub fn get_active_counts(&self) -> Result<(u32, u32, u32), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        
        let active_portfolios: i64 = conn.query_row(
            "SELECT COUNT(*) FROM portfolios",
            [],
            |row| row.get(0)
        )?;
        
        let open_orders: i64 = conn.query_row(
            "SELECT COUNT(*) FROM orders WHERE status IN ('pending', 'open', 'partially_filled')",
            [],
            |row| row.get(0)
        )?;
        
        let open_positions: i64 = conn.query_row(
            "SELECT COUNT(*) FROM positions WHERE closed_at IS NULL",
            [],
            |row| row.get(0)
        )?;
        
        Ok((
            active_portfolios as u32,
            open_orders as u32,
            open_positions as u32,
        ))
    }

    /// Get recent node metrics for a specific node.
    pub fn get_node_metrics(&self, node_id: &str, limit: usize) -> Result<Vec<crate::types::NodeMetrics>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, node_id, timestamp, sync_lag_ms, pending_sync_count,
                    synced_entities_1m, sync_errors_1m, sync_throughput_mbps,
                    db_size_mb, db_row_count, db_write_rate, db_read_rate,
                    cpu_usage_pct, memory_usage_mb, disk_usage_pct,
                    network_rx_mbps, network_tx_mbps,
                    active_users, active_portfolios, open_orders, open_positions
             FROM node_metrics
             WHERE node_id = ?1
             ORDER BY timestamp DESC
             LIMIT ?2",
        )?;

        let metrics = stmt.query_map(params![node_id, limit as i64], |row| {
            Ok(crate::types::NodeMetrics {
                id: row.get(0)?,
                node_id: row.get(1)?,
                timestamp: row.get(2)?,
                sync_lag_ms: row.get(3)?,
                pending_sync_count: row.get(4)?,
                synced_entities_1m: row.get(5)?,
                sync_errors_1m: row.get(6)?,
                sync_throughput_mbps: row.get(7)?,
                db_size_mb: row.get(8)?,
                db_row_count: row.get(9)?,
                db_write_rate: row.get(10)?,
                db_read_rate: row.get(11)?,
                cpu_usage_pct: row.get(12)?,
                memory_usage_mb: row.get(13)?,
                disk_usage_pct: row.get(14)?,
                network_rx_mbps: row.get(15)?,
                network_tx_mbps: row.get(16)?,
                active_users: row.get(17)?,
                active_portfolios: row.get(18)?,
                open_orders: row.get(19)?,
                open_positions: row.get(20)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(metrics)
    }
}

// ========== Parsing Helpers for Trading Types ==========

fn parse_cost_basis_method(s: &str) -> CostBasisMethod {
    match s {
        "lifo" => CostBasisMethod::Lifo,
        "average" => CostBasisMethod::Average,
        _ => CostBasisMethod::Fifo,
    }
}

fn parse_asset_class(s: &str) -> AssetClass {
    match s {
        "crypto_spot" => AssetClass::CryptoSpot,
        "stock" => AssetClass::Stock,
        "etf" => AssetClass::Etf,
        "perp" => AssetClass::Perp,
        "option" => AssetClass::Option,
        "forex" => AssetClass::Forex,
        _ => AssetClass::CryptoSpot,
    }
}

fn parse_order_side(s: &str) -> OrderSide {
    match s {
        "sell" => OrderSide::Sell,
        _ => OrderSide::Buy,
    }
}

fn parse_order_type(s: &str) -> OrderType {
    match s {
        "limit" => OrderType::Limit,
        "stop_loss" => OrderType::StopLoss,
        "take_profit" => OrderType::TakeProfit,
        "stop_limit" => OrderType::StopLimit,
        "trailing_stop" => OrderType::TrailingStop,
        _ => OrderType::Market,
    }
}

fn parse_order_status(s: &str) -> OrderStatus {
    match s {
        "open" => OrderStatus::Open,
        "partially_filled" => OrderStatus::PartiallyFilled,
        "filled" => OrderStatus::Filled,
        "cancelled" => OrderStatus::Cancelled,
        "expired" => OrderStatus::Expired,
        "rejected" => OrderStatus::Rejected,
        _ => OrderStatus::Pending,
    }
}

fn parse_time_in_force(s: &str) -> TimeInForce {
    match s {
        "gtd" => TimeInForce::Gtd,
        "fok" => TimeInForce::Fok,
        "ioc" => TimeInForce::Ioc,
        _ => TimeInForce::Gtc,
    }
}

fn parse_position_side(s: &str) -> PositionSide {
    match s {
        "short" => PositionSide::Short,
        _ => PositionSide::Long,
    }
}

fn parse_margin_mode(s: &str) -> MarginMode {
    match s {
        "cross" => MarginMode::Cross,
        _ => MarginMode::Isolated,
    }
}

fn parse_bracket_role(s: &str) -> BracketRole {
    match s {
        "stop_loss" => BracketRole::StopLoss,
        "take_profit" => BracketRole::TakeProfit,
        _ => BracketRole::Entry,
    }
}

fn parse_margin_change_type(s: &str) -> MarginChangeType {
    match s {
        "position_opened" => MarginChangeType::PositionOpened,
        "position_closed" => MarginChangeType::PositionClosed,
        "position_increased" => MarginChangeType::PositionIncreased,
        "position_decreased" => MarginChangeType::PositionDecreased,
        "funding_payment" => MarginChangeType::FundingPayment,
        "unrealized_pnl_change" => MarginChangeType::UnrealizedPnlChange,
        "liquidation" => MarginChangeType::Liquidation,
        "manual_adjustment" => MarginChangeType::ManualAdjustment,
        _ => MarginChangeType::PositionOpened,
    }
}

fn parse_option_type(s: &str) -> OptionType {
    match s {
        "put" => OptionType::Put,
        _ => OptionType::Call,
    }
}

fn parse_option_style(s: &str) -> OptionStyle {
    match s {
        "american" => OptionStyle::American,
        _ => OptionStyle::European,
    }
}

fn parse_strategy_status(s: &str) -> StrategyStatus {
    match s {
        "active" => StrategyStatus::Active,
        "paused" => StrategyStatus::Paused,
        "disabled" => StrategyStatus::Disabled,
        "deleted" => StrategyStatus::Deleted,
        _ => StrategyStatus::Paused,
    }
}

fn parse_backtest_status(s: &str) -> crate::types::BacktestStatus {
    match s {
        "pending" => crate::types::BacktestStatus::Pending,
        "running" => crate::types::BacktestStatus::Running,
        "completed" => crate::types::BacktestStatus::Completed,
        "failed" => crate::types::BacktestStatus::Failed,
        "cancelled" => crate::types::BacktestStatus::Cancelled,
        _ => crate::types::BacktestStatus::Pending,
    }
}

fn parse_entity_type(s: &str) -> crate::types::EntityType {
    match s {
        "profile" => crate::types::EntityType::Profile,
        "portfolio" => crate::types::EntityType::Portfolio,
        "order" => crate::types::EntityType::Order,
        "position" => crate::types::EntityType::Position,
        "trade" => crate::types::EntityType::Trade,
        "optionsposition" | "options_position" => crate::types::EntityType::OptionsPosition,
        "strategy" => crate::types::EntityType::Strategy,
        "fundingpayment" | "funding_payment" => crate::types::EntityType::FundingPayment,
        "liquidation" => crate::types::EntityType::Liquidation,
        "marginhistory" | "margin_history" => crate::types::EntityType::MarginHistory,
        "portfoliosnapshot" | "portfolio_snapshot" => crate::types::EntityType::PortfolioSnapshot,
        "insurancefund" | "insurance_fund" => crate::types::EntityType::InsuranceFund,
        "predictionhistory" | "prediction_history" => crate::types::EntityType::PredictionHistory,
        _ => crate::types::EntityType::Portfolio, // Default fallback
    }
}

fn parse_sync_operation(s: &str) -> crate::types::SyncOperation {
    match s {
        "insert" => crate::types::SyncOperation::Insert,
        "update" => crate::types::SyncOperation::Update,
        "delete" => crate::types::SyncOperation::Delete,
        _ => crate::types::SyncOperation::Update, // Default fallback
    }
}

/// Accuracy statistics.
#[derive(Debug, Default, Clone)]
pub struct AccuracyStats {
    pub total: i64,
    pub correct: i64,
    pub incorrect: i64,
    pub neutral: i64,
}

impl AccuracyStats {
    /// Calculate accuracy percentage (correct / (correct + incorrect) * 100).
    pub fn accuracy_pct(&self) -> f64 {
        let decidable = self.correct + self.incorrect;
        if decidable == 0 {
            return 0.0;
        }
        (self.correct as f64 / decidable as f64) * 100.0
    }
}

/// Parse direction string to SignalDirection.
fn parse_direction(s: &str) -> crate::types::SignalDirection {
    match s {
        "strong_buy" => crate::types::SignalDirection::StrongBuy,
        "buy" => crate::types::SignalDirection::Buy,
        "sell" => crate::types::SignalDirection::Sell,
        "strong_sell" => crate::types::SignalDirection::StrongSell,
        _ => crate::types::SignalDirection::Neutral,
    }
}

/// Parse outcome string to PredictionOutcome.
fn parse_outcome(s: &str) -> PredictionOutcome {
    match s {
        "correct" => PredictionOutcome::Correct,
        "incorrect" => PredictionOutcome::Incorrect,
        _ => PredictionOutcome::Neutral,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SignalDirection;

    #[test]
    fn test_profile_crud() {
        let store = SqliteStore::new_in_memory().unwrap();

        // Create profile
        let profile = Profile::new("abc123".repeat(8), "TestTrader42".to_string());
        store.save_profile(&profile).unwrap();

        // Read profile
        let loaded = store.get_profile(&profile.public_key).unwrap();
        assert_eq!(loaded.id, profile.id);
        assert_eq!(loaded.public_key, profile.public_key);

        // Update last_seen
        store.update_last_seen(&profile.public_key).unwrap();
        let updated = store.get_profile(&profile.public_key).unwrap();
        assert!(updated.last_seen >= loaded.last_seen);

        // Delete
        store.delete_profile(&profile.public_key).unwrap();
        assert!(store.get_profile(&profile.public_key).is_none());
    }

    #[test]
    fn test_prediction_archive() {
        let store = SqliteStore::new_in_memory().unwrap();

        let prediction = SignalPrediction::new(
            "BTC".to_string(),
            "RSI".to_string(),
            SignalDirection::Buy,
            75,
            50000.0,
        );

        store.archive_prediction(&prediction).unwrap();

        let predictions = store.get_predictions("btc", None, 10);
        assert_eq!(predictions.len(), 1);
        assert_eq!(predictions[0].indicator, "RSI");
    }

    #[test]
    fn test_accuracy_stats() {
        let store = SqliteStore::new_in_memory().unwrap();

        // Add some predictions with outcomes
        for i in 0..10 {
            let mut prediction = SignalPrediction::new(
                "ETH".to_string(),
                "MACD".to_string(),
                SignalDirection::Buy,
                60,
                3000.0,
            );
            prediction.price_after_5m = Some(3010.0);
            prediction.price_after_1h = Some(3050.0);
            prediction.validated = true;

            // 7 correct, 3 incorrect
            if i < 7 {
                prediction.outcome_1h = Some(PredictionOutcome::Correct);
            } else {
                prediction.outcome_1h = Some(PredictionOutcome::Incorrect);
            }

            store.archive_prediction(&prediction).unwrap();
        }

        let stats = store.get_accuracy_stats("eth", "1h");
        assert_eq!(stats.total, 10);
        assert_eq!(stats.correct, 7);
        assert_eq!(stats.incorrect, 3);
        assert!((stats.accuracy_pct() - 70.0).abs() < 0.01);
    }

    // ========== Trading Tests ==========

    #[test]
    fn test_portfolio_crud() {
        let store = SqliteStore::new_in_memory().unwrap();

        // Create portfolio
        let portfolio = Portfolio::new("user123".to_string(), "Test Portfolio".to_string());
        store.create_portfolio(&portfolio).unwrap();

        // Read portfolio
        let loaded = store.get_portfolio(&portfolio.id).unwrap();
        assert_eq!(loaded.id, portfolio.id);
        assert_eq!(loaded.user_id, "user123");
        assert_eq!(loaded.name, "Test Portfolio");
        assert_eq!(loaded.starting_balance, 250_000.0);

        // Update portfolio
        let mut updated = loaded.clone();
        updated.cash_balance = 225_000.0;
        updated.unrealized_pnl = 10_000.0;
        updated.recalculate();
        store.update_portfolio(&updated).unwrap();

        let reloaded = store.get_portfolio(&portfolio.id).unwrap();
        assert_eq!(reloaded.cash_balance, 225_000.0);

        // Get user portfolios
        let portfolios = store.get_user_portfolios("user123");
        assert_eq!(portfolios.len(), 1);

        // Delete portfolio
        store.delete_portfolio(&portfolio.id).unwrap();
        assert!(store.get_portfolio(&portfolio.id).is_none());
    }

    #[test]
    fn test_portfolio_snapshots() {
        let store = SqliteStore::new_in_memory().unwrap();

        // Create portfolio
        let portfolio = Portfolio::new("user123".to_string(), "Snapshot Test".to_string());
        store.create_portfolio(&portfolio).unwrap();

        // Create several snapshots
        store
            .create_portfolio_snapshot(
                &portfolio.id,
                250_000.0, // equity
                250_000.0, // cash
                0.0,       // positions_value
                0.0,       // realized_pnl
                0.0,       // unrealized_pnl
                0.0,       // drawdown_pct
                250_000.0, // peak_equity
            )
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(10));

        store
            .create_portfolio_snapshot(
                &portfolio.id,
                260_000.0, // equity increased
                240_000.0, // cash decreased (bought something)
                20_000.0,  // positions_value
                0.0,       // realized_pnl
                10_000.0,  // unrealized_pnl
                0.0,       // drawdown_pct
                260_000.0, // new peak
            )
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(10));

        store
            .create_portfolio_snapshot(
                &portfolio.id,
                255_000.0, // equity dropped
                240_000.0, // cash same
                15_000.0,  // positions_value dropped
                0.0,       // realized_pnl
                5_000.0,   // unrealized_pnl dropped
                1.92,      // drawdown_pct = (260000 - 255000) / 260000 * 100
                260_000.0, // peak unchanged
            )
            .unwrap();

        // Verify snapshot count
        assert_eq!(store.snapshot_count(&portfolio.id), 3);

        // Get all snapshots
        let snapshots = store.get_portfolio_snapshots(&portfolio.id, None, None);
        assert_eq!(snapshots.len(), 3);

        // Verify order is ascending by timestamp
        assert!(snapshots[0].timestamp < snapshots[1].timestamp);
        assert!(snapshots[1].timestamp < snapshots[2].timestamp);

        // Verify values
        assert_eq!(snapshots[0].equity, 250_000.0);
        assert_eq!(snapshots[1].equity, 260_000.0);
        assert_eq!(snapshots[2].equity, 255_000.0);

        // Get latest snapshot
        let latest = store.get_latest_portfolio_snapshot(&portfolio.id).unwrap();
        assert_eq!(latest.equity, 255_000.0);
        assert_eq!(latest.unrealized_pnl, 5_000.0);

        // Create snapshot from portfolio object
        let mut test_portfolio = portfolio.clone();
        test_portfolio.total_value = 270_000.0;
        test_portfolio.cash_balance = 220_000.0;
        test_portfolio.unrealized_pnl = 20_000.0;
        store.create_snapshot_from_portfolio(&test_portfolio).unwrap();

        assert_eq!(store.snapshot_count(&portfolio.id), 4);

        // Test cleanup (keep only recent)
        let deleted = store.cleanup_old_snapshots(&portfolio.id, 0).unwrap();
        // All snapshots created in this test should be deleted since days_to_keep=0
        assert!(deleted > 0);
    }

    #[test]
    fn test_order_crud() {
        let store = SqliteStore::new_in_memory().unwrap();

        // Create portfolio first
        let portfolio = Portfolio::new("user123".to_string(), "Trading".to_string());
        store.create_portfolio(&portfolio).unwrap();

        // Create order
        let order = Order::market(
            portfolio.id.clone(),
            "BTC".to_string(),
            AssetClass::CryptoSpot,
            OrderSide::Buy,
            1.0,
        );
        store.create_order(&order).unwrap();

        // Read order
        let loaded = store.get_order(&order.id).unwrap();
        assert_eq!(loaded.id, order.id);
        assert_eq!(loaded.symbol, "BTC");
        assert_eq!(loaded.side, OrderSide::Buy);
        assert_eq!(loaded.order_type, OrderType::Market);
        assert_eq!(loaded.status, OrderStatus::Pending);

        // Update order with fill
        let mut filled_order = loaded.clone();
        filled_order.add_fill(Fill::new(1.0, 50000.0, 50.0));
        store.update_order(&filled_order).unwrap();

        let reloaded = store.get_order(&order.id).unwrap();
        assert_eq!(reloaded.status, OrderStatus::Filled);
        assert_eq!(reloaded.filled_quantity, 1.0);
        assert_eq!(reloaded.avg_fill_price, Some(50000.0));

        // Get open orders (should be empty now)
        let open = store.get_open_orders(&portfolio.id);
        assert!(open.is_empty());
    }

    #[test]
    fn test_position_crud() {
        let store = SqliteStore::new_in_memory().unwrap();

        // Create portfolio
        let portfolio = Portfolio::new("user123".to_string(), "Trading".to_string());
        store.create_portfolio(&portfolio).unwrap();

        // Create position
        let position = Position::new(
            portfolio.id.clone(),
            "ETH".to_string(),
            AssetClass::CryptoSpot,
            PositionSide::Long,
            10.0,
            2500.0,
            1.0,
        );
        store.create_position(&position).unwrap();

        // Read position
        let loaded = store.get_position(&position.id).unwrap();
        assert_eq!(loaded.id, position.id);
        assert_eq!(loaded.symbol, "ETH");
        assert_eq!(loaded.side, PositionSide::Long);
        assert_eq!(loaded.quantity, 10.0);
        assert_eq!(loaded.entry_price, 2500.0);

        // Get by symbol
        let by_symbol = store
            .get_position_by_symbol(&portfolio.id, "ETH", PositionSide::Long)
            .unwrap();
        assert_eq!(by_symbol.id, position.id);

        // Update position
        let mut updated = loaded.clone();
        updated.update_price(2600.0);
        store.update_position(&updated).unwrap();

        let reloaded = store.get_position(&position.id).unwrap();
        assert_eq!(reloaded.current_price, 2600.0);
        assert!(reloaded.unrealized_pnl > 0.0);

        // Position count
        assert_eq!(store.position_count(&portfolio.id), 1);

        // Close position
        store.close_position(&position.id).unwrap();
        assert!(store.get_position(&position.id).is_none());
        assert_eq!(store.position_count(&portfolio.id), 0);
    }

    #[test]
    fn test_trade_crud() {
        let store = SqliteStore::new_in_memory().unwrap();

        // Create portfolio and order
        let portfolio = Portfolio::new("user123".to_string(), "Trading".to_string());
        store.create_portfolio(&portfolio).unwrap();

        let order = Order::market(
            portfolio.id.clone(),
            "BTC".to_string(),
            AssetClass::CryptoSpot,
            OrderSide::Buy,
            1.0,
        );
        store.create_order(&order).unwrap();

        // Create trade
        let trade = Trade::new(
            order.id.clone(),
            portfolio.id.clone(),
            "BTC".to_string(),
            AssetClass::CryptoSpot,
            OrderSide::Buy,
            1.0,
            50000.0,
            50.0,
            10.0,
        );
        store.create_trade(&trade).unwrap();

        // Get portfolio trades
        let trades = store.get_portfolio_trades(&portfolio.id, 10);
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].symbol, "BTC");
        assert_eq!(trades[0].price, 50000.0);

        // Get order trades
        let order_trades = store.get_order_trades(&order.id);
        assert_eq!(order_trades.len(), 1);
    }

    #[test]
    fn test_limit_order_persistence() {
        let store = SqliteStore::new_in_memory().unwrap();

        let portfolio = Portfolio::new("user123".to_string(), "Trading".to_string());
        store.create_portfolio(&portfolio).unwrap();

        let order = Order::limit(
            portfolio.id.clone(),
            "AAPL".to_string(),
            AssetClass::Stock,
            OrderSide::Buy,
            100.0,
            150.0,
        );
        store.create_order(&order).unwrap();

        let loaded = store.get_order(&order.id).unwrap();
        assert_eq!(loaded.order_type, OrderType::Limit);
        assert_eq!(loaded.price, Some(150.0));
        assert_eq!(loaded.asset_class, AssetClass::Stock);
    }

    #[test]
    fn test_leveraged_position() {
        let store = SqliteStore::new_in_memory().unwrap();

        let portfolio = Portfolio::new("user123".to_string(), "Perps".to_string());
        store.create_portfolio(&portfolio).unwrap();

        let mut position = Position::new(
            portfolio.id.clone(),
            "BTC".to_string(),
            AssetClass::Perp,
            PositionSide::Long,
            1.0,
            50000.0,
            10.0, // 10x leverage
        );
        position.calculate_liquidation_price();
        store.create_position(&position).unwrap();

        let loaded = store.get_position(&position.id).unwrap();
        assert_eq!(loaded.leverage, 10.0);
        assert_eq!(loaded.margin_used, 5000.0); // 50000 / 10
        assert!(loaded.liquidation_price.is_some());
    }

    // ========== Options Position Tests ==========

    #[test]
    fn test_option_position_crud() {
        use crate::types::OptionContract;

        let store = SqliteStore::new_in_memory().unwrap();

        // Create portfolio
        let portfolio = Portfolio::new("user123".to_string(), "Options".to_string());
        store.create_portfolio(&portfolio).unwrap();

        // Create option contract
        let expiration = chrono::Utc::now().timestamp_millis() + 30 * 24 * 60 * 60 * 1000; // 30 days
        let contract = OptionContract::new(
            "AAPL".to_string(),
            OptionType::Call,
            180.0,
            expiration,
            OptionStyle::American,
        );

        // Create option position
        let position = OptionPosition::new(
            portfolio.id.clone(),
            &contract,
            5,      // 5 contracts
            3.50,   // $3.50 premium per share
        );
        store.create_option_position(&position).unwrap();

        // Read position
        let loaded = store.get_option_position(&position.id).unwrap();
        assert_eq!(loaded.id, position.id);
        assert_eq!(loaded.underlying_symbol, "AAPL");
        assert_eq!(loaded.option_type, OptionType::Call);
        assert_eq!(loaded.strike, 180.0);
        assert_eq!(loaded.contracts, 5);
        assert_eq!(loaded.entry_premium, 3.50);
        assert_eq!(loaded.style, OptionStyle::American);
        assert_eq!(loaded.multiplier, 100);

        // Get portfolio option positions
        let positions = store.get_portfolio_option_positions(&portfolio.id);
        assert_eq!(positions.len(), 1);

        // Update position
        let mut updated = loaded.clone();
        updated.current_premium = 4.00;
        updated.underlying_price = 185.0;
        updated.unrealized_pnl = (4.00 - 3.50) * 5.0 * 100.0; // 250.0
        updated.updated_at = chrono::Utc::now().timestamp_millis();
        store.update_option_position(&updated).unwrap();

        let reloaded = store.get_option_position(&position.id).unwrap();
        assert_eq!(reloaded.current_premium, 4.00);
        assert_eq!(reloaded.unrealized_pnl, 250.0);

        // Position count
        assert_eq!(store.option_position_count(&portfolio.id), 1);

        // Close position
        store.close_option_position(&position.id).unwrap();
        assert!(store.get_option_position(&position.id).is_none());
        assert_eq!(store.option_position_count(&portfolio.id), 0);
    }

    #[test]
    fn test_option_positions_by_underlying() {
        use crate::types::OptionContract;

        let store = SqliteStore::new_in_memory().unwrap();

        let portfolio = Portfolio::new("user123".to_string(), "Options".to_string());
        store.create_portfolio(&portfolio).unwrap();

        let expiration = chrono::Utc::now().timestamp_millis() + 30 * 24 * 60 * 60 * 1000;

        // Create AAPL call
        let aapl_call = OptionContract::new(
            "AAPL".to_string(),
            OptionType::Call,
            180.0,
            expiration,
            OptionStyle::American,
        );
        let pos1 = OptionPosition::new(portfolio.id.clone(), &aapl_call, 5, 3.50);
        store.create_option_position(&pos1).unwrap();

        // Create AAPL put
        let aapl_put = OptionContract::new(
            "AAPL".to_string(),
            OptionType::Put,
            175.0,
            expiration,
            OptionStyle::American,
        );
        let pos2 = OptionPosition::new(portfolio.id.clone(), &aapl_put, -3, 2.00);
        store.create_option_position(&pos2).unwrap();

        // Create MSFT call
        let msft_call = OptionContract::new(
            "MSFT".to_string(),
            OptionType::Call,
            400.0,
            expiration,
            OptionStyle::American,
        );
        let pos3 = OptionPosition::new(portfolio.id.clone(), &msft_call, 2, 5.00);
        store.create_option_position(&pos3).unwrap();

        // Get AAPL positions
        let aapl_positions = store.get_option_positions_by_underlying(&portfolio.id, "AAPL");
        assert_eq!(aapl_positions.len(), 2);

        // Get MSFT positions
        let msft_positions = store.get_option_positions_by_underlying(&portfolio.id, "MSFT");
        assert_eq!(msft_positions.len(), 1);

        // Get all positions
        let all_positions = store.get_portfolio_option_positions(&portfolio.id);
        assert_eq!(all_positions.len(), 3);
    }

    #[test]
    fn test_expiring_option_positions() {
        use crate::types::OptionContract;

        let store = SqliteStore::new_in_memory().unwrap();

        let portfolio = Portfolio::new("user123".to_string(), "Options".to_string());
        store.create_portfolio(&portfolio).unwrap();

        let now = chrono::Utc::now().timestamp_millis();

        // Create position expiring in 7 days
        let exp_7d = now + 7 * 24 * 60 * 60 * 1000;
        let contract1 = OptionContract::new("AAPL".to_string(), OptionType::Call, 180.0, exp_7d, OptionStyle::American);
        let pos1 = OptionPosition::new(portfolio.id.clone(), &contract1, 5, 3.50);
        store.create_option_position(&pos1).unwrap();

        // Create position expiring in 30 days
        let exp_30d = now + 30 * 24 * 60 * 60 * 1000;
        let contract2 = OptionContract::new("AAPL".to_string(), OptionType::Call, 185.0, exp_30d, OptionStyle::American);
        let pos2 = OptionPosition::new(portfolio.id.clone(), &contract2, 3, 4.00);
        store.create_option_position(&pos2).unwrap();

        // Get positions expiring before 14 days
        let exp_before_14d = now + 14 * 24 * 60 * 60 * 1000;
        let expiring = store.get_expiring_option_positions(&portfolio.id, exp_before_14d);
        assert_eq!(expiring.len(), 1);
        assert_eq!(expiring[0].strike, 180.0);

        // Get positions expiring before 45 days
        let exp_before_45d = now + 45 * 24 * 60 * 60 * 1000;
        let all_expiring = store.get_expiring_option_positions(&portfolio.id, exp_before_45d);
        assert_eq!(all_expiring.len(), 2);
    }

    #[test]
    fn test_option_position_greeks_persistence() {
        use crate::types::OptionContract;

        let store = SqliteStore::new_in_memory().unwrap();

        let portfolio = Portfolio::new("user123".to_string(), "Options".to_string());
        store.create_portfolio(&portfolio).unwrap();

        let expiration = chrono::Utc::now().timestamp_millis() + 30 * 24 * 60 * 60 * 1000;
        let contract = OptionContract::new("SPY".to_string(), OptionType::Put, 450.0, expiration, OptionStyle::European);

        let mut position = OptionPosition::new(portfolio.id.clone(), &contract, 10, 5.00);
        position.greeks = Greeks::new(0.45, 0.02, -0.05, 0.15, 0.01);
        store.create_option_position(&position).unwrap();

        let loaded = store.get_option_position(&position.id).unwrap();
        assert!((loaded.greeks.delta - 0.45).abs() < 0.001);
        assert!((loaded.greeks.gamma - 0.02).abs() < 0.001);
        assert!((loaded.greeks.theta - (-0.05)).abs() < 0.001);
        assert!((loaded.greeks.vega - 0.15).abs() < 0.001);
        assert!((loaded.greeks.rho - 0.01).abs() < 0.001);

        // Update Greeks
        let mut updated = loaded.clone();
        updated.greeks = Greeks::new(0.50, 0.025, -0.06, 0.14, 0.012);
        updated.updated_at = chrono::Utc::now().timestamp_millis();
        store.update_option_position(&updated).unwrap();

        let reloaded = store.get_option_position(&position.id).unwrap();
        assert!((reloaded.greeks.delta - 0.50).abs() < 0.001);
        assert!((reloaded.greeks.gamma - 0.025).abs() < 0.001);
    }

    // ========== Strategy Tests ==========

    #[test]
    fn test_strategy_crud() {
        use crate::types::{RuleCondition, RuleAction, TradingRule, IndicatorType, ComparisonOperator, PositionSizeType};

        let store = SqliteStore::new_in_memory().unwrap();

        // Create portfolio
        let portfolio = Portfolio::new("user123".to_string(), "Strategies".to_string());
        store.create_portfolio(&portfolio).unwrap();

        // Create strategy
        let mut strategy = TradingStrategy::new(
            portfolio.id.clone(),
            "RSI Strategy".to_string(),
            vec!["BTC".to_string(), "ETH".to_string()],
        );

        // Add a rule
        let rule = TradingRule::new(
            "Buy Oversold".to_string(),
            vec![RuleCondition::new(IndicatorType::Rsi, ComparisonOperator::LessThan, 30.0)],
            RuleAction::market_buy(PositionSizeType::PortfolioPercent, 5.0),
        );
        strategy.add_rule(rule);
        strategy.activate();

        store.create_strategy(&strategy).unwrap();

        // Read strategy
        let loaded = store.get_strategy(&strategy.id).unwrap();
        assert_eq!(loaded.id, strategy.id);
        assert_eq!(loaded.name, "RSI Strategy");
        assert_eq!(loaded.symbols.len(), 2);
        assert_eq!(loaded.rules.len(), 1);
        assert_eq!(loaded.status, StrategyStatus::Active);

        // Get portfolio strategies
        let strategies = store.get_portfolio_strategies(&portfolio.id);
        assert_eq!(strategies.len(), 1);

        // Get active strategies
        let active = store.get_active_strategies(&portfolio.id);
        assert_eq!(active.len(), 1);

        // Update strategy
        let mut updated = loaded.clone();
        updated.record_trade(true, 100.0);
        updated.pause();
        store.update_strategy(&updated).unwrap();

        let reloaded = store.get_strategy(&strategy.id).unwrap();
        assert_eq!(reloaded.total_trades, 1);
        assert_eq!(reloaded.winning_trades, 1);
        assert_eq!(reloaded.realized_pnl, 100.0);
        assert_eq!(reloaded.status, StrategyStatus::Paused);

        // Active strategies should now be empty
        let active = store.get_active_strategies(&portfolio.id);
        assert_eq!(active.len(), 0);

        // Strategy count
        assert_eq!(store.strategy_count(&portfolio.id), 1);

        // Delete strategy
        store.delete_strategy(&strategy.id).unwrap();
        let deleted = store.get_strategy(&strategy.id).unwrap();
        assert_eq!(deleted.status, StrategyStatus::Deleted);

        // Strategy count should be 0 now (deleted strategies excluded)
        assert_eq!(store.strategy_count(&portfolio.id), 0);
    }

    #[test]
    fn test_strategy_with_multiple_rules() {
        use crate::types::{RuleCondition, RuleAction, TradingRule, IndicatorType, ComparisonOperator, PositionSizeType, LogicalOperator};

        let store = SqliteStore::new_in_memory().unwrap();

        let portfolio = Portfolio::new("user123".to_string(), "Multi-Rule".to_string());
        store.create_portfolio(&portfolio).unwrap();

        let mut strategy = TradingStrategy::new(
            portfolio.id.clone(),
            "Complex Strategy".to_string(),
            vec!["BTC".to_string()],
        );

        // Add buy rule
        let buy_rule = TradingRule::new(
            "Buy Signal".to_string(),
            vec![
                RuleCondition::new(IndicatorType::Rsi, ComparisonOperator::LessThan, 30.0),
                RuleCondition::new(IndicatorType::Macd, ComparisonOperator::CrossesAbove, 0.0),
            ],
            RuleAction::market_buy(PositionSizeType::PortfolioPercent, 5.0),
        );
        strategy.add_rule(buy_rule);

        // Add sell rule
        let sell_rule = TradingRule::new(
            "Sell Signal".to_string(),
            vec![RuleCondition::new(IndicatorType::Rsi, ComparisonOperator::GreaterThan, 70.0)],
            RuleAction::market_sell(PositionSizeType::PortfolioPercent, 100.0),
        );
        strategy.add_rule(sell_rule);

        store.create_strategy(&strategy).unwrap();

        let loaded = store.get_strategy(&strategy.id).unwrap();
        assert_eq!(loaded.rules.len(), 2);
        assert_eq!(loaded.rules[0].name, "Buy Signal");
        assert_eq!(loaded.rules[0].conditions.len(), 2);
        assert_eq!(loaded.rules[1].name, "Sell Signal");
        assert_eq!(loaded.rules[1].conditions.len(), 1);
    }
}
