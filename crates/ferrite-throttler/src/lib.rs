//! Ferrite in-memory rate limiter.
//!
//! * `ThrottlerService` — `#[injectable]` core primitive (per-key fixed window
//!   counters stored in a lock-free DashMap). Safe across concurrent
//!   handlers, no Redis required.
//! * `ThrottlerGuard` — Nest-style `Guard` returning `false` on hit (triggers
//!   Ferrite's standard 403).
//! * `ThrottlerInterceptor` — `Interceptor` variant that returns an RFC
//!   compliant `429 Too Many Requests` with `Retry-After`, `X-RateLimit-*`
//!   headers. Prefer this one.
//!
//! Env config:
//! ```text
//! THROTTLER_LIMIT=60            # max hits per window
//! THROTTLER_WINDOW_SECONDS=60   # window length (seconds)
//! ```
//!
//! # Manual usage
//! ```ignore
//! use ferrite_throttler::ThrottlerService;
//! use ferrite_macros::controller;
//!
//! #[controller("/cats")]
//! pub struct CatsController { throttle: ThrottlerService }
//! ```

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use axum::{
    body::Body,
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use dashmap::DashMap;
use ferrite_config::ConfigService;
use ferrite_http::{Guard, HttpError, Interceptor, Next, RequestCtx};
use ferrite_macros::{inject, injectable, module};
use fnv::FnvBuildHasher;
use thiserror::Error;

/// The result of `ThrottlerService::hit` on success.
#[derive(Debug, Clone, Copy)]
pub struct HitOk {
    pub limit: u64,
    pub used: u64,
    pub remaining: u64,
    pub window_reset_at: u64,
    pub retry_after_secs: u64,
}

/// The result of `ThrottlerService::hit` when rate-limited.
#[derive(Debug, Clone, Copy)]
pub struct HitLimit {
    pub limit: u64,
    pub used: u64,
    pub window_reset_at: u64,
    pub retry_after_secs: u64,
}

#[derive(Debug, Error)]
pub enum ThrottlerError {
    #[error("rate limit exceeded: retry after {0}s")]
    TooManyRequests(u64),
}

#[derive(Default)]
struct Bucket {
    start: u64,
    count: u64,
}

/// Core rate-limiter — a `#[injectable]` DashMap of fixed-window counters.
/// Keys are arbitrary strings (typically `${method}${path}:${client_ip}` or
/// `${user_id}`). `FNV` hasher keeps the map light and unbiased for string
/// keys.
#[injectable]
pub struct ThrottlerService {
    config: ConfigService,
    buckets: DashMap<String, Bucket, FnvBuildHasher>,
    _anchor: u8,
}

impl ThrottlerService {
    #[inject]
    pub fn new(config: ConfigService) -> Self {
        Self {
            config,
            buckets: Arc::new(DashMap::default()),
            _anchor: Arc::new(0),
        }
    }

    pub fn default_limit(&self) -> u64 {
        self.config.get_or_parse("THROTTLER_LIMIT", 60)
    }

    pub fn default_window(&self) -> u64 {
        self.config.get_or_parse("THROTTLER_WINDOW_SECONDS", 60)
    }

    pub fn now_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    /// Attempt a single hit for `key` with `limit` requests allowed per
    /// `window_secs`. Uses a fixed window — the window start for `key` is
    /// floored to `now - (now % window)` so that all keys sharing the same
    /// window length reset at consistent wall-clock boundaries.
    pub fn hit(&self, key: &str, limit: u64, window_secs: u64) -> Result<HitOk, HitLimit> {
        let limit = limit.max(1);
        let window = window_secs.max(1);
        let now = Self::now_secs();
        let start = now - (now % window);
        let reset_at = start + window;
        let mut entry = self.buckets.entry(key.to_string()).or_default();
        if entry.start != start {
            entry.start = start;
            entry.count = 0;
        }
        if entry.count >= limit {
            return Err(HitLimit {
                limit,
                used: entry.count,
                window_reset_at: reset_at,
                retry_after_secs: reset_at.saturating_sub(now).max(1),
            });
        }
        entry.count += 1;
        Ok(HitOk {
            limit,
            used: entry.count,
            remaining: limit - entry.count,
            window_reset_at: reset_at,
            retry_after_secs: reset_at.saturating_sub(now).max(1),
        })
    }

    /// Peek at the current count for `key` without incrementing it.
    pub fn peek(&self, key: &str, window_secs: u64) -> (u64, u64) {
        let now = Self::now_secs();
        let window = window_secs.max(1);
        let start = now - (now % window);
        let reset_at = start + window;
        let count = self
            .buckets
            .get(key)
            .filter(|b| b.start == start)
            .map(|b| b.count)
            .unwrap_or(0);
        (count, reset_at.saturating_sub(now).max(1))
    }

    /// Force-reset the counter for `key`.
    pub fn reset(&self, key: &str) {
        self.buckets.remove(key);
    }

    /// Best-effort cleanup: drop any bucket whose window ended more than
    /// `keep_secs` ago. Call this periodically from a background task or
    /// ignore (DashMap entries are bounded by unique key cardinality).
    pub fn purge_old(&self, keep_secs: u64) -> usize {
        let now = Self::now_secs();
        let before = self.buckets.len();
        self.buckets
            .retain(|_, b| now.saturating_sub(b.start) <= keep_secs);
        before.saturating_sub(self.buckets.len())
    }

    /// Build a per-request throttler key out of the request's HTTP method,
    /// matched path, and the client peer IP. If no IP is available the key
    /// falls back to a hash of User-Agent + request path — this makes it
    /// *slightly* less effective behind reverse proxies without
    /// `X-Forwarded-For` but still usable for demos.
    pub fn key_for_request(&self, ctx: &RequestCtx) -> String {
        let method = ctx.method().as_str();
        let path = ctx.uri().path().to_string();
        let ip = ctx
            .headers()
            .get("X-Forwarded-For")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.split(',').next())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| {
                ctx.headers()
                    .get("User-Agent")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("unknown")
                    .to_string()
            });
        format!("{method}{path}:{ip}")
    }
}

