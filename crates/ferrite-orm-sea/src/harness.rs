//! App-level migration, revert and seed harness for Ferrite apps using the
//! SeaORM adapter.
//!
//! The CLI (`ferrite db migrate|revert|seed|status`) sets `FERRITE_DB_ACTION`
//! and then runs `cargo run`. Your application binary should call
//! [`run_from_env`] early in `main`, before the HTTP bootstrap. If the env
//! var is set, the harness runs the requested DB action and then exits the
//! process cleanly. If it is unset, [`run_from_env`] is a no-op and control
//! returns to your app bootstrap.
//!
//! # Layout convention
//!
//! ```text
//! migrations/
//!   1710000000_create_users/
//!     up.sql
//!     down.sql
//!   1710000001_create_posts/
//!     up.sql
//!     down.sql
//! seeds/
//!   001_users.sql
//!   002_posts.sql
//! ```
//!
//! Migration directory names must start with an integer version (unix
//! timestamp, monotonically increasing counter …) followed by an underscore
//! and a descriptive name. Applied versions are recorded in a
//! `__ferrite_migrations` table.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use ferrite_config::ConfigService;
use ferrite_orm::OrmError;

use sea_orm::{ConnectionTrait, Database, DatabaseConnection, Statement};

/// One parsed migration directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationFile {
    /// Numeric version prefix (decoded from the directory name's `NNN_…`
    /// part). Used to order and de-duplicate.
    pub version: u64,
    /// Human-readable slug (everything after the first `_`).
    pub name: String,
    /// Absolute/relative path to the directory containing `up.sql` and
    /// `down.sql`.
    pub dir: PathBuf,
}

/// High level summary returned by [`status`] — handy for CLI output.
#[derive(Debug, Default, Clone)]
pub struct StatusReport {
    pub applied: Vec<(u64, String, u64)>,
    pub pending: Vec<(u64, String)>,
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Scan a migrations directory for entries shaped like `<version>_<name>/`.
/// Only entries that contain both an `up.sql` and a `down.sql` file are
/// accepted. Returns entries sorted ascending by version.
pub fn discover(migrations_dir: &Path) -> Result<Vec<MigrationFile>, OrmError> {
    if !migrations_dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(migrations_dir).map_err(|e| OrmError::Backend(e.to_string()))? {
        let entry = entry.map_err(|e| OrmError::Backend(e.to_string()))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(dir_name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let (ver_prefix, rest) = match dir_name.split_once('_') {
            Some(pair) => pair,
            None => continue,
        };
        let Ok(version) = ver_prefix.parse::<u64>() else {
            continue;
        };
        if !path.join("up.sql").exists() || !path.join("down.sql").exists() {
            eprintln!(
                "  \x1b[33mwarn\x1b[0m migration `{dir_name}` missing up.sql/down.sql — skipping"
            );
            continue;
        }
        out.push(MigrationFile {
            version,
            name: rest.to_string(),
            dir: path,
        });
    }
    out.sort_by_key(|a| a.version);
    Ok(out)
}

async fn ensure_meta_table(conn: &DatabaseConnection) -> Result<(), OrmError> {
    let backend = conn.get_database_backend();
    let sql = if backend == sea_orm::DatabaseBackend::MySql {
        r#"CREATE TABLE IF NOT EXISTS __ferrite_migrations (
            version VARCHAR(64) NOT NULL PRIMARY KEY,
            name VARCHAR(255) NOT NULL,
            applied_at BIGINT NOT NULL
        )"#
    } else {
        r#"CREATE TABLE IF NOT EXISTS __ferrite_migrations (
            version TEXT PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            applied_at INTEGER NOT NULL
        )"#
    };
    conn.execute_unprepared(sql)
        .await
        .map(|_| ())
        .map_err(|e| OrmError::Backend(e.to_string()))
}

