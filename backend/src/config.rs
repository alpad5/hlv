use std::env;

/// Server-wide configuration loaded from environment variables at startup.
/// Every field has a default so the server runs without any env setup.
#[derive(Clone)]
pub struct Config {
    /// How long a thread stays alive without activity (seconds).
    pub inactivity_ttl_secs: i64,
    /// Maximum thread lifetime from creation, regardless of activity (seconds).
    pub hard_cap_secs: i64,
    /// Default Gaussian jitter applied to posted coordinates (metres).
    pub default_sigma_m: f64,
    /// Upper bound on client-requested sigma (metres).
    pub max_sigma_m: f64,
    /// Maximum character length for thread and comment content.
    pub max_content_len: usize,
    /// Maximum feed radius a client may request (kilometres).
    pub max_radius_km: f64,
}

impl Config {
    /// Read config from environment, falling back to protocol defaults for any missing key.
    pub fn from_env() -> Self {
        Self {
            inactivity_ttl_secs: parse_env("INACTIVITY_TTL_SECS", 1800),
            hard_cap_secs:        parse_env("HARD_CAP_SECS",        3600),
            default_sigma_m:      parse_env("DEFAULT_SIGMA_M",       300.0),
            max_sigma_m:          parse_env("MAX_SIGMA_M",           1000.0),
            max_content_len:      parse_env("MAX_CONTENT_LEN",       300),
            max_radius_km:        parse_env("MAX_RADIUS_KM",         10.0),
        }
    }
}

/// Parse a typed value from an env var, returning `default` if the var is
/// absent or unparseable — so a bad value never silently becomes 0.
fn parse_env<T: std::str::FromStr>(key: &str, default: T) -> T {
    env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
