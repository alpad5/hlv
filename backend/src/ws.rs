use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use futures_util::sink::SinkExt;
use futures_util::stream::{SplitSink, SplitStream, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use crate::geo::haversine_km;

/// A connected WebSocket client with its last known location.
pub struct WsClient {
    pub lat: f64,
    pub lng: f64,
    pub radius_km: f64,
    pub tx: mpsc::UnboundedSender<String>,
}

pub type ClientMap = Arc<RwLock<HashMap<Uuid, WsClient>>>;

/// Outbound event pushed from server to client.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsEvent {
    NewThread { data: serde_json::Value },
    NewComment { thread_id: String, data: serde_json::Value },
    // Sent when a thread's Redis key expires so clients can remove it from
    // their feed without waiting for a refresh.
    ThreadExpired { thread_id: String },
}

/// Inbound message from client to register/update location.
#[derive(Deserialize)]
struct LocationUpdate {
    lat: f64,
    lng: f64,
    radius_km: f64,
}

/// Broadcast an event to all clients whose radius covers the given coordinates.
pub async fn broadcast(clients: &ClientMap, thread_lat: f64, thread_lng: f64, event: WsEvent) {
    let json = match serde_json::to_string(&event) {
        Ok(s) => s,
        Err(_) => return,
    };
    let map = clients.read().await;
    for client in map.values() {
        if haversine_km(thread_lat, thread_lng, client.lat, client.lng) <= client.radius_km {
            let _ = client.tx.send(json.clone());
        }
    }
}

/// Broadcast an event to every connected client regardless of location.
/// Used for thread_expired: any client could have the thread in their feed,
/// and a client that doesn't will simply no-op the filter on their end.
pub async fn broadcast_all(clients: &ClientMap, event: WsEvent) {
    let json = match serde_json::to_string(&event) {
        Ok(s) => s,
        Err(_) => return,
    };
    let map = clients.read().await;
    for client in map.values() {
        let _ = client.tx.send(json.clone());
    }
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<crate::AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state.clients))
}

async fn handle_socket(socket: WebSocket, clients: ClientMap) {
    let id = Uuid::new_v4();
    let (tx, rx) = mpsc::unbounded_channel::<String>();

    let (sink, stream): (SplitSink<WebSocket, Message>, SplitStream<WebSocket>) = socket.split();

    clients.write().await.insert(
        id,
        WsClient { lat: 0.0, lng: 0.0, radius_km: 5.0, tx },
    );

    let send_task = tokio::spawn(run_sender(rx, sink));
    run_receiver(stream, &clients, id).await;

    clients.write().await.remove(&id);
    send_task.abort();
}

async fn run_sender(
    mut rx: mpsc::UnboundedReceiver<String>,
    mut sink: SplitSink<WebSocket, Message>,
) {
    while let Some(msg) = rx.recv().await {
        if sink.send(Message::Text(msg.into())).await.is_err() {
            break;
        }
    }
}

async fn run_receiver(
    mut stream: SplitStream<WebSocket>,
    clients: &ClientMap,
    id: Uuid,
) {
    while let Some(result) = stream.next().await {
        let Ok(msg) = result else { break };
        if let Message::Text(text) = msg {
            if let Ok(update) = serde_json::from_str::<LocationUpdate>(&text) {
                let radius = update.radius_km.clamp(1.0, 10.0);
                if let Some(client) = clients.write().await.get_mut(&id) {
                    client.lat = update.lat;
                    client.lng = update.lng;
                    client.radius_km = radius;
                }
            }
        }
    }
}