// ---------------------------------------------------------------------------
// Guard (short-circuit 403)
// ---------------------------------------------------------------------------

/// Nest-style Guard: `false` ⇒ `403 Forbidden`.
#[derive(Clone)]
pub struct ThrottlerGuard {
    pub service: Arc<ThrottlerService>,
    pub limit: u64,
    pub window: u64,
}

#[async_trait]
impl Guard for ThrottlerGuard {
    async fn can_activate(&self, ctx: &RequestCtx) -> bool {
        let key = self.service.key_for_request(ctx);
        self.service.hit(&key, self.limit, self.window).is_ok()
    }
}

// ---------------------------------------------------------------------------
// Interceptor (429 + Retry-After headers) — use this in production
// ---------------------------------------------------------------------------

/// Preferred integration point: returns 429 with standard headers on hit.
#[derive(Clone)]
pub struct ThrottlerInterceptor {
    pub service: Arc<ThrottlerService>,
    pub limit: u64,
    pub window: u64,
}

#[async_trait]
impl Interceptor for ThrottlerInterceptor {
    async fn intercept(&self, ctx: RequestCtx, next: Next) -> Response {
        let key = self.service.key_for_request(&ctx);
        match self.service.hit(&key, self.limit, self.window) {
            Ok(ok) => {
                let mut res = next.run(ctx).await;
                append_rate_headers(
                    res.headers_mut(),
                    ok.limit,
                    ok.used,
                    ok.remaining,
                    ok.window_reset_at,
                );
                res
            }
            Err(blocked) => too_many_requests(blocked).into_response(),
        }
    }
}

fn append_rate_headers(
    headers: &mut HeaderMap,
    limit: u64,
    used: u64,
    remaining: u64,
    reset_at: u64,
) {
    fn h(_name: &'static str, value: u64) -> HeaderValue {
        HeaderValue::from_str(&value.to_string()).unwrap()
    }
    headers.insert(
        HeaderName::from_static("X-RateLimit-Limit"),
        h("X-RateLimit-Limit", limit),
    );
    headers.insert(
        HeaderName::from_static("X-RateLimit-Used"),
        h("X-RateLimit-Used", used),
    );
    headers.insert(
        HeaderName::from_static("X-RateLimit-Remaining"),
        h("X-RateLimit-Remaining", remaining),
    );
    headers.insert(
        HeaderName::from_static("X-RateLimit-Reset"),
        h("X-RateLimit-Reset", reset_at),
    );
}