async fn applied_rows(conn: &DatabaseConnection) -> Result<Vec<(String, String, u64)>, OrmError> {
    ensure_meta_table(conn).await?;
    let backend = conn.get_database_backend();
    let sql = "SELECT version, name, applied_at FROM __ferrite_migrations ORDER BY version ASC";
    let stmt = Statement::from_string(backend, sql);
    let rows = conn
        .query_all_raw(stmt)
        .await
        .map_err(|e| OrmError::Backend(e.to_string()))?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let version: String = r.try_get("", "version").unwrap_or_default();
        let name: String = r.try_get("", "name").unwrap_or_default();
        let applied_at: i64 = r.try_get("", "applied_at").unwrap_or(0);
        out.push((version, name, applied_at.max(0) as u64));
    }
    Ok(out)
}

async fn execute_sql_file(conn: &DatabaseConnection, path: &Path) -> Result<(), OrmError> {
    let sql = fs::read_to_string(path).map_err(|e| OrmError::Backend(e.to_string()))?;
    for statement in split_statements(&sql) {
        let trimmed = statement.trim();
        if trimmed.is_empty() {
            continue;
        }
        conn.execute_unprepared(trimmed).await.map_err(|e| {
            let disp = path.display();
            OrmError::Backend(format!("while running {disp}: {e}"))
        })?;
    }
    Ok(())
}

