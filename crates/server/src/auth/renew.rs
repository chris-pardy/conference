//! Token renewal, in the background and on demand.
//!
//! Refresh tokens are single-use: a second refresh with the same token is
//! refused with `invalid_grant`, which would wrongly end the session. So every
//! refresh first takes a short lease on the session's row, and only the
//! caller holding it talks to the authorization server. That holds across
//! instances sharing one database.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use super::session;
use crate::AppState;
use crate::db::{ms, now_ms};
use crate::keys::EcKey;
use crate::oauth::{AuthServer, OAuthError, TokenSet};

/// How long a refresh may hold its lease. A refresh is cut off well before:
/// if a lease lapsed mid-refresh, another instance could spend the same
/// single-use token, and the refusal would end a healthy session.
const LEASE_MS: i64 = 90_000;
const REFRESH_TIMEOUT: Duration = Duration::from_secs(60);
/// How many refreshes run at once.
const CONCURRENCY: usize = 8;

#[derive(Debug)]
pub enum Renewal {
    Renewed,
    /// Someone else holds the lease; their result will be in the row.
    Busy,
    /// The authorization server refused: the session has ended.
    Ended,
    /// It couldn't be reached, or the session has no tokens to refresh. The
    /// session is unchanged.
    Failed(String),
}

/// Refreshes one session's tokens, if no one else is.
pub async fn refresh(state: &AppState, id_hash: &str) -> Renewal {
    let now = now_ms();
    let leased = sqlx::query(
        "UPDATE sessions SET refresh_lease_until = $1 WHERE id_hash = $2 AND ended_at IS NULL \
         AND refresh_token IS NOT NULL AND (refresh_lease_until IS NULL OR refresh_lease_until < $3)",
    )
    .bind(now + LEASE_MS)
    .bind(id_hash)
    .bind(now)
    .execute(&state.db)
    .await;
    match leased {
        Ok(done) if done.rows_affected() == 1 => {}
        Ok(_) => return Renewal::Busy,
        Err(err) => return Renewal::Failed(err.to_string()),
    }
    let outcome = tokio::time::timeout(REFRESH_TIMEOUT, refresh_leased(state, id_hash))
        .await
        .unwrap_or_else(|_| Renewal::Failed("the refresh timed out".into()));
    if !matches!(outcome, Renewal::Renewed | Renewal::Ended) {
        release(state, id_hash).await;
    }
    outcome
}

async fn refresh_leased(state: &AppState, id_hash: &str) -> Renewal {
    let row = match session::load(&state.db, id_hash).await {
        Ok(Some(row)) => row,
        Ok(None) => return Renewal::Failed("the session is gone".into()),
        Err(err) => return Renewal::Failed(err.to_string()),
    };
    // Granted under an older scope list (and client ID): the person signs in
    // again rather than renewing a grant the server no longer asks for.
    if session::missing_scope(state, &row.scopes) {
        return match session::end(state, &row).await {
            Ok(()) => Renewal::Ended,
            Err(err) => Renewal::Failed(err.to_string()),
        };
    }
    let (Some(issuer), Some(old), Some(key)) = (&row.issuer, &row.refresh_token, &row.dpop_key)
    else {
        return Renewal::Failed("the session has no tokens".into());
    };
    let Ok(key) = EcKey::from_jwk(key) else {
        return Renewal::Failed("the session's DPoP key is unreadable".into());
    };
    let server = match state.oauth.cached_auth_server(issuer).await {
        Ok(server) => server,
        Err(why) => return Renewal::Failed(why),
    };
    match state.oauth.refresh(&server, old, &key).await {
        // Tokens for someone else are a refused grant, not a renewal.
        Ok(tokens) if tokens.sub.as_deref() != Some(row.did.as_str()) => {
            eprintln!("renewal: a refresh for {} came back for {:?}", row.did, tokens.sub);
            revoke_unsaved(state, &server, &tokens, &key).await;
            match session::wipe(&state.db, id_hash).await {
                Ok(()) => Renewal::Ended,
                Err(err) => Renewal::Failed(err.to_string()),
            }
        }
        Ok(tokens) => {
            let scopes = tokens.scope.clone().unwrap_or_else(|| row.scopes.clone());
            let saved = sqlx::query(
                "UPDATE sessions SET access_token = $1, refresh_token = $2, token_expires_at = $3, scopes = $4, \
                 refresh_lease_until = NULL WHERE id_hash = $5 AND refresh_token = $6",
            )
            .bind(&tokens.access_token)
            .bind(tokens.refresh_token.as_deref().unwrap_or(old))
            .bind(now_ms() + tokens.expires_in.unwrap_or(300) * 1000)
            .bind(scopes)
            .bind(id_hash)
            .bind(old)
            .execute(&state.db)
            .await;
            match saved {
                Ok(done) if done.rows_affected() == 1 => Renewal::Renewed,
                // Signed out or ended mid-refresh: nothing holds the new grant, so revoke it.
                Ok(_) => {
                    revoke_unsaved(state, &server, &tokens, &key).await;
                    Renewal::Ended
                }
                Err(err) => {
                    revoke_unsaved(state, &server, &tokens, &key).await;
                    Renewal::Failed(format!("could not save renewed tokens: {err}"))
                }
            }
        }
        Err(OAuthError::InvalidGrant(why)) => {
            // Only if the row still holds the token that was refused.
            let ended = sqlx::query(
                "UPDATE sessions SET ended_at = $1, access_token = NULL, refresh_token = NULL, dpop_key = NULL, \
                 refresh_lease_until = NULL WHERE id_hash = $2 AND refresh_token = $3 AND ended_at IS NULL",
            )
            .bind(now_ms())
            .bind(id_hash)
            .bind(old)
            .execute(&state.db)
            .await;
            eprintln!("renewal: {}'s grant was refused, ending the session: {why}", row.did);
            match ended {
                Ok(_) => Renewal::Ended,
                Err(err) => Renewal::Failed(err.to_string()),
            }
        }
        Err(err) => {
            if matches!(err, OAuthError::Unavailable(_)) {
                state.oauth.mark_down(issuer);
            }
            Renewal::Failed(err.to_string())
        }
    }
}

