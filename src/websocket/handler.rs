use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::types::{ClientMessage, ServerMessage};
use crate::AppState;

/// WebSocket upgrade handler.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // Create a channel for sending messages to this client
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    // Register the client
    let client_id = state.room_manager.register(tx);
    info!("WebSocket client connected: {}", client_id);

    // Spawn a task to forward messages from the channel to the WebSocket
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    // Clone state for the broadcast task
    let state_clone = state.clone();
    let room_manager = state.room_manager.clone();

    // Spawn a task to broadcast price updates to subscribed clients
    let mut price_rx = state.coordinator.subscribe();
    let broadcast_client_id = client_id;
    let broadcast_room_manager = room_manager.clone();

    // Clone chart_store for broadcast task to calculate change_24h
    let broadcast_chart_store = state.chart_store.clone();

    let broadcast_task = tokio::spawn(async move {
        while let Ok(mut price_update) = price_rx.recv().await {
            let symbol = price_update.symbol.clone();

            // Check if this client is subscribed to this asset
            if let Some(client) = broadcast_room_manager.clients.get(&broadcast_client_id) {
                if !client.assets.contains(&symbol) {
                    continue;
                }
            } else {
                break;
            }

            // Enrich with change_24h from chart store if not already set
            if price_update.change_24h.is_none() {
                if let Some(change) = broadcast_chart_store.get_price_change(&symbol, 86400) {
                    price_update.change_24h = Some(change);
                }
            }

            let msg = ServerMessage::PriceUpdate {
                data: price_update.into(),
            };

            if let Ok(json) = serde_json::to_string(&msg) {
                let _ = broadcast_room_manager.broadcast(&symbol, &json);
            }
        }
    });

    // Handle incoming messages
    while let Some(result) = receiver.next().await {
        match result {
            Ok(Message::Text(text)) => {
                debug!("Received message from {}: {}", client_id, text);
                handle_message(&state_clone, client_id, &text).await;
            }
            Ok(Message::Close(_)) => {
                info!("WebSocket client disconnecting: {}", client_id);
                break;
            }
            Ok(Message::Ping(_)) => {
                // Pong is handled automatically by axum
                debug!("Received ping from {}", client_id);
            }
            Err(e) => {
                error!("WebSocket error for {}: {}", client_id, e);
                break;
            }
            _ => {}
        }
    }

    // Clean up
    state.room_manager.unregister(client_id);
    send_task.abort();
    broadcast_task.abort();
    info!("WebSocket client disconnected: {}", client_id);
}

async fn handle_message(state: &AppState, client_id: Uuid, text: &str) {
    let msg: ClientMessage = match serde_json::from_str(text) {
        Ok(m) => m,
        Err(e) => {
            send_error(state, client_id, &format!("Invalid message: {}", e));
            return;
        }
    };

    match msg {
        ClientMessage::Subscribe { assets } => {
            let subscribed = state.room_manager.subscribe(client_id, &assets);
            debug!("Client {} subscribed to: {:?}", client_id, subscribed);

            // Subscribe to Coinbase WebSocket for these assets
            state.coordinator.subscribe_assets(&subscribed).await;

            let response = ServerMessage::Subscribed { assets: subscribed };
            send_message(state, client_id, &response);
        }
        ClientMessage::Unsubscribe { assets } => {
            let unsubscribed = state.room_manager.unsubscribe(client_id, &assets);
            debug!("Client {} unsubscribed from: {:?}", client_id, unsubscribed);

            let response = ServerMessage::Unsubscribed { assets: unsubscribed };
            send_message(state, client_id, &response);
        }
    }
}

fn send_message(state: &AppState, client_id: Uuid, msg: &ServerMessage) {
    if let Ok(json) = serde_json::to_string(msg) {
        if let Some(client) = state.room_manager.clients.get(&client_id) {
            let _ = client.tx.send(json);
        }
    }
}

fn send_error(state: &AppState, client_id: Uuid, error: &str) {
    let msg = ServerMessage::Error {
        error: error.to_string(),
    };
    send_message(state, client_id, &msg);
}
