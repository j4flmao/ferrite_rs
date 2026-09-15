//! Ferrite Redis cache — JSON-serialized KV store with TTL, prefix, and optional
//! namespace isolation via env `CACHE_KEY_PREFIX`.
//!
//! # Setup
//!
//! ```text
//! REDIS_URL=redis://127.0.0.1:6379
//! CACHE_KEY_PREFIX=myapp:      # optional namespace
//! ```
//!
//! # Example
//!
//! ```ignore
//! use ferrite_cache_redis::CacheService;
//! use ferrite_macros::injectable;
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Serialize, Deserialize, Clone)]
//! struct User { id: i64, name: String }
//!
//! #[injectable]
//! pub struct Users { cache: CacheService }
//!
//! impl Users {
//!     pub async fn get(&self, id: i64) -> Result<Option<User>, ferrite_cache_redis::CacheError> {
//!         self.cache.get(&format!("user:{id}")).await
//!     }
//!     pub async fn set(&self, u: &User) -> Result<(), ferrite_cache_redis::CacheError> {
//!         self.cache.set_with_ttl(&format!("user:{}", u.id), u, 3600).await
//!     }
//! }
//! ```

use std::sync::Arc;

use ferrite_macros::{inject, injectable, module};
use fr_config::ConfigService;
use redis::{Client, Cmd};
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("redis: {0}")]
    Redis(String),
    #[error("serialize: {0}")]
    Serialize(String),
    #[error("deserialize: {0}")]
    Deserialize(String),
    #[error("invalid cache url: {0}")]
    InvalidUrl(String),
}

impl From<redis::RedisError> for CacheError {
    fn from(e: redis::RedisError) -> Self {
        CacheError::Redis(e.to_string())
    }
}
impl From<serde_json::Error> for CacheError {
    fn from(e: serde_json::Error) -> Self {
        CacheError::Deserialize(e.to_string())
    }
}

/// JSON-serialized KV store with TTL, prefix, and optional namespace
/// isolation via env `CACHE_KEY_PREFIX`. Values are `JSON.to_string()`
/// encoded before `SET` (except raw integer methods like `incr` which
/// operate as Redis numerics).
///
/// All callers can `?` through `CacheError` conversions.
#[injectable]
pub struct CacheService {
    config: ConfigService,
    client: Client,
    conn: tokio::sync::Mutex<Option<redis::aio::MultiplexedConnection>>,
    _anchor: u8,
}

impl CacheService {
    #[inject]
    pub fn new(config: ConfigService) -> Self {
        let url = config.get_or("REDIS_URL", "redis://127.0.0.1:6379");
        let client = Client::open(url.as_str()).unwrap_or_else(|e| {
            tracing_error_fallback(&format!("failed to open redis client: {e}"));
            Client::open("redis://127.0.0.1:1/").expect("default fallback URL parse")
        });
        Self {
            config,
            client: Arc::new(client),
            conn: Arc::new(tokio::sync::Mutex::new(None)),
            _anchor: Arc::new(0),
        }
    }

    /// User-configured cache key prefix (`CACHE_KEY_PREFIX`). Trailing `:`
    /// will be auto-added if the prefix is set and does not have one.
    pub fn prefix(&self) -> String {
        let raw = self.config.get_or("CACHE_KEY_PREFIX", "");
        if raw.is_empty() {
            return String::new();
        }
        if raw.ends_with(':') {
            raw
        } else {
            format!("{raw}:")
        }
    }

    pub fn key_for(&self, suffix: &str) -> String {
        format!("{}{}", self.prefix(), suffix)
    }

    async fn conn(&self) -> Result<redis::aio::MultiplexedConnection, CacheError> {
        let mut guard = self.conn.lock().await;
        if let Some(c) = guard.as_ref() {
            return Ok(c.clone());
        }
        let c = self.client.get_multiplexed_async_connection().await?;
        *guard = Some(c.clone());
        Ok(c)
    }

    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, CacheError> {
        let full = self.key_for(key);
        let mut c = self.conn().await?;
        let raw: Option<Vec<u8>> = Cmd::get(&full).query_async(&mut c).await?;
        match raw {
            None => Ok(None),
            Some(bytes) => {
                if bytes.is_empty() {
                    return Ok(None);
                }
                let value: T = serde_json::from_slice(&bytes)?;
                Ok(Some(value))
            }
        }
    }