fn split_statements(sql: &str) -> Vec<String> {
    // Split at `;` only when outside single-quote, double-quote, or
    // line-comment blocks.
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = sql.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;
    while let Some(ch) = chars.next() {
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '-' if !in_single && !in_double && chars.peek() == Some(&'-') => {
                // Line comment: consume until newline (don't split inside it
                // either).
                cur.push(ch);
                cur.push(chars.next().unwrap());
                while let Some(&nc) = chars.peek() {
                    if nc == '\n' {
                        break;
                    }
                    cur.push(chars.next().unwrap());
                }
                continue;
            }
            ';' if !in_single && !in_double => {
                cur.push(';');
                out.push(std::mem::take(&mut cur));
                continue;
            }
            _ => {}
        }
        cur.push(ch);
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

async fn record_applied(
    conn: &DatabaseConnection,
    version: u64,
    name: &str,
) -> Result<(), OrmError> {
    let at = now_unix_secs();
    let sql = format!(
        "INSERT INTO __ferrite_migrations (version, name, applied_at) VALUES ('{version}', '{}', {at})",
        escape_literal(name)
    );
    conn.execute_unprepared(&sql)
        .await
        .map(|_| ())
        .map_err(|e| OrmError::Backend(e.to_string()))
}

async fn unrecord_applied(conn: &DatabaseConnection, version: u64) -> Result<(), OrmError> {
    let sql = format!("DELETE FROM __ferrite_migrations WHERE version = '{version}'");
    conn.execute_unprepared(&sql)
        .await
        .map(|_| ())
        .map_err(|e| OrmError::Backend(e.to_string()))
}

fn escape_literal(s: &str) -> String {
    s.replace('\'', "''")
}

/// Open a database connection using `DATABASE_URL` from the config/env.
pub async fn connect(url: Option<&str>) -> Result<DatabaseConnection, OrmError> {
    let resolved = match url {
        Some(u) if !u.is_empty() => u.to_string(),
        _ => ConfigService::load().get_or("DATABASE_URL", "sqlite::memory:"),
    };
    Database::connect(&resolved)
        .await
        .map_err(|e| OrmError::Backend(e.to_string()))
}

/// Run every pending migration in ascending order. Prints a progress line to
/// stdout for each migration it applies.
pub async fn migrate(db_url: Option<&str>, migrations_dir: &Path) -> Result<(), OrmError> {
    let conn = connect(db_url).await?;
    let discovered = discover(migrations_dir)?;
    let applied: std::collections::HashSet<u64> = applied_rows(&conn)
        .await?
        .into_iter()
        .filter_map(|(v, _, _)| v.parse::<u64>().ok())
        .collect();

    if discovered.is_empty() {
        println!(
            "  \x1b[33mnote\x1b[0m no migrations found in `{}`",
            migrations_dir.display()
        );
        return Ok(());
    }

    let mut applied_count = 0usize;
    for m in &discovered {
        if applied.contains(&m.version) {
            continue;
        }
        let up = m.dir.join("up.sql");
        println!("  \x1b[34m↑ running\x1b[0m {}_{} …", m.version, m.name);
        execute_sql_file(&conn, &up).await?;
        record_applied(&conn, m.version, &m.name).await?;
        applied_count += 1;
    }

    match applied_count {
        0 => println!("  \x1b[32mok\x1b[0m database up to date"),
        n => println!("  \x1b[32mok\x1b[0m applied {n} migration(s)"),
    }
    Ok(())
}

/// Roll back the most recently applied migration using its `down.sql`.
pub async fn revert(db_url: Option<&str>, migrations_dir: &Path) -> Result<(), OrmError> {
    let conn = connect(db_url).await?;
    let discovered: std::collections::HashMap<u64, MigrationFile> = discover(migrations_dir)?
        .into_iter()
        .map(|m| (m.version, m))
        .collect();

    let rows = applied_rows(&conn).await?;
    let Some((ver_str, name, _)) = rows.last() else {
        println!("  \x1b[33mnote\x1b[0m nothing to revert");
        return Ok(());
    };
    let version: u64 = ver_str
        .parse()
        .map_err(|_| OrmError::Backend(format!("invalid version stored: {ver_str}")))?;
    let Some(migration) = discovered.get(&version) else {
        return Err(OrmError::Backend(format!(
            "migration {version}_{name} recorded but directory missing"
        )));
    };
    let down = migration.dir.join("down.sql");
    println!("  \x1b[35m↓ reverting\x1b[0m {version}_{name} …");
    execute_sql_file(&conn, &down).await?;
    unrecord_applied(&conn, version).await?;
    println!("  \x1b[32mok\x1b[0m reverted 1 migration");
    Ok(())
}

/// Execute every `.sql` file inside `seeds_dir` in alphabetical order.
/// Seed files are not tracked in the migrations meta table — they are
/// re-runnable by convention (use `INSERT OR IGNORE` / `ON CONFLICT DO
/// NOTHING` / `DELETE` + fresh insert inside each seed).
pub async fn seed(db_url: Option<&str>, seeds_dir: &Path) -> Result<(), OrmError> {
    let conn = connect(db_url).await?;
    if !seeds_dir.exists() {
        println!(
            "  \x1b[33mnote\x1b[0m seeds dir `{}` missing — nothing to do",
            seeds_dir.display()
        );
        return Ok(());
    }
    let mut files: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(seeds_dir).map_err(|e| OrmError::Backend(e.to_string()))? {
        let entry = entry.map_err(|e| OrmError::Backend(e.to_string()))?;
        let path = entry.path();
        if path.is_file() {
            let is_sql = path
                .extension()
                .map(|e| e.eq_ignore_ascii_case("sql"))
                .unwrap_or(false);
            if is_sql {
                files.push(path);
            }
        }
    }
    files.sort();
    if files.is_empty() {
        println!(
            "  \x1b[33mnote\x1b[0m no *.sql seeds in `{}`",
            seeds_dir.display()
        );
        return Ok(());
    }
    let n = files.len();
    for f in &files {
        let disp = f.display();
        println!("  \x1b[36m» seed\x1b[0m {disp} …");
        execute_sql_file(&conn, f).await?;
    }
    println!("  \x1b[32mok\x1b[0m ran {n} seed file(s)");
    Ok(())
}

/// Compute a [`StatusReport`] describing applied vs pending migrations.
pub async fn status(db_url: Option<&str>, migrations_dir: &Path) -> Result<StatusReport, OrmError> {
    let conn = connect(db_url).await?;
    let discovered = discover(migrations_dir)?;
    let rows = applied_rows(&conn).await?;

    let applied_set: std::collections::HashSet<u64> = rows
        .iter()
        .filter_map(|(v, _, _)| v.parse::<u64>().ok())
        .collect();

    let applied: Vec<(u64, String, u64)> = rows
        .into_iter()
        .filter_map(|(v, n, t)| v.parse::<u64>().ok().map(|ver| (ver, n, t)))
        .collect();
    let pending: Vec<(u64, String)> = discovered
        .into_iter()
        .filter(|m| !applied_set.contains(&m.version))
        .map(|m| (m.version, m.name))
        .collect();
    Ok(StatusReport { applied, pending })
}

/// Convenience: inspect the `FERRITE_DB_ACTION` env variable and dispatch to
/// [`migrate`] / [`revert`] / [`seed`] / [`status`] accordingly. Returns
/// `Ok(true)` when an action was dispatched (caller should exit the process),
/// `Ok(false)` when no action env was set (caller continues normal app
/// startup). Defaults used:
///
/// * migrations dir: `./migrations`
/// * seeds dir: `./seeds`
pub async fn run_from_env() -> Result<bool, OrmError> {
    let Some(action) = std::env::var_os("FERRITE_DB_ACTION") else {
        return Ok(false);
    };
    let action = action.to_string_lossy().to_ascii_lowercase();
    let cwd = std::env::current_dir().map_err(|e| OrmError::Backend(e.to_string()))?;
    let migrations_dir = cwd.join("migrations");
    let seeds_dir = cwd.join("seeds");
    let url = std::env::var("DATABASE_URL").ok();
    let url = url.as_deref();
    match action.as_str() {
        "migrate" => migrate(url, &migrations_dir).await.map(|_| true),
        "revert" => revert(url, &migrations_dir).await.map(|_| true),
        "seed" => seed(url, &seeds_dir).await.map(|_| true),
        "status" => {
            let report = status(url, &migrations_dir).await?;
            println!("\n  \x1b[1mApplied migrations\x1b[0m");
            if report.applied.is_empty() {
                println!("    (none)");
            } else {
                for (v, n, t) in &report.applied {
                    use std::time::{Duration, UNIX_EPOCH};
                    let stamp = UNIX_EPOCH + Duration::from_secs(*t);
                    let disp: String = chrono_like(stamp);
                    println!("    \x1b[32m✓\x1b[0m {v}_{n}  \x1b[90m[{disp}]\x1b[0m");
                }
            }
            println!("\n  \x1b[1mPending migrations\x1b[0m");
            if report.pending.is_empty() {
                println!("    (none)");
            } else {
                for (v, n) in &report.pending {
                    println!("    \x1b[33m•\x1b[0m {v}_{n}");
                }
            }
            println!();
            Ok(true)
        }
        other => Err(OrmError::Backend(format!(
            "unknown FERRITE_DB_ACTION={other:?}; expected migrate|revert|seed|status"
        ))),
    }
}

// Minimal `chrono`-free datetime formatter — we just need "YYYY-MM-DD HH:MM"
// style for status output. Uses UTC only to avoid pulling in extra deps.
fn chrono_like<T: Into<std::time::SystemTime>>(t: T) -> String {
    let dur: std::time::Duration = match t.into().duration_since(SystemTime::UNIX_EPOCH) {
        Ok(d) => d,
        Err(_) => return "unknown".into(),
    };
    let secs = dur.as_secs() as i64;
    const DAYS_PER_400Y: i64 = 146097;
    const DAYS_PER_100Y: i64 = 36524;
    const DAYS_PER_4Y: i64 = 1461;
    let mut days = secs / 86400;
    let rem = secs % 86400;
    let mut year: i64 = 1970;
    let qc = days / DAYS_PER_400Y;
    days -= qc * DAYS_PER_400Y;
    year += qc * 400;
    let c = (days / DAYS_PER_100Y).min(3);
    days -= c * DAYS_PER_100Y;
    year += c * 100;
    let q = (days / DAYS_PER_4Y).min(25);
    days -= q * DAYS_PER_4Y;
    year += q * 4;
    let y = (days / 365).min(3);
    days -= y * 365;
    year += y;
    let mut md = days as u32 + 1;
    let leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
    let mdays: [u32; 12] = [31, 28 + leap as u32, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 0;
    for (i, d) in mdays.iter().enumerate() {
        if md <= *d {
            month = i + 1;
            break;
        }
        md -= d;
    }
    let hour = rem / 3600;
    let minute = (rem % 3600) / 60;
    format!("{year:04}-{month:02}-{md:02} {hour:02}:{minute:02} UTC")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_statements_respects_quotes_and_comments() {
        let sql = "CREATE TABLE t (id INT, name VARCHAR(10));\n-- this is; a comment\nINSERT INTO t VALUES (1, 'a;b');";
        let stmts = split_statements(sql);
        assert_eq!(stmts.len(), 2, "got {stmts:?}");
        assert!(stmts[0].starts_with("CREATE TABLE"));
        assert!(stmts[1].contains("VALUES (1, 'a;b')"));
    }
}
