use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::{
    geo::fuzz_coordinates,
    models::{CreateThread, FeedQuery, Thread},
    store::{get_feed, save_thread},
    ws::{broadcast, WsEvent},
    AppState,
};

pub async fn post_thread(
    State(state): State<AppState>,
    Json(body): Json<CreateThread>,
) -> Result<Json<Thread>, StatusCode> {
    // Reject content that exceeds the configured character limit.
    if body.content.len() > state.config.max_content_len {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    // Use the client-supplied sigma if provided, otherwise fall back to the default.
    // Clamped to 0–max_sigma_m so the client can't request absurd noise levels.
    let sigma = body.noise_sigma
        .unwrap_or(state.config.default_sigma_m)
        .clamp(0.0, state.config.max_sigma_m);
    let (fuzzed_lat, fuzzed_lng) = fuzz_coordinates(body.lat, body.lng, sigma);

    let thread = Thread {
        id: Uuid::new_v4().to_string(),
        content: body.content,
        lat: fuzzed_lat,
        lng: fuzzed_lng,
        created_at: now,
        expires_at: now + state.config.hard_cap_secs,
        last_activity: now,
        comment_count: 0,
    };

    let mut con = state.redis.clone();
    save_thread(&mut con, &thread, state.config.inactivity_ttl_secs)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Broadcast to nearby connected clients
    let event = WsEvent::NewThread {
        data: serde_json::to_value(&thread).unwrap(),
    };
    broadcast(&state.clients, thread.lat, thread.lng, event).await;

    Ok(Json(thread))
}

pub async fn get_feed_handler(
    State(state): State<AppState>,
    Query(params): Query<FeedQuery>,
) -> Result<Json<Vec<Thread>>, StatusCode> {
    let radius = params.radius_km.clamp(1.0, state.config.max_radius_km);
    let mut con = state.redis.clone();

    let threads = get_feed(&mut con, params.lat, params.lng, radius)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(threads))
}
