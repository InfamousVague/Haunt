use dashmap::DashMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// A client's subscription information.
pub struct ClientSubscription {
    /// Subscribed asset symbols.
    pub assets: HashSet<String>,
    /// Channel to send messages to the client.
    pub tx: mpsc::UnboundedSender<String>,
    /// Throttle interval in milliseconds (0 = no throttling).
    pub throttle_ms: AtomicU64,
    /// Last update time per symbol for throttling.
    pub last_updates: RwLock<HashMap<String, Instant>>,
    /// Whether this client is subscribed to peer updates.
    pub subscribed_to_peers: std::sync::atomic::AtomicBool,
    /// Subscribed trading portfolio IDs.
    pub trading_portfolios: RwLock<HashSet<String>>,
}

/// Manages WebSocket client subscriptions.
pub struct RoomManager {
    /// Client subscriptions keyed by client ID.
    pub clients: DashMap<Uuid, ClientSubscription>,
    /// Asset rooms: asset symbol -> set of client IDs.
    rooms: DashMap<String, HashSet<Uuid>>,
    /// Trading rooms: portfolio_id -> set of client IDs.
    trading_rooms: DashMap<String, HashSet<Uuid>>,
    /// Gridline rooms: symbol -> set of client IDs (for multiplier broadcasts).
    gridline_rooms: DashMap<String, HashSet<Uuid>>,
}