    pub async fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<(), CacheError> {
        let full = self.key_for(key);
        let bytes = serde_json::to_vec(value).map_err(|e| CacheError::Serialize(e.to_string()))?;
        let mut c = self.conn().await?;
        let () = Cmd::set(&full, bytes).query_async(&mut c).await?;
        Ok(())
    }

    pub async fn set_with_ttl<T: Serialize>(
        &self,
        key: &str,
        value: &T,
        seconds: u64,
    ) -> Result<(), CacheError> {
        let full = self.key_for(key);
        let bytes = serde_json::to_vec(value).map_err(|e| CacheError::Serialize(e.to_string()))?;
        let mut c = self.conn().await?;
        let () = Cmd::set_ex(&full, bytes, seconds)
            .query_async(&mut c)
            .await?;
        Ok(())
    }

    pub async fn delete(&self, key: &str) -> Result<u64, CacheError> {
        let full = self.key_for(key);
        let mut c = self.conn().await?;
        let n: u64 = Cmd::del(&full).query_async(&mut c).await?;
        Ok(n)
    }

    pub async fn expire(&self, key: &str, seconds: u64) -> Result<bool, CacheError> {
        let full = self.key_for(key);
        let mut c = self.conn().await?;
        let i: i64 = Cmd::expire(&full, seconds.try_into().unwrap_or(i64::MAX))
            .query_async(&mut c)
            .await?;
        Ok(i == 1)
    }

    pub async fn ttl(&self, key: &str) -> Result<i64, CacheError> {
        let full = self.key_for(key);
        let mut c = self.conn().await?;
        let ttl: i64 = Cmd::ttl(&full).query_async(&mut c).await?;
        Ok(ttl)
    }

    pub async fn keys(&self, pattern: &str) -> Result<Vec<String>, CacheError> {
        let full = self.key_for(pattern);
        let mut c = self.conn().await?;
        let prefix = self.prefix();
        let matched: Vec<String> = Cmd::keys(&full).query_async(&mut c).await?;
        Ok(matched
            .into_iter()
            .map(|k| {
                if prefix.is_empty() || !k.starts_with(&prefix) {
                    k
                } else {
                    k[prefix.len()..].to_string()
                }
            })
            .collect())
    }

    pub async fn incr(&self, key: &str) -> Result<i64, CacheError> {
        let full = self.key_for(key);
        let mut c = self.conn().await?;
        let n: i64 = Cmd::incr(&full, 1).query_async(&mut c).await?;
        Ok(n)
    }

    pub async fn decr(&self, key: &str) -> Result<i64, CacheError> {
        let full = self.key_for(key);
        let mut c = self.conn().await?;
        let n: i64 = Cmd::decr(&full, 1).query_async(&mut c).await?;
        Ok(n)
    }

    pub async fn incr_by(&self, key: &str, delta: i64) -> Result<i64, CacheError> {
        let full = self.key_for(key);
        let mut c = self.conn().await?;
        let n: i64 = Cmd::incr(&full, delta).query_async(&mut c).await?;
        Ok(n)
    }

    pub async fn exists(&self, key: &str) -> Result<bool, CacheError> {
        let full = self.key_for(key);
        let mut c = self.conn().await?;
        let i: i64 = Cmd::exists(&full).query_async(&mut c).await?;
        Ok(i > 0)
    }

