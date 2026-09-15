//! Ferrite Health — composable health indicators and a ready-made
//! `/health` endpoint for Ferrite apps.
//!
//! Design follows NestJS `@nestjs/terminus`: any crate can register a
//! [`HealthIndicator`] via the inventory registry. The built-in
//! [`HealthService`] gathers every registered indicator, runs them in
//! parallel, and reports an aggregate status with per-indicator details.
//!
//! # Usage
//!
//! ```ignore
//! use ferrite_health::{HealthIndicator, Health, async_trait, submit_indicator};
//!
//! #[derive(Default)]
//! pub struct MyIndicator;
//!
//! #[async_trait]
//! impl HealthIndicator for MyIndicator {
//!     fn name(&self) -> &'static str { "custom" }
//!     async fn check(&self) -> HealthResult {
//!         Health::up().finish()
//!     }
//! }
//!
//! submit_indicator!(MyIndicator);
//! ```
//!
//! Then add `HealthModule` to your root module imports. `GET /health`
//! returns a JSON body shaped like:
//!
//! ```json
//! { "status": "UP", "indicators": { "db": { "status": "UP" }, "custom": { "status": "UP" } } }
//! ```

use std::any::TypeId;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

pub use async_trait::async_trait;
use ferrite_framework::__export::inventory;
use ferrite_framework::extract::Json as HttpJson;
use ferrite_macros::{controller, impl_controller, inject, injectable, module};
use serde::Serialize;

type HealthCheckFuture = Pin<Box<dyn std::future::Future<Output = (String, HealthResult)> + Send>>;

/// Result of checking a single [`HealthIndicator`].
pub struct HealthResult {
    pub status: HealthStatus,
    pub details: serde_json::Value,
}

impl HealthResult {
    fn to_entry(&self) -> serde_json::Value {
        let mut obj = serde_json::Map::new();
        obj.insert("status".into(), self.status.as_str().into());
        if !self.details.is_null() {
            obj.insert("details".into(), self.details.clone());
        }
        serde_json::Value::Object(obj)
    }
}

/// Overall endpoint status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum HealthStatus {
    Up,
    Down,
    OutOfService,
    Unknown,
}

impl HealthStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            HealthStatus::Up => "UP",
            HealthStatus::Down => "DOWN",
            HealthStatus::OutOfService => "OUT_OF_SERVICE",
            HealthStatus::Unknown => "UNKNOWN",
        }
    }
}

/// Builder returned by [`Health::up`] / [`Health::down`].
pub struct HealthBuilder {
    status: HealthStatus,
    details: serde_json::Value,
}

impl HealthBuilder {
    pub fn with_details(mut self, details: impl serde::Serialize) -> Self {
        match serde_json::to_value(&details) {
            Ok(v) => self.details = v,
            Err(_) => self.details = serde_json::Value::Null,
        }
        self
    }
    pub fn finish(self) -> HealthResult {
        HealthResult {
            status: self.status,
            details: self.details,
        }
    }
}

pub struct Health;
impl Health {
    pub fn up() -> HealthBuilder {
        HealthBuilder {
            status: HealthStatus::Up,
            details: serde_json::Value::Null,
        }
    }
    pub fn down() -> HealthBuilder {
        HealthBuilder {
            status: HealthStatus::Down,
            details: serde_json::Value::Null,
        }
    }
    pub fn out_of_service() -> HealthBuilder {
        HealthBuilder {
            status: HealthStatus::OutOfService,
            details: serde_json::Value::Null,
        }
    }
    pub fn unknown() -> HealthBuilder {
        HealthBuilder {
            status: HealthStatus::Unknown,
            details: serde_json::Value::Null,
        }
    }
}

#[async_trait]
pub trait HealthIndicator: Send + Sync + 'static {
    fn name(&self) -> &'static str;
    async fn check(&self) -> HealthResult;
}

/// Register a [`HealthIndicator`] (that is `Default`) in the global
/// inventory so [`HealthService`] discovers it automatically.
#[macro_export]
macro_rules! submit_indicator {
    ($t:ty) => {
        const _: () = {
            fn __type_id() -> ::std::any::TypeId {
                ::std::any::TypeId::of::<$t>()
            }
            fn __name() -> &'static str {
                <$t as ::std::default::Default>::default().name()
            }
            fn __make() -> ::std::sync::Arc<dyn $crate::HealthIndicator> {
                ::std::sync::Arc::new(<$t as ::std::default::Default>::default())
                    as ::std::sync::Arc<dyn $crate::HealthIndicator>
            }
            ::ferrite_framework::__export::inventory::submit! {
                $crate::HealthIndicatorEntry {
                    type_id: __type_id,
                    name: __name,
                    make: __make,
                }
            }
        };
    };
}

