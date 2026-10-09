//! Sessions: the rows behind the cookie, and how they end.

use std::time::Duration;

use axum::http::HeaderMap;

use crate::AppState;
use crate::db::{Db, ms, now_ms};
use crate::keys::{EcKey, random_token, sha256_b64};

/// How long an ended session keeps its DID and handle, so the PWA can sign
/// the same person back in. The cookie lives this long past the idle timeout,
/// counted from its last renewal; the sweeper keeps a row that long after it
/// ended, which is never sooner.
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
    /// The client ID the grant was issued to, which refreshes and revokes it.
    pub client_id: String,
    pub csrf_token: String,
    pub last_seen_at: i64,
    pub ended_at: Option<i64>,
    /// `attendee` (behind a cookie), or `admin` or `org` (cookieless, made
    /// by the admin CLI's `connect` and `org connect`).
    pub kind: String,
}

/// A browser's session, behind a cookie.
pub const ATTENDEE: &str = "attendee";
/// An admin's session: proves who runs an admin command `--as` them.
pub const ADMIN: &str = "admin";
/// An organization's account, which eventside writes its conference records as.
pub const ORG: &str = "org";

impl SessionRow {
    /// An admin's or an organization's session: never behind a cookie, and
    /// never ended for being idle.
    pub fn is_cookieless(&self) -> bool {
        self.kind != ATTENDEE
    }

    pub fn scope_list(&self) -> Vec<String> {
        self.scopes.split_whitespace().map(str::to_owned).collect()
    }
}

const COLUMNS: &str = "id_hash, did, handle, display_name, avatar, pds, dpop_key, issuer, access_token, \
    refresh_token, token_expires_at, scopes, client_id, csrf_token, last_seen_at, ended_at, kind";

/// Where a browser's session stands.
pub enum Lookup {
    /// No cookie, or one that names no session.
    None,
    /// The session ended (idle, refused refresh, or a scope it lacks).
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
    pub client_id: &'a str,
    /// `ATTENDEE`, `ADMIN` or `ORG`.
    pub kind: &'a str,
}

