mod config;
mod geo;
mod models;
mod routes;
mod store;
mod ws;

use axum::{
    routing::{get, post},
    Router,
};
use config::Config;
use redis::aio::ConnectionManager;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;

#[derive(Clone)]
pub struct AppState {
    pub redis: ConnectionManager,
    pub clients: ws::ClientMap,
    pub config: Config,
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
        config: Config::from_env(),
    };

    // Background task: every 30 seconds, scan the geo index for thread keys
    // that Redis has expired, clean them up, and push thread_expired events
    // so connected clients remove stale threads without needing a refresh.
    let sweep_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            let mut con = sweep_state.redis.clone();
            match store::sweep_expired_threads(&mut con).await {
                Ok(expired) => {
                    for thread_id in expired {
                        ws::broadcast_all(
                            &sweep_state.clients,
                            ws::WsEvent::ThreadExpired { thread_id },
                        )
                        .await;
                    }
                }
                Err(e) => tracing::warn!("sweep_expired_threads error: {e}"),
            }
        }
    });

    let app = Router::new()
        .route("/threads", post(routes::threads::post_thread))
        .route("/feed", get(routes::threads::get_feed_handler))
        .route("/threads/:id/comments", post(routes::comments::post_comment))
        .route("/threads/:id/comments", get(routes::comments::get_comments_handler))
        .route("/ws", get(ws::ws_handler))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{port}");
    tracing::info!("Listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
