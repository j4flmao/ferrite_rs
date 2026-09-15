//! # Ferrite Cron
//!
//! Scheduled tasks for Ferrite applications. Supports both fixed intervals
//! and standard 5- or 6-field cron expressions.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use ferrite_cron::SchedulerService;
//! use fr_core::Module;
//! use std::time::Duration;
//! use chrono::Utc;
//!
//! // In a module provider/handler:
//! // pub async fn bootstrap(scheduler: Arc<SchedulerService>) {
//! //     scheduler.add_interval("heartbeat", Duration::from_secs(30), || async {
//! //         eprintln!("tick at {:?}", Utc::now());
//! //     }).await.unwrap();
//! //     scheduler.add_cron("cleanup", "0 */5 * * * *", || async {
//! //         eprintln!("cleanup at {:?}", Utc::now());
//! //     }).await.unwrap();
//! //     tokio::spawn(async move { scheduler.run().await });
//! // }
//! ```

use std::any::TypeId;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, TimeZone, Utc};
use cron::Schedule;
use dashmap::DashMap;
use ferrite_config::ConfigService;
use fr_core::{ModuleDescriptor, OnApplicationBootstrap, ProviderEntry, Scope};
use thiserror::Error;
use tokio::task::JoinHandle;

type JobFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;
type JobFn = Arc<dyn Fn() -> JobFuture + Send + Sync + 'static>;

#[derive(Debug, Error)]
pub enum CronError {
    #[error("invalid cron expression: {0}")]
    InvalidCron(String),
    #[error("task named '{0}' is already registered")]
    AlreadyExists(String),
}

enum TaskKind {
    Interval { period: Duration },
    Cron { schedule: Box<Schedule> },
}

struct TaskEntry {
    kind: TaskKind,
    job: JobFn,
    next_run_ts_ms: u64,
    handle: Option<JoinHandle<()>>,
}

pub struct SchedulerService {
    tasks: DashMap<String, TaskEntry>,
    tick_ms: u64,
}

impl Default for SchedulerService {
    fn default() -> Self {
        Self {
            tasks: DashMap::new(),
            tick_ms: 500,
        }
    }
}

impl SchedulerService {
    pub fn new(config: Arc<ConfigService>) -> Self {
        let tick_ms = config.get_or_parse("CRON_TICK_MS", 500u64);
        Self {
            tasks: DashMap::new(),
            tick_ms: tick_ms.max(1),
        }
    }

    /// Register a fixed-interval job. The first tick fires after one period
    /// (to align with scheduler semantics — change manually if needed).
    pub async fn add_interval<F, Fut>(
        &self,
        name: &str,
        period: Duration,
        f: F,
    ) -> Result<(), CronError>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let name = name.to_string();
        if self.tasks.contains_key(&name) {
            return Err(CronError::AlreadyExists(name));
        }
        let job: JobFn = Arc::new(move || Box::pin(f()));
        let now_ms = now_ms();
        let next_run_ts_ms = now_ms.saturating_add(period.as_millis() as u64);
        self.tasks.insert(
            name,
            TaskEntry {
                kind: TaskKind::Interval { period },
                job,
                next_run_ts_ms,
                handle: None,
            },
        );
        Ok(())
    }

    /// Register a cron-scheduled job. Accepts either:
    ///   - 6-field expression  : `sec min hour dom month dow`
    ///   - 5-field expression  : `min hour dom month dow` (seconds auto-padded to 0)
    pub async fn add_cron<F, Fut>(&self, name: &str, expr: &str, f: F) -> Result<(), CronError>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let name = name.to_string();
        if self.tasks.contains_key(&name) {
            return Err(CronError::AlreadyExists(name));
        }
        let normalized = normalize_cron_expr(expr);
        let schedule =
            Schedule::from_str(&normalized).map_err(|e| CronError::InvalidCron(format!("{e}")))?;
        let job: JobFn = Arc::new(move || Box::pin(f()));
        let next = schedule
            .upcoming(Utc)
            .next()
            .ok_or_else(|| CronError::InvalidCron("expression has no upcoming fire time".into()))?;
        self.tasks.insert(
            name,
            TaskEntry {
                kind: TaskKind::Cron {
                    schedule: Box::new(schedule),
                },
                job,
                next_run_ts_ms: datetime_to_ms(next),
                handle: None,
            },
        );
        Ok(())
    }

    /// Run the scheduler loop forever. Typical callers wrap this with
    /// `tokio::spawn` to avoid blocking the application bootstrap.
    pub async fn run(&self) {
        loop {
            self.tick().await;
            tokio::time::sleep(Duration::from_millis(self.tick_ms)).await;
        }
    }

    /// Run a single scheduler tick — useful for deterministic tests.
    pub async fn tick(&self) -> usize {
        let now = now_ms();
        let mut fired = 0usize;
        let mut next_updates: HashMap<String, u64> = HashMap::new();
        for mut entry in self.tasks.iter_mut() {
            let (name, task) = entry.pair_mut();
            if now >= task.next_run_ts_ms {
                let job = task.job.clone();
                let handle = tokio::spawn(async move { job().await });
                task.handle = Some(handle);
                fired += 1;
                let next = match &task.kind {
                    TaskKind::Interval { period } => {
                        // Advance in whole-period steps to avoid drift.
                        let mut n = task.next_run_ts_ms;
                        while n <= now {
                            n = n.saturating_add(period.as_millis() as u64);
                        }
                        n
                    }
                    TaskKind::Cron { schedule } => {
                        let now_dt = ms_to_datetime(now);
                        schedule
                            .upcoming(Utc)
                            .find(|t| *t > now_dt)
                            .map(datetime_to_ms)
                            .unwrap_or(u64::MAX)
                    }
                };
                next_updates.insert(name.clone(), next);
            }
        }
        for (name, next) in next_updates {
            if let Some(mut e) = self.tasks.get_mut(&name) {
                e.next_run_ts_ms = next;
            }
        }
        fired
    }
}

