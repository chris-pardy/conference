//! Sessions: the rows behind the cookie, and how they end.

use std::time::Duration;

use axum::http::HeaderMap;

use crate::AppState;
use crate::db::{Db, ms, now_ms};
use crate::keys::{random_token, sha256_b64};

/// How long an ended session keeps its DID and handle, so the PWA can sign
/// the same person back in. The cookie lives this long past the idle timeout.
pub const TOMBSTONE: Duration = Duration::from_secs(30 * 24 * 60 * 60);

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SessionRow {
    pub id_hash: String,
    pub did: String,
    pub handle: String,
    pub display_name: Option<String>,
    pub avatar: Option<String>,
    pub pds: Option<String>,
    pub dpop_key: Option<String>,
    pub issuer: Option<String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub token_expires_at: Option<i64>,
    pub scopes: String,
    pub csrf_token: String,
    pub last_seen_at: i64,
    pub ended_at: Option<i64>,
}

impl SessionRow {
    pub fn scope_list(&self) -> Vec<String> {
        self.scopes.split_whitespace().map(str::to_owned).collect()
    }
}

const COLUMNS: &str = "id_hash, did, handle, display_name, avatar, pds, dpop_key, issuer, access_token, \
    refresh_token, token_expires_at, scopes, csrf_token, last_seen_at, ended_at";

/// Where a browser's session stands.
pub enum Lookup {
    /// No cookie, or one that names no session.
    None,
    /// The session ended (idle, refused refresh, or scopes that grew).
    Expired(SessionRow),
    Live(SessionRow),
}

pub struct NewSession<'a> {
    pub did: &'a str,
    pub handle: &'a str,
    pub display_name: Option<&'a str>,
    pub avatar: Option<&'a str>,
    pub pds: &'a str,
    pub dpop_key: &'a str,
    pub issuer: &'a str,
    pub access_token: &'a str,
    pub refresh_token: Option<&'a str>,
    pub token_expires_at: i64,
    pub scopes: &'a str,
}

/// Stores a new session, returning the cookie value that names it.
pub async fn create(db: &Db, s: NewSession<'_>) -> Result<String, sqlx::Error> {
    let id = random_token(32);
    let now = now_ms();
    sqlx::query(
        "INSERT INTO sessions (id_hash, did, handle, display_name, avatar, pds, dpop_key, issuer, access_token, \
         refresh_token, token_expires_at, scopes, csrf_token, created_at, last_seen_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)",
    )
    .bind(sha256_b64(&id))
    .bind(s.did)
    .bind(s.handle)
    .bind(s.display_name)
    .bind(s.avatar)
    .bind(s.pds)
    .bind(s.dpop_key)
    .bind(s.issuer)
    .bind(s.access_token)
    .bind(s.refresh_token)
    .bind(s.token_expires_at)
    .bind(s.scopes)
    .bind(random_token(24))
    .bind(now)
    .bind(now)
    .execute(db)
    .await?;
    Ok(id)
}

pub async fn load(db: &Db, id_hash: &str) -> Result<Option<SessionRow>, sqlx::Error> {
    sqlx::query_as::<_, SessionRow>(&format!("SELECT {COLUMNS} FROM sessions WHERE id_hash = $1"))
        .bind(id_hash)
        .fetch_optional(db)
        .await
}

/// The browser's session, ending it first if it has gone idle or no longer
/// carries every sign-in scope.
pub async fn lookup(state: &AppState, headers: &HeaderMap) -> Result<Lookup, sqlx::Error> {
    let Some(cookie) = super::cookies::session(headers, state.secure_cookies()) else {
        return Ok(Lookup::None);
    };
    let Some(row) = load(&state.db, &sha256_b64(&cookie)).await? else {
        return Ok(Lookup::None);
    };
    if row.ended_at.is_some() {
        return Ok(Lookup::Expired(row));
    }
    let now = now_ms();
    let idle = now - row.last_seen_at > ms(state.config.session_idle_timeout);
    let granted = row.scope_list();
    let missing_scope = state.config.scopes.iter().any(|s| !granted.contains(s));
    if idle || missing_scope {
        end(&state.db, &row.id_hash).await?;
        return Ok(Lookup::Expired(row));
    }
    Ok(Lookup::Live(row))
}

/// Records use of a session, at most every so often. Returns whether it did,
/// so the caller can renew the cookie too.
pub async fn touch(state: &AppState, row: &SessionRow) -> Result<bool, sqlx::Error> {
    let every = (ms(state.config.session_idle_timeout) / 10).clamp(100, 5 * 60 * 1000);
    let now = now_ms();
    if now - row.last_seen_at < every {
        return Ok(false);
    }
    sqlx::query("UPDATE sessions SET last_seen_at = $1 WHERE id_hash = $2 AND ended_at IS NULL")
        .bind(now)
        .bind(&row.id_hash)
        .execute(&state.db)
        .await?;
    Ok(true)
}

/// Ends a session: its tokens and keys are wiped, but the row keeps who it
/// was until the sweeper removes it.
pub async fn end(db: &Db, id_hash: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE sessions SET ended_at = $1, access_token = NULL, refresh_token = NULL, dpop_key = NULL, \
         refresh_lease_until = NULL WHERE id_hash = $2 AND ended_at IS NULL",
    )
    .bind(now_ms())
    .bind(id_hash)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete(db: &Db, id_hash: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM sessions WHERE id_hash = $1").bind(id_hash).execute(db).await?;
    Ok(())
}

/// Removes stale pending requests, and ended or long-idle sessions past
/// their tombstone.
pub async fn sweep(state: &AppState) -> Result<(), sqlx::Error> {
    let now = now_ms();
    sqlx::query("DELETE FROM oauth_requests WHERE expires_at < $1")
        .bind(now)
        .execute(&state.db)
        .await?;
    let cutoff = now - ms(TOMBSTONE);
    sqlx::query("DELETE FROM sessions WHERE ended_at IS NOT NULL AND ended_at < $1")
        .bind(cutoff)
        .execute(&state.db)
        .await?;
    sqlx::query("DELETE FROM sessions WHERE ended_at IS NULL AND last_seen_at < $1")
        .bind(cutoff - ms(state.config.session_idle_timeout))
        .execute(&state.db)
        .await?;
    Ok(())
}