pub struct HealthIndicatorEntry {
    #[doc(hidden)]
    pub type_id: fn() -> TypeId,
    #[doc(hidden)]
    pub name: fn() -> &'static str,
    #[doc(hidden)]
    pub make: fn() -> Arc<dyn HealthIndicator>,
}

inventory::collect!(HealthIndicatorEntry);

fn iter_indicators() -> Vec<&'static HealthIndicatorEntry> {
    inventory::iter::<HealthIndicatorEntry>().collect()
}

#[injectable]
pub struct HealthService {
    _anchor: u8,
}

impl Default for HealthService {
    fn default() -> Self {
        Self::new()
    }
}

impl HealthService {
    #[inject]
    pub fn new() -> Self {
        Self {
            _anchor: Arc::new(0),
        }
    }

    pub async fn check(&self) -> AggregateHealth {
        let entries = iter_indicators();
        let n = entries.len();
        let mut futs: Vec<HealthCheckFuture> = Vec::with_capacity(n);
        for entry in &entries {
            let ind = (entry.make)();
            let name = (entry.name)().to_string();
            futs.push(Box::pin(async move {
                let res = ind.check().await;
                (name, res)
            }));
        }
        let results = join_all(futs).await;
        let mut aggregate = HealthStatus::Up;
        let mut indicators = serde_json::Map::new();
        for (name, res) in results {
            match res.status {
                HealthStatus::Down => aggregate = HealthStatus::Down,
                HealthStatus::OutOfService if aggregate == HealthStatus::Up => {
                    aggregate = HealthStatus::OutOfService
                }
                HealthStatus::Unknown if aggregate == HealthStatus::Up => {
                    aggregate = HealthStatus::Unknown
                }
                _ => {}
            }
            indicators.insert(name, res.to_entry());
        }
        AggregateHealth {
            status: aggregate,
            indicators: serde_json::Value::Object(indicators),
        }
    }
}

fn join_all<T>(futs: Vec<Pin<Box<dyn std::future::Future<Output = T> + Send>>>) -> JoinAll<T> {
    let n = futs.len();
    JoinAll {
        futs,
        results: Vec::with_capacity(n),
        done: false,
    }
}

struct JoinAll<T> {
    futs: Vec<Pin<Box<dyn std::future::Future<Output = T> + Send>>>,
    results: Vec<Option<T>>,
    done: bool,
}

impl<T> Unpin for JoinAll<T> {}

impl<T> std::future::Future for JoinAll<T> {
    type Output = Vec<T>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if this.done {
            panic!("JoinAll polled after completion");
        }
        let mut all_ready = true;
        let n = this.futs.len();
        if this.results.len() != n {
            this.results.resize_with(n, || None);
        }
        for i in 0..n {
            if this.results[i].is_some() {
                continue;
            }
            match this.futs[i].as_mut().poll(cx) {
                Poll::Ready(v) => this.results[i] = Some(v),
                Poll::Pending => all_ready = false,
            }
        }
        if all_ready {
            this.done = true;
            let out: Vec<T> = std::mem::take(&mut this.results)
                .into_iter()
                .map(|v| v.expect("all_ready but one missing"))
                .collect();
            Poll::Ready(out)
        } else {
            Poll::Pending
        }
    }
}

#[derive(Debug, Serialize)]
pub struct AggregateHealth {
    pub status: HealthStatus,
    pub indicators: serde_json::Value,
}

#[controller("/health")]
pub struct HealthController {
    service: HealthService,
}

#[impl_controller]
impl HealthController {
    #[inject]
    pub fn new(service: HealthService) -> Self {
        Self { service }
    }

    #[ferrite_macros::get("/")]
    pub async fn check(&self) -> HttpJson<AggregateHealth> {
        let report = self.service.check().await;
        HttpJson(report)
    }
}

#[derive(Default)]
pub struct LivenessIndicator;

#[async_trait]
impl HealthIndicator for LivenessIndicator {
    fn name(&self) -> &'static str {
        "app"
    }
    async fn check(&self) -> HealthResult {
        Health::up()
            .with_details(serde_json::json!({
                "message": "ferrite ok",
            }))
            .finish()
    }
}

submit_indicator!(LivenessIndicator);

#[module(
    controllers = [HealthController],
    providers = [HealthService],
)]
pub struct HealthModule;