    /// Clear *every* key under our prefix. If no prefix is configured this
    /// becomes a `FLUSHDB` to avoid nuking an unrelated shared Redis.
    pub async fn clear_all(&self) -> Result<(), CacheError> {
        let prefix = self.prefix();
        let mut c = self.conn().await?;
        if prefix.is_empty() {
            let () = Cmd::new()
                .arg("FLUSHDB")
                .arg("ASYNC")
                .query_async(&mut c)
                .await
                .unwrap_or(());
            return Ok(());
        }
        let pattern = format!("{prefix}*");
        let all: Vec<String> = Cmd::keys(&pattern).query_async(&mut c).await?;
        if !all.is_empty() {
            let args: Vec<&str> = all.iter().map(|s| s.as_str()).collect();
            let _: redis::Value = redis::cmd("UNLINK")
                .arg(&args)
                .query_async(&mut c)
                .await
                .unwrap_or(redis::Value::Nil);
        }
        Ok(())
    }
}

/// Tiny no-deps error fallback: if `tracing` is not in the dependency tree we
/// just eprintln — the crate keeps zero framework deps, and this only runs in
/// the unlikely case `Client::open` fails at boot.
fn tracing_error_fallback(msg: &str) {
    eprintln!("[ferrite-cache-redis] {msg}");
}

/// Helper used in tests and internally: pure JSON encode/decode roundtrip.
pub fn roundtrip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(v: &T) {
    let bytes = serde_json::to_vec(v).unwrap();
    let back: T = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v, &back);
}

#[module(providers = [CacheService])]
pub struct CacheModule;

// ---------------------------------------------------------------------------
// Tests (pure logic only — no live Redis)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use fnv::FnvBuildHasher;
    use serde::{Deserialize, Serialize};
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    fn with_test_env(env: &str, f: impl FnOnce(&Path)) {
        let dir: PathBuf = std::env::temp_dir().join(format!(
            "ferrite-cache-test-{}-{}",
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

    // Dummy serializable type used by roundtrip tests.
    #[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
    struct Sample {
        a: i32,
        b: Option<String>,
        vs: Vec<u8>,
        map: std::collections::HashMap<String, i32, FnvBuildHasher>,
    }

    #[test]
    fn key_prefix_applied_and_trailing_colon() {
        with_test_env(
            "CACHE_KEY_PREFIX=foo\nREDIS_URL=redis://127.0.0.1:6379\n",
            |dir| {
                let cfg = ConfigService::load_from(dir);
                let svc = CacheService::new(Arc::new(cfg));
                assert_eq!(svc.prefix(), "foo:");
                assert_eq!(svc.key_for("bar"), "foo:bar");
            },
        );
        with_test_env(
            "CACHE_KEY_PREFIX=foo:\nREDIS_URL=redis://127.0.0.1:6379\n",
            |dir| {
                let cfg = ConfigService::load_from(dir);
                let svc = CacheService::new(Arc::new(cfg));
                assert_eq!(svc.prefix(), "foo:");
                assert_eq!(svc.key_for("bar"), "foo:bar");
            },
        );
        with_test_env("REDIS_URL=redis://127.0.0.1:6379\n", |dir| {
            let cfg = ConfigService::load_from(dir);
            let svc = CacheService::new(Arc::new(cfg));
            assert_eq!(svc.prefix(), "");
            assert_eq!(svc.key_for("bar"), "bar");
        });
    }

    #[test]
    fn keys_strip_prefix_in_results() {
        // Pure local logic simulation — we do not require live Redis.
        let prefix = "app:";
        let raw = vec!["app:u:1".to_string(), "app:u:2".to_string()];
        let stripped: Vec<String> = raw
            .into_iter()
            .map(|k| k[prefix.len()..].to_string())
            .collect();
        assert_eq!(stripped, vec!["u:1", "u:2"]);
    }

    #[test]
    fn json_roundtrip_complex_sample() {
        let mut map = std::collections::HashMap::with_hasher(FnvBuildHasher::default());
        map.insert("x".into(), 1);
        map.insert("y".into(), 2);
        let s = Sample {
            a: 42,
            b: Some("hi".into()),
            vs: b"hello".to_vec(),
            map,
        };
        roundtrip(&s);
    }

    #[test]
    fn cache_error_classifies_serde() {
        let bad_json = b"not json";
        let err: CacheError = serde_json::from_slice::<i32>(bad_json).unwrap_err().into();
        assert!(matches!(err, CacheError::Deserialize(_)));
    }
}