impl RoomManager {
    /// Create a new room manager.
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            clients: DashMap::new(),
            rooms: DashMap::new(),
            trading_rooms: DashMap::new(),
            gridline_rooms: DashMap::new(),
        })
    }

    /// Register a new client.
    pub fn register(&self, tx: mpsc::UnboundedSender<String>) -> Uuid {
        let client_id = Uuid::new_v4();
        self.clients.insert(
            client_id,
            ClientSubscription {
                assets: HashSet::new(),
                tx,
                throttle_ms: AtomicU64::new(0),
                last_updates: RwLock::new(HashMap::new()),
                subscribed_to_peers: std::sync::atomic::AtomicBool::new(false),
                trading_portfolios: RwLock::new(HashSet::new()),
            },
        );
        client_id
    }

    /// Subscribe a client to peer updates.
    pub fn subscribe_peers(&self, client_id: Uuid) -> bool {
        if let Some(client) = self.clients.get(&client_id) {
            client
                .subscribed_to_peers
                .store(true, std::sync::atomic::Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Unsubscribe a client from peer updates.
    pub fn unsubscribe_peers(&self, client_id: Uuid) -> bool {
        if let Some(client) = self.clients.get(&client_id) {
            client
                .subscribed_to_peers
                .store(false, std::sync::atomic::Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Check if a client is subscribed to peer updates.
    pub fn is_subscribed_to_peers(&self, client_id: Uuid) -> bool {
        self.clients
            .get(&client_id)
            .map(|c| {
                c.subscribed_to_peers
                    .load(std::sync::atomic::Ordering::Relaxed)
            })
            .unwrap_or(false)
    }

    /// Get all clients subscribed to peer updates.
    pub fn get_peer_subscribers(&self) -> Vec<mpsc::UnboundedSender<String>> {
        self.clients
            .iter()
            .filter(|c| {
                c.subscribed_to_peers
                    .load(std::sync::atomic::Ordering::Relaxed)
            })
            .map(|c| c.tx.clone())
            .collect()
    }

    /// Subscribe a client to trading updates for a portfolio.
    pub async fn subscribe_trading(&self, client_id: Uuid, portfolio_id: &str) -> bool {
        if let Some(client) = self.clients.get(&client_id) {
            let mut portfolios = client.trading_portfolios.write().await;
            if portfolios.insert(portfolio_id.to_string()) {
                // Add to trading room
                self.trading_rooms
                    .entry(portfolio_id.to_string())
                    .or_default()
                    .insert(client_id);
                return true;
            }
        }
        false
    }

    /// Unsubscribe a client from trading updates for a portfolio.
    pub async fn unsubscribe_trading(&self, client_id: Uuid, portfolio_id: &str) -> bool {
        if let Some(client) = self.clients.get(&client_id) {
            let mut portfolios = client.trading_portfolios.write().await;
            if portfolios.remove(portfolio_id) {
                // Remove from trading room
                if let Some(mut room) = self.trading_rooms.get_mut(portfolio_id) {
                    room.remove(&client_id);
                }
                return true;
            }
        }
        false
    }

    /// Get all clients subscribed to a portfolio's trading updates.
    pub fn get_trading_subscribers(&self, portfolio_id: &str) -> Vec<mpsc::UnboundedSender<String>> {
        let client_ids: Vec<Uuid> = self
            .trading_rooms
            .get(portfolio_id)
            .map(|room| room.iter().copied().collect())
            .unwrap_or_default();

        client_ids
            .iter()
            .filter_map(|id| self.clients.get(id).map(|c| c.tx.clone()))
            .collect()
    }

    /// Broadcast a trading update to all clients subscribed to a portfolio.
    pub fn broadcast_trading(&self, portfolio_id: &str, message: &str) {
        let senders = self.get_trading_subscribers(portfolio_id);
        for tx in senders {
            let _ = tx.send(message.to_string());
        }
    }

    // =========================================================================
    // Gridline Rooms (symbol-based, for multiplier broadcasts)
    // =========================================================================

    /// Subscribe a client to gridline updates for a symbol.
    pub fn subscribe_gridline(&self, client_id: Uuid, symbol: &str) -> bool {
        if self.clients.get(&client_id).is_some() {
            let symbol_upper = symbol.to_uppercase();
            self.gridline_rooms
                .entry(symbol_upper)
                .or_default()
                .insert(client_id);
            true
        } else {
            false
        }
    }

    /// Unsubscribe a client from gridline updates for a symbol.
    pub fn unsubscribe_gridline(&self, client_id: Uuid, symbol: &str) -> bool {
        let symbol_upper = symbol.to_uppercase();
        if let Some(mut room) = self.gridline_rooms.get_mut(&symbol_upper) {
            room.remove(&client_id);
            true
        } else {
            false
        }
    }

    /// Get all clients subscribed to gridline updates for a symbol.
    pub fn get_gridline_subscribers(&self, symbol: &str) -> Vec<mpsc::UnboundedSender<String>> {
        let symbol_upper = symbol.to_uppercase();
        let client_ids: Vec<Uuid> = self
            .gridline_rooms
            .get(&symbol_upper)
            .map(|room| room.iter().copied().collect())
            .unwrap_or_default();

        client_ids
            .iter()
            .filter_map(|id| self.clients.get(id).map(|c| c.tx.clone()))
            .collect()
    }

    /// Broadcast a gridline update to all clients subscribed to a symbol.
    pub fn broadcast_gridline(&self, symbol: &str, message: &str) {
        let senders = self.get_gridline_subscribers(symbol);
        for tx in senders {
            let _ = tx.send(message.to_string());
        }
    }

    /// Get all symbols that have at least one gridline subscriber.
    pub fn active_gridline_symbols(&self) -> Vec<String> {
        self.gridline_rooms
            .iter()
            .filter(|r| !r.is_empty())
            .map(|r| r.key().clone())
            .collect()
    }

    /// Set throttle interval for a client.
    pub fn set_throttle(&self, client_id: Uuid, throttle_ms: u64) {
        if let Some(client) = self.clients.get(&client_id) {
            client.throttle_ms.store(throttle_ms, Ordering::Relaxed);
        }
    }

    /// Get throttle interval for a client.
    pub fn get_throttle(&self, client_id: Uuid) -> u64 {
        self.clients
            .get(&client_id)
            .map(|c| c.throttle_ms.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    /// Check if a client should receive an update for a symbol (based on throttling).
    /// Returns true if the update should be sent, false if throttled.
    pub async fn should_send_update(&self, client_id: Uuid, symbol: &str) -> bool {
        if let Some(client) = self.clients.get(&client_id) {
            let throttle_ms = client.throttle_ms.load(Ordering::Relaxed);

            // No throttling
            if throttle_ms == 0 {
                return true;
            }

            let now = Instant::now();
            let symbol_lower = symbol.to_lowercase();

            // Check last update time
            {
                let last_updates = client.last_updates.read().await;
                if let Some(last_time) = last_updates.get(&symbol_lower) {
                    let elapsed = now.duration_since(*last_time).as_millis() as u64;
                    if elapsed < throttle_ms {
                        return false;
                    }
                }
            }

            // Update the timestamp
            {
                let mut last_updates = client.last_updates.write().await;
                last_updates.insert(symbol_lower, now);
            }

            return true;
        }
        false
    }

    /// Unregister a client and remove from all rooms.
    pub fn unregister(&self, client_id: Uuid) {
        if let Some((_, subscription)) = self.clients.remove(&client_id) {
            // Remove from asset rooms
            for asset in subscription.assets {
                if let Some(mut room) = self.rooms.get_mut(&asset) {
                    room.remove(&client_id);
                }
            }
            // Remove from trading rooms - iterate through all since we can't await
            for mut room in self.trading_rooms.iter_mut() {
                room.remove(&client_id);
            }
            // Remove from gridline rooms
            for mut room in self.gridline_rooms.iter_mut() {
                room.remove(&client_id);
            }
        }
    }

    /// Subscribe a client to assets.
    pub fn subscribe(&self, client_id: Uuid, assets: &[String]) -> Vec<String> {
        let mut subscribed = Vec::new();

        if let Some(mut client) = self.clients.get_mut(&client_id) {
            for asset in assets {
                let asset_lower = asset.to_lowercase();
                if client.assets.insert(asset_lower.clone()) {
                    subscribed.push(asset_lower.clone());

                    // Add to room
                    self.rooms.entry(asset_lower).or_default().insert(client_id);
                }
            }
        }

        subscribed
    }

    /// Unsubscribe a client from assets.
    pub fn unsubscribe(&self, client_id: Uuid, assets: &[String]) -> Vec<String> {
        let mut unsubscribed = Vec::new();

        if let Some(mut client) = self.clients.get_mut(&client_id) {
            for asset in assets {
                let asset_lower = asset.to_lowercase();
                if client.assets.remove(&asset_lower) {
                    unsubscribed.push(asset_lower.clone());

                    // Remove from room
                    if let Some(mut room) = self.rooms.get_mut(&asset_lower) {
                        room.remove(&client_id);
                    }
                }
            }
        }

        unsubscribed
    }

    /// Get all clients subscribed to an asset.
    pub fn get_subscribers(&self, asset: &str) -> Vec<mpsc::UnboundedSender<String>> {
        let asset_lower = asset.to_lowercase();

        let client_ids: Vec<Uuid> = self
            .rooms
            .get(&asset_lower)
            .map(|room| room.iter().copied().collect())
            .unwrap_or_default();

        client_ids
            .iter()
            .filter_map(|id| self.clients.get(id).map(|c| c.tx.clone()))
            .collect()
    }

    /// Broadcast a message to all clients subscribed to an asset.
    pub fn broadcast(&self, asset: &str, message: &str) {
        let senders = self.get_subscribers(asset);
        for tx in senders {
            let _ = tx.send(message.to_string());
        }
    }

    /// Broadcast a message to all connected clients.
    pub fn broadcast_all(&self, message: &str) {
        for client in self.clients.iter() {
            let _ = client.tx.send(message.to_string());
        }
    }

    /// Get the number of connected clients.
    pub fn client_count(&self) -> usize {
        self.clients.len()
    }

    /// Get the number of active rooms (assets with subscribers).
    pub fn room_count(&self) -> usize {
        self.rooms.iter().filter(|r| !r.is_empty()).count()
    }

    /// Get all assets that have at least one subscriber.
    pub fn active_assets(&self) -> Vec<String> {
        self.rooms
            .iter()
            .filter(|r| !r.is_empty())
            .map(|r| r.key().clone())
            .collect()
    }
}

impl Default for RoomManager {
    fn default() -> Self {
        Self {
            clients: DashMap::new(),
            rooms: DashMap::new(),
            trading_rooms: DashMap::new(),
            gridline_rooms: DashMap::new(),
        }
    }
}