/// Stores a new session, returning the cookie value that names it.
pub async fn create(db: &Db, s: NewSession<'_>) -> Result<String, sqlx::Error> {
    let id = random_token(32);
    let now = now_ms();
    sqlx::query(
        "INSERT INTO sessions (id_hash, did, handle, display_name, avatar, pds, dpop_key, issuer, access_token, \
         refresh_token, token_expires_at, scopes, client_id, csrf_token, created_at, last_seen_at, kind) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)",
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
    .bind(s.client_id)
    .bind(random_token(24))
    .bind(now)
    .bind(now)
    .bind(s.kind)
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

/// The browser's session, ending it first if it has gone idle or its grant
/// is outdated.
pub async fn lookup(state: &AppState, headers: &HeaderMap) -> Result<Lookup, sqlx::Error> {
    let Some(cookie) = super::cookies::session(headers, state.secure_cookies()) else {
        return Ok(Lookup::None);
    };
    let Some(row) = load(&state.db, &sha256_b64(&cookie)).await? else {
        return Ok(Lookup::None);
    };
    // Admin and organization sessions never get a cookie; one that somehow
    // names one is no session.
    if row.is_cookieless() {
        return Ok(Lookup::None);
    }
    if row.ended_at.is_some() {
        return Ok(Lookup::Expired(row));
    }
    let now = now_ms();
    let idle = now - row.last_seen_at > ms(state.config.session_idle_timeout);
    if idle || outdated(state, &row) {
        end(state, &row).await?;
        return Ok(Lookup::Expired(row));
    }
    Ok(Lookup::Live(row))
}

/// Records use of a session, at most every so often. (`getSession`, which
/// the PWA calls on every load, also renews the cookie each time.)
pub async fn touch(state: &AppState, row: &SessionRow) -> Result<(), sqlx::Error> {
    // At most hourly, or more often for idle timeouts short enough to need it.
    let every = (ms(state.config.session_idle_timeout) / 10).clamp(100, 60 * 60 * 1000);
    let now = now_ms();
    if now - row.last_seen_at < every {
        return Ok(());
    }
    sqlx::query("UPDATE sessions SET last_seen_at = $1 WHERE id_hash = $2 AND ended_at IS NULL")
        .bind(now)
        .bind(&row.id_hash)
        .execute(&state.db)
        .await?;
    Ok(())
}

/// Whether a session's grant is outdated, so the person signs in again: it
/// lacks a scope this instance asks for at sign-in (or, for an admin's or an
/// organization's session, when connecting). Nothing else counts: a
/// grant with more scopes, or issued to another client ID (another scope
/// list, e.g. from an instance on another version during a rolling deploy),
/// is still good, and is refreshed and revoked as its own client ID.
pub fn outdated(state: &AppState, row: &SessionRow) -> bool {
    let granted: Vec<&str> = row.scopes.split_whitespace().collect();
    match row.kind.as_str() {
        ADMIN => crate::config::ADMIN_SCOPES.iter().any(|s| !granted.contains(s)),
        ORG => crate::config::ORG_SCOPES.iter().any(|s| !granted.contains(s)),
        _ => state.config.scopes.iter().any(|s| !granted.contains(&s.as_str())),
    }
}

/// Ends a session: its tokens and keys are wiped, but the row keeps who it
/// was until the sweeper removes it. Whoever ends it revokes its grant, the
/// one it wiped (which a renewal may have changed since `row` was read), in
/// the background; a request that finds it already ended leaves that be.
pub async fn end(state: &AppState, row: &SessionRow) -> Result<(), sqlx::Error> {
    if let Some(wiped) = wipe(&state.db, &row.id_hash).await? {
        let background = state.clone();
        tokio::spawn(async move { revoke(&background, &wiped).await });
    }
    Ok(())
}

/// How long `end_revoking` waits on the authorization server.
const REVOKE_TIMEOUT: Duration = Duration::from_secs(15);

/// Ends a session like `end`, but revokes its grant in the caller's task (for
/// at most `REVOKE_TIMEOUT`), so a caller that limits its own concurrency,
/// like the renewer, limits its revocations too. An issuer that recently
/// couldn't be reached isn't waited on: the session is only wiped. Like
/// `end`, only the call that ends the session revokes its grant.
pub async fn end_revoking(state: &AppState, row: &SessionRow) -> Result<(), sqlx::Error> {
    let Some(row) = wipe(&state.db, &row.id_hash).await? else {
        return Ok(());
    };
    match &row.issuer {
        Some(issuer) if state.oauth.backing_off(issuer) => {
            eprintln!("could not revoke {}'s grant: {issuer} is backing off", row.did);
        }
        issuer => {
            if tokio::time::timeout(REVOKE_TIMEOUT, revoke(state, &row)).await.is_err() {
                eprintln!("could not revoke {}'s grant: timed out", row.did);
                if let Some(issuer) = issuer {
                    state.oauth.mark_down(issuer);
                }
            }
        }
    }
    Ok(())
}

/// Revokes a session's grant at its authorization server, as the client ID
/// it was issued to, best effort. An unreachable server is marked down,
/// which the renewer heeds.
pub async fn revoke(state: &AppState, row: &SessionRow) {
    let (Some(issuer), Some(refresh), Some(key)) = (&row.issuer, &row.refresh_token, &row.dpop_key)
    else {
        return;
    };
    let Ok(key) = EcKey::from_jwk(key) else { return };
    let result = match state.oauth.cached_auth_server(issuer).await {
        Ok(server) => {
            state.oauth.revoke(&server, &row.client_id, refresh, &key).await.map_err(|e| {
                if matches!(e, crate::oauth::OAuthError::Unavailable(_)) {
                    state.oauth.mark_down(issuer);
                }
                e.to_string()
            })
        }
        Err(why) => Err(why.to_string()),
    };
    if let Err(why) = result {
        eprintln!("could not revoke {}'s grant: {why}", row.did);
    }
}

/// How many times `wipe` re-reads a row whose tokens a renewal changed
/// under it before wiping whatever is there.
const WIPE_ATTEMPTS: usize = 3;

/// Marks a session ended and wipes its tokens, without revoking them (for a
/// grant the authorization server has already refused). Returns the row as it
/// was just before this call wiped it, so a caller revokes the tokens that
/// were actually removed, or `None` if it was already ended (or gone).
///
/// The tokens are wiped only if they're still the ones read, so a renewal
/// that saves new tokens in between is never lost track of: the row is read
/// again and the new tokens are the ones returned.
pub async fn wipe(db: &Db, id_hash: &str) -> Result<Option<SessionRow>, sqlx::Error> {
    for attempt in 1..=WIPE_ATTEMPTS {
        let Some(row) = load(db, id_hash).await? else { return Ok(None) };
        if row.ended_at.is_some() {
            return Ok(None);
        }
        // On the last attempt, wipe whatever is there: a renewal racing that
        // often is vanishingly unlikely, and the session must still end.
        let checked = attempt < WIPE_ATTEMPTS;
        let unchanged = if checked { " AND COALESCE(refresh_token, '') = $3" } else { "" };
        let sql = format!(
            "UPDATE sessions SET ended_at = $1, access_token = NULL, refresh_token = NULL, dpop_key = NULL, \
             refresh_lease_until = NULL WHERE id_hash = $2 AND ended_at IS NULL{unchanged}"
        );
        let mut query = sqlx::query(&sql).bind(now_ms()).bind(id_hash);
        if checked {
            query = query.bind(row.refresh_token.clone().unwrap_or_default());
        }
        let done = query.execute(db).await?;
        if done.rows_affected() == 1 {
            return Ok(Some(row));
        }
    }
    Ok(None)
}

/// Deletes a session, returning the row it deleted (`None` if it was already
/// gone), so the caller revokes the tokens it actually held.
pub async fn delete(db: &Db, id_hash: &str) -> Result<Option<SessionRow>, sqlx::Error> {
    sqlx::query_as::<_, SessionRow>(&format!(
        "DELETE FROM sessions WHERE id_hash = $1 RETURNING {COLUMNS}"
    ))
    .bind(id_hash)
    .fetch_optional(db)
    .await
}

/// Removes stale pending requests, and sessions whose cookie has run out:
/// it lives the idle timeout plus the tombstone from its last renewal, which
/// is no later than `last_seen_at`, or `ended_at` for an ended session.
pub async fn sweep(state: &AppState) -> Result<(), sqlx::Error> {
    let now = now_ms();
    sqlx::query("DELETE FROM oauth_requests WHERE expires_at < $1")
        .bind(now)
        .execute(&state.db)
        .await?;
    let cutoff = now - ms(TOMBSTONE) - ms(state.config.session_idle_timeout);
    sqlx::query("DELETE FROM sessions WHERE ended_at IS NOT NULL AND ended_at < $1")
        .bind(cutoff)
        .execute(&state.db)
        .await?;
    // Admin and organization sessions don't idle out: the CLI and the
    // server use them, not a browser.
    sqlx::query(
        "DELETE FROM sessions WHERE ended_at IS NULL AND last_seen_at < $1 AND kind = 'attendee'",
    )
    .bind(cutoff)
    .execute(&state.db)
    .await?;
    Ok(())
}