fn normalize_cron_expr(expr: &str) -> String {
    let tokens: Vec<&str> = expr.split_whitespace().collect();
    if tokens.len() == 5 {
        // 5-field: min hour dom month dow  -> prepend seconds=0
        let mut out = vec!["0"];
        out.extend(tokens);
        out.join(" ")
    } else {
        expr.to_string()
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn datetime_to_ms(dt: DateTime<Utc>) -> u64 {
    dt.timestamp_millis().max(0) as u64
}

fn ms_to_datetime(ms: u64) -> DateTime<Utc> {
    let secs = (ms / 1000) as i64;
    let nsecs = ((ms % 1000) * 1_000_000) as u32;
    Utc.timestamp_opt(secs, nsecs)
        .single()
        .unwrap_or_else(|| Utc.timestamp_opt(0, 0).unwrap())
}

inventory::submit! {
    ProviderEntry::new_static::<SchedulerService>(
        "SchedulerService",
        Scope::Singleton,
        || vec![TypeId::of::<ConfigService>()],
        |container| -> std::sync::Arc<dyn std::any::Any + Send + Sync> {
            let cfg = container.get::<ConfigService>();
            std::sync::Arc::new(SchedulerService::new(cfg))
                as std::sync::Arc<dyn std::any::Any + Send + Sync>
        },
    )
}

// ---------------------------------------------------------------------------
// CronModule
// ---------------------------------------------------------------------------

pub struct CronModule;

impl CronModule {
    pub fn for_root() -> CronModuleImpl {
        CronModuleImpl
    }
}

#[derive(Clone, Copy)]
pub struct CronModuleImpl;

impl fr_core::Module for CronModuleImpl {
    fn __module_descriptor() -> ModuleDescriptor {
        ModuleDescriptor {
            name: String::from("CronModule"),
            providers: vec![TypeId::of::<SchedulerService>()],
            controllers: vec![],
            imports: vec![],
            exports: vec![TypeId::of::<SchedulerService>()],
            middleware: vec![],
            global: false,
        }
    }
}

impl OnApplicationBootstrap for CronModuleImpl {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn normalize_five_field_cron_to_six() {
        assert_eq!(normalize_cron_expr("* * * * *"), "0 * * * * *");
        assert_eq!(normalize_cron_expr("0 0 * * *"), "0 0 0 * * *");
        assert_eq!(normalize_cron_expr("1 2 3 4 5 6"), "1 2 3 4 5 6");
    }

    #[test]
    fn cron_next_after_from_fixed_timestamp() {
        let schedule = Schedule::from_str("* * * * * *").unwrap();
        let anchor = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let mut it = schedule.after(&anchor);
        let n1 = it.next().unwrap();
        assert_eq!(n1, Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 1).unwrap());
        let n2 = it.next().unwrap();
        assert_eq!(n2, Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 2).unwrap());
    }

    #[test]
    fn cron_minute_granularity_next_from_anchor() {
        let schedule = Schedule::from_str("0 * * * * *").unwrap();
        let anchor = Utc.with_ymd_and_hms(2024, 6, 15, 12, 30, 45).unwrap();
        let next = schedule.after(&anchor).next().unwrap();
        assert_eq!(next, Utc.with_ymd_and_hms(2024, 6, 15, 12, 31, 0).unwrap());
    }

    #[tokio::test]
    async fn interval_task_fires_in_tick_window() {
        let sched = SchedulerService::default();
        let fired = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let f1 = fired.clone();
        sched
            .add_interval("ping", Duration::from_millis(10), move || {
                let f = f1.clone();
                async move {
                    f.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
            })
            .await
            .unwrap();

        // Sleep longer than one period so next_run_ts_ms is behind "now" when
        // we manually call tick.
        tokio::time::sleep(Duration::from_millis(30)).await;
        let n = sched.tick().await;
        assert!(n >= 1, "expected at least 1 fired tick, got {n}");

        // Wait a little for spawned tasks to actually execute the closure.
        tokio::time::sleep(Duration::from_millis(15)).await;
        let count = fired.load(std::sync::atomic::Ordering::SeqCst);
        assert!(count >= 1, "expected fired count >=1, got {count}");
    }

    #[tokio::test]
    async fn duplicate_task_name_returns_already_exists() {
        let sched = SchedulerService::default();
        sched
            .add_interval("same", Duration::from_millis(50), || async {})
            .await
            .unwrap();
        let err = sched
            .add_interval("same", Duration::from_millis(60), || async {})
            .await
            .unwrap_err();
        assert!(matches!(err, CronError::AlreadyExists(_)));
    }

    #[tokio::test]
    async fn invalid_cron_expression_returns_parse_error() {
        let sched = SchedulerService::default();
        let err = sched
            .add_cron("x", "not a cron", || async {})
            .await
            .unwrap_err();
        assert!(matches!(err, CronError::InvalidCron(_)));
    }
}
