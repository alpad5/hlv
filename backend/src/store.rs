use crate::models::{Comment, Thread};
use redis::{aio::ConnectionManager, AsyncCommands, Script};
use std::time::{SystemTime, UNIX_EPOCH};

const GEO_KEY: &str = "feed:geo";
const INACTIVITY_TTL_SECS: i64 = 15 * 60; // 15 minutes
const MAX_LIFETIME_SECS: i64 = 60 * 60;   // 1 hour hard cap

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

pub async fn save_thread(
    con: &mut ConnectionManager,
    thread: &Thread,
) -> redis::RedisResult<()> {
    let key = format!("thread:{}", thread.id);
    let ttl = INACTIVITY_TTL_SECS;

    redis::pipe()
        .hset_multiple(
            &key,
            &[
                ("id", thread.id.clone()),
                ("content", thread.content.clone()),
                ("lat", thread.lat.to_string()),
                ("lng", thread.lng.to_string()),
                ("created_at", thread.created_at.to_string()),
                ("expires_at", thread.expires_at.to_string()),
                ("last_activity", thread.last_activity.to_string()),
                ("comment_count", thread.comment_count.to_string()),
            ],
        )
        .expire(&key, ttl)
        .ignore()
        // Add to geo index (no TTL on geo set; we filter stale entries by checking thread key)
        .geo_add(GEO_KEY, (thread.lng, thread.lat, thread.id.clone()))
        .ignore()
        .query_async(con)
        .await
}

/// Atomically add a comment and reset the thread TTL (bounded by hard cap).
/// Returns false if the thread no longer exists.
pub async fn add_comment(
    con: &mut ConnectionManager,
    comment: &Comment,
) -> redis::RedisResult<bool> {
    let thread_key = format!("thread:{}", comment.thread_id);
    let comments_key = format!("thread:{}:comments", comment.thread_id);

    // Lua script: atomic TTL reset bounded by hard cap
    let script = Script::new(
        r#"
        local thread_key = KEYS[1]
        local comments_key = KEYS[2]
        local comment_json = ARGV[1]
        local now = tonumber(ARGV[2])
        local inactivity_ttl = tonumber(ARGV[3])
        local max_lifetime = tonumber(ARGV[4])

        if redis.call('EXISTS', thread_key) == 0 then
            return 0
        end

        local created_at = tonumber(redis.call('HGET', thread_key, 'created_at'))
        local hard_expires_at = created_at + max_lifetime
        local remaining = hard_expires_at - now

        if remaining <= 0 then
            return 0
        end

        local new_ttl = math.min(inactivity_ttl, remaining)
        redis.call('HSET', thread_key, 'last_activity', now)
        redis.call('HINCRBY', thread_key, 'comment_count', 1)
        redis.call('EXPIRE', thread_key, new_ttl)
        redis.call('RPUSH', comments_key, comment_json)
        redis.call('EXPIRE', comments_key, new_ttl)

        return 1
        "#,
    );

    let comment_json = serde_json::to_string(comment).unwrap();
    let result: i64 = script
        .key(&thread_key)
        .key(&comments_key)
        .arg(&comment_json)
        .arg(now())
        .arg(INACTIVITY_TTL_SECS)
        .arg(MAX_LIFETIME_SECS)
        .invoke_async(con)
        .await?;

    Ok(result == 1)
}

pub async fn get_thread(
    con: &mut ConnectionManager,
    thread_id: &str,
) -> redis::RedisResult<Option<Thread>> {
    let key = format!("thread:{thread_id}");
    let fields: Vec<String> = con.hgetall(&key).await?;
    if fields.is_empty() {
        return Ok(None);
    }
    Ok(parse_thread_fields(&fields))
}

pub async fn get_comments(
    con: &mut ConnectionManager,
    thread_id: &str,
) -> redis::RedisResult<Vec<Comment>> {
    let key = format!("thread:{thread_id}:comments");
    let raw: Vec<String> = con.lrange(&key, 0, -1).await?;
    Ok(raw
        .iter()
        .filter_map(|s| serde_json::from_str(s).ok())
        .collect())
}

pub async fn get_feed(
    con: &mut ConnectionManager,
    lat: f64,
    lng: f64,
    radius_km: f64,
) -> redis::RedisResult<Vec<Thread>> {
    // GEOSEARCH returns member names within radius
    let ids: Vec<String> = redis::cmd("GEOSEARCH")
        .arg(GEO_KEY)
        .arg("FROMLONLAT")
        .arg(lng)
        .arg(lat)
        .arg("BYRADIUS")
        .arg(radius_km)
        .arg("km")
        .arg("ASC")
        .query_async(con)
        .await?;

    let mut threads = Vec::new();
    for id in ids {
        if let Ok(Some(t)) = get_thread(con, &id).await {
            threads.push(t);
        }
        // if thread key is gone (expired), we skip it; geo entry is stale but harmless
    }

    Ok(threads)
}

fn parse_thread_fields(fields: &[String]) -> Option<Thread> {
    let mut map = std::collections::HashMap::new();
    let mut iter = fields.iter();
    while let (Some(k), Some(v)) = (iter.next(), iter.next()) {
        map.insert(k.as_str(), v.as_str());
    }

    Some(Thread {
        id: map.get("id")?.to_string(),
        content: map.get("content")?.to_string(),
        lat: map.get("lat")?.parse().ok()?,
        lng: map.get("lng")?.parse().ok()?,
        created_at: map.get("created_at")?.parse().ok()?,
        expires_at: map.get("expires_at")?.parse().ok()?,
        last_activity: map.get("last_activity")?.parse().ok()?,
        comment_count: map.get("comment_count")?.parse().ok()?,
    })
}
