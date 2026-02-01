use dashmap::DashMap;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

/// A client's subscription information.
pub struct ClientSubscription {
    /// Subscribed asset symbols.
    pub assets: HashSet<String>,
    /// Channel to send messages to the client.
    pub tx: mpsc::UnboundedSender<String>,
}

/// Manages WebSocket client subscriptions.
pub struct RoomManager {
    /// Client subscriptions keyed by client ID.
    pub clients: DashMap<Uuid, ClientSubscription>,
    /// Asset rooms: asset symbol -> set of client IDs.
    rooms: DashMap<String, HashSet<Uuid>>,
}

impl RoomManager {
    /// Create a new room manager.
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            clients: DashMap::new(),
            rooms: DashMap::new(),
        })
    }

    /// Register a new client.
    pub fn register(&self, tx: mpsc::UnboundedSender<String>) -> Uuid {
        let client_id = Uuid::new_v4();
        self.clients.insert(client_id, ClientSubscription {
            assets: HashSet::new(),
            tx,
        });
        client_id
    }

    /// Unregister a client and remove from all rooms.
    pub fn unregister(&self, client_id: Uuid) {
        if let Some((_, subscription)) = self.clients.remove(&client_id) {
            for asset in subscription.assets {
                if let Some(mut room) = self.rooms.get_mut(&asset) {
                    room.remove(&client_id);
                }
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
                    self.rooms
                        .entry(asset_lower)
                        .or_insert_with(HashSet::new)
                        .insert(client_id);
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

        let client_ids: Vec<Uuid> = self.rooms
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
        }
    }
}
