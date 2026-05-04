use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::{
    models::{Comment, CreateComment},
    store::{add_comment, get_comments, get_thread},
    ws::{broadcast, WsEvent},
    AppState,
};

pub async fn post_comment(
    State(state): State<AppState>,
    Path(thread_id): Path<String>,
    Json(body): Json<CreateComment>,
) -> Result<Json<Comment>, StatusCode> {
    // Reject content that exceeds the configured character limit.
    if body.content.len() > state.config.max_content_len {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let comment = Comment {
        id: Uuid::new_v4().to_string(),
        thread_id: thread_id.clone(),
        content: body.content,
        created_at: now,
    };

    let mut con = state.redis.clone();
    let saved = add_comment(&mut con, &comment, state.config.inactivity_ttl_secs, state.config.hard_cap_secs)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !saved {
        return Err(StatusCode::NOT_FOUND);
    }

    // Fetch thread location to determine which clients are nearby
    if let Ok(Some(thread)) = get_thread(&mut con, &thread_id).await {
        let event = WsEvent::NewComment {
            thread_id: thread_id.clone(),
            data: serde_json::to_value(&comment).unwrap(),
        };
        broadcast(&state.clients, thread.lat, thread.lng, event).await;
    }

    Ok(Json(comment))
}

pub async fn get_comments_handler(
    State(state): State<AppState>,
    Path(thread_id): Path<String>,
) -> Result<Json<Vec<Comment>>, StatusCode> {
    let mut con = state.redis.clone();
    let comments = get_comments(&mut con, &thread_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(comments))
}