fn too_many_requests(blocked: HitLimit) -> impl IntoResponse {
    let retry = blocked.retry_after_secs;
    let body = serde_json::json!({
        "statusCode": 429,
        "message": format!("Rate limit exceeded: {used}/{limit}. Retry after {retry}s.",
            used = blocked.used, limit = blocked.limit),
        "error": "Too Many Requests",
    });

    Response::builder()
        .status(StatusCode::TOO_MANY_REQUESTS)
        .header("Retry-After", retry.to_string())
        .header("X-RateLimit-Limit", blocked.limit.to_string())
        .header("X-RateLimit-Used", blocked.used.to_string())
        .header("X-RateLimit-Remaining", "0")
        .header("X-RateLimit-Reset", blocked.window_reset_at.to_string())
        .body(Body::from(serde_json::to_vec(&body).unwrap_or_default()))
        .unwrap_or_else(|_| HttpError::internal("throttler").into_response())
}

// ---------------------------------------------------------------------------
// Module registration
// ---------------------------------------------------------------------------

/// If added to your module imports: registers [`ThrottlerService`] as a
/// global injectable. Guards/Interceptors are instantiated *per-route* by
/// callers who want them (so you can apply per-route limits), but the
/// service is shared so counters are consistent everywhere.
#[module(providers = [ThrottlerService])]
pub struct ThrottlerModule;

// ---------------------------------------------------------------------------
// Tests (pure logic — DashMap only, no HTTP)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn with_test_env(env: &str, f: impl FnOnce(&Path)) {
        let dir: PathBuf = std::env::temp_dir().join(format!(
            "ferrite-throttle-test-{}-{}",
            std::process::id(),
            rand_suffix()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(".env"), env).unwrap();
        f(&dir);
        let _ = std::fs::remove_dir_all(&dir);
    }
    fn rand_suffix() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        let t = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        t ^ std::process::id() as u64
    }

    #[test]
    fn fixed_window_allows_limit_and_then_blocks() {
        with_test_env("THROTTLER_LIMIT=3\nTHROTTLER_WINDOW_SECONDS=10\n", |dir| {
            let cfg = ConfigService::load_from(dir);
            let svc = ThrottlerService::new(Arc::new(cfg));
            assert_eq!(svc.default_limit(), 3);
            assert_eq!(svc.default_window(), 10);

            // 3 hits OK
            let ok1 = svc.hit("a", 3, 10).unwrap();
            assert_eq!(ok1.used, 1);
            assert_eq!(ok1.remaining, 2);
            svc.hit("a", 3, 10).unwrap();
            let ok3 = svc.hit("a", 3, 10).unwrap();
            assert_eq!(ok3.used, 3);
            assert_eq!(ok3.remaining, 0);

            // 4th blocked
            let blk = svc.hit("a", 3, 10).unwrap_err();
            assert_eq!(blk.limit, 3);
            assert_eq!(blk.used, 3);
            assert!(blk.retry_after_secs >= 1);
        });
    }

    #[test]
    fn independent_keys_dont_interfere() {
        with_test_env("", |dir| {
            let cfg = ConfigService::load_from(dir);
            let svc = ThrottlerService::new(Arc::new(cfg));
            svc.hit("k1", 2, 60).unwrap();
            svc.hit("k1", 2, 60).unwrap();
            // k1 blocked
            assert!(svc.hit("k1", 2, 60).is_err());
            // k2 still fine
            assert!(svc.hit("k2", 2, 60).is_ok());
        });
    }

    #[test]
    fn reset_clears_counter() {
        with_test_env("", |dir| {
            let cfg = ConfigService::load_from(dir);
            let svc = ThrottlerService::new(Arc::new(cfg));
            svc.hit("r", 1, 3600).unwrap();
            assert!(svc.hit("r", 1, 3600).is_err());
            svc.reset("r");
            assert!(svc.hit("r", 1, 3600).is_ok());
        });
    }

    #[test]
    fn purge_old_removes_stale_entries() {
        with_test_env("", |dir| {
            let cfg = ConfigService::load_from(dir);
            let svc = ThrottlerService::new(Arc::new(cfg));
            // Use a 1-second window so the window start for "fresh" is always
            // within the last second, meaning no wall-clock floor makes it
            // look "old".
            svc.hit("fresh", 99, 1).unwrap();
            let now = ThrottlerService::now_secs();
            // Force an old bucket whose window started way before keep_secs.
            svc.buckets.entry("stale".into()).or_default().start = now.saturating_sub(10_000);
            svc.buckets.entry("stale".into()).or_default().count = 5;
            assert_eq!(svc.buckets.len(), 2);
            // keep_secs = 600s: "stale" is purged, "fresh" stays alive.
            let removed = svc.purge_old(600);
            assert_eq!(removed, 1);
            assert_eq!(svc.buckets.len(), 1);
            assert_eq!(svc.peek("fresh", 1).0, 1);
        });
    }
}
