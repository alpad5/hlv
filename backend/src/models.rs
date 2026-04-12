use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Thread {
    pub id: String,
    pub content: String,
    pub lat: f64,
    pub lng: f64,
    pub created_at: i64,   // unix timestamp
    pub expires_at: i64,   // unix timestamp (hard cap: created_at + 3600)
    pub last_activity: i64, // unix timestamp (resets on comment)
    pub comment_count: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Comment {
    pub id: String,
    pub thread_id: String,
    pub content: String,
    pub created_at: i64,
}

// POST /threads
#[derive(Debug, Deserialize)]
pub struct CreateThread {
    pub content: String,
    pub lat: f64,
    pub lng: f64,
    // How much Gaussian noise to apply to the post location, in metres.
    // Optional — defaults to 300m on the backend if not provided.
    pub noise_sigma: Option<f64>,
}

// POST /threads/:id/comments
#[derive(Debug, Deserialize)]
pub struct CreateComment {
    pub content: String,
}

// GET /feed
#[derive(Debug, Deserialize)]
pub struct FeedQuery {
    pub lat: f64,
    pub lng: f64,
    pub radius_km: f64,
}