/// Revokes a refreshed grant that no session will hold.
async fn revoke_unsaved(state: &AppState, server: &AuthServer, tokens: &TokenSet, key: &EcKey) {
    if let Some(refresh) = &tokens.refresh_token
        && let Err(err) = state.oauth.revoke(server, refresh, key).await
    {
        eprintln!("renewal: could not revoke an unsaved grant: {err}");
    }
}

async fn release(state: &AppState, id_hash: &str) {
    let released = sqlx::query("UPDATE sessions SET refresh_lease_until = NULL WHERE id_hash = $1")
        .bind(id_hash)
        .execute(&state.db)
        .await;
    if let Err(err) = released {
        eprintln!("renewal: could not release a lease: {err}");
    }
}

/// One renewal run: ends idle sessions, and refreshes every live session
/// whose access token expires before the next run.
pub async fn run_once(state: &AppState) {
    let now = now_ms();
    let config = &state.config;
    let due = now + ms(config.token_renew_interval) + ms(config.token_refresh_skew);
    let idle_before = now - ms(config.session_idle_timeout);
    let rows = sqlx::query_as::<_, (String, i64, Option<String>)>(
        "SELECT id_hash, last_seen_at, issuer FROM sessions WHERE ended_at IS NULL \
         AND (last_seen_at < $1 OR (refresh_token IS NOT NULL AND token_expires_at <= $2))",
    )
    .bind(idle_before)
    .bind(due)
    .fetch_all(&state.db)
    .await;
    let rows = match rows {
        Ok(rows) => rows,
        Err(err) => {
            eprintln!("renewal: could not list sessions: {err}");
            return;
        }
    };
    let permits = Arc::new(Semaphore::new(CONCURRENCY));
    let mut tasks = JoinSet::new();
    for (id_hash, last_seen, issuer) in rows {
        // Leave an unreachable issuer alone for a while, rather than timing
        // out once per session on it every run.
        let backing_off = issuer.is_some_and(|i| state.oauth.backing_off(&i));
        if backing_off && last_seen >= idle_before {
            continue;
        }
        let state = state.clone();
        let permits = permits.clone();
        tasks.spawn(async move {
            let Ok(_permit) = permits.acquire().await else { return };
            if last_seen < idle_before {
                let ended = match session::load(&state.db, &id_hash).await {
                    Ok(Some(row)) => session::end(&state, &row).await,
                    Ok(None) => Ok(()),
                    Err(err) => Err(err),
                };
                if let Err(err) = ended {
                    eprintln!("renewal: could not end an idle session: {err}");
                }
                return;
            }
            if let Renewal::Failed(why) = refresh(&state, &id_hash).await {
                eprintln!("renewal: {why}");
            }
        });
    }
    while tasks.join_next().await.is_some() {}
}

/// Runs renewal and the sweeper for as long as the server does.
pub fn spawn(state: AppState) {
    tokio::spawn(async move {
        let mut ticks = tokio::time::interval(state.config.token_renew_interval);
        ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticks.tick().await;
            run_once(&state).await;
            if let Err(err) = session::sweep(&state).await {
                eprintln!("sweeper: {err}");
            }
        }
    });
}
