mod geo;
mod models;
mod routes;
mod store;
mod ws;

use axum::{
    routing::{get, post},
    Router,
};
use redis::aio::ConnectionManager;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;

#[derive(Clone)]
pub struct AppState {
    pub redis: ConnectionManager,
    pub clients: ws::ClientMap,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1/".to_string());
    let client = redis::Client::open(redis_url).expect("Invalid Redis URL");
    let manager = ConnectionManager::new(client)
        .await
        .expect("Failed to connect to Redis");

    let state = AppState {
        redis: manager,
        clients: Arc::new(RwLock::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/threads", post(routes::threads::post_thread))
        .route("/feed", get(routes::threads::get_feed_handler))
        .route("/threads/:id/comments", post(routes::comments::post_comment))
        .route("/threads/:id/comments", get(routes::comments::get_comments_handler))
        .route("/ws", get(ws::ws_handler))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = "0.0.0.0:3000";
    tracing::info!("Listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
