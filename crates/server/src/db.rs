//! The appview's database: SQLite or Postgres, chosen by the scheme of
//! `DATABASE_URL`, behind one `Any` pool. Queries use `$1` placeholders and
//! only TEXT and BIGINT columns, which both backends share.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sqlx::any::{AnyPoolOptions, install_default_drivers};

pub type Db = sqlx::AnyPool;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Sqlite,
    Postgres,
}

impl Backend {
    /// The backend a `DATABASE_URL` asks for.
    pub fn of(url: &str) -> Result<Self, String> {
        let scheme = url.split_once(':').map_or("", |(scheme, _)| scheme);
        match scheme {
            "sqlite" => Ok(Self::Sqlite),
            "postgres" | "postgresql" => Ok(Self::Postgres),
            _ => Err(format!(
                "DATABASE_URL has an unsupported scheme {scheme:?}: use sqlite://<path> or postgres://<host>/<database>"
            )),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Sqlite => "SQLite",
            Self::Postgres => "Postgres",
        }
    }
}

/// Connects to the database and brings its schema up to date.
pub async fn connect(url: &str) -> Result<Db, String> {
    let backend = Backend::of(url)?;
    if backend == Backend::Sqlite {
        ensure_sqlite_dir(url)?;
    }
    install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(8)
        .acquire_timeout(Duration::from_secs(10))
        .connect(url)
        .await
        .map_err(|e| format!("could not connect to the {} database: {e}", backend.name()))?;
    if backend == Backend::Sqlite {
        // Readers (and other processes reading the file) never wait on the
        // renewer's writes. The setting is kept in the file.
        sqlx::query("PRAGMA journal_mode = WAL")
            .fetch_optional(&pool)
            .await
            .map_err(|e| format!("could not set up the SQLite database: {e}"))?;
    }
    MIGRATOR
        .run(&pool)
        .await
        .map_err(|e| format!("could not migrate the {} database: {e}", backend.name()))?;
    Ok(pool)
}

/// SQLite creates the file but not its directory.
fn ensure_sqlite_dir(url: &str) -> Result<(), String> {
    let path = url.trim_start_matches("sqlite:").trim_start_matches("//");
    let path = path.split('?').next().unwrap_or_default();
    if path.is_empty() || path == ":memory:" {
        return Ok(());
    }
    match std::path::Path::new(path).parent() {
        Some(dir) if !dir.as_os_str().is_empty() => std::fs::create_dir_all(dir)
            .map_err(|e| format!("could not create the database directory {}: {e}", dir.display())),
        _ => Ok(()),
    }
}

/// Milliseconds since the Unix epoch, the unit every timestamp column uses.
pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

pub fn ms(duration: Duration) -> i64 {
    duration.as_millis().min(i64::MAX as u128) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scheme_picks_the_backend() {
        assert_eq!(Backend::of("sqlite://data/eventside.db?mode=rwc"), Ok(Backend::Sqlite));
        assert_eq!(Backend::of("postgres://u@host/db"), Ok(Backend::Postgres));
        assert_eq!(Backend::of("postgresql://u@host/db"), Ok(Backend::Postgres));
        let err = Backend::of("mysql://u@host/db").unwrap_err();
        assert!(err.contains("sqlite") && err.contains("postgres"), "{err}");
    }
}
