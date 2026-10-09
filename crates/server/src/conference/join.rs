//! Joining a conference, by the methods its organizer turned on: a shared
//! invite code, the attendee list (matched by the DID each handle named at
//! import), or open joining. Each ends in a decision, at the joiner's own
//! rank, so the precedence table decides whether they may come back after a
//! removal, and a ban refuses every method.

use sqlx::{Any, Transaction};

use super::Conference;
use super::decide::{self, Action, Actor, Outcome, Role};
use crate::AppState;
use crate::db::now_ms;
use crate::keys::sha256_b64;

/// How many refused attempts with a code a person gets in
/// [`ATTEMPT_WINDOW_MS`] before they're told to wait.
pub const MAX_REFUSED: i64 = 5;
/// The window refused attempts are counted in.
pub const ATTEMPT_WINDOW_MS: i64 = 10 * 60 * 1000;

/// How a join went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Joined {
    Joined,
    Refused,
    /// Too many refused attempts: wait before trying again.
    SlowDown,
}

/// A code as kept: its hash, for this conference.
pub fn code_hash(conference: &str, code: &str) -> String {
    sha256_b64(&format!("{conference}\n{}", code.trim()))
}

/// Joins a person to a conference by whichever of its methods admits them.
pub async fn join(
    state: &AppState,
    conference: &Conference,
    did: &str,
    code: Option<&str>,
) -> Result<Joined, String> {
    let mut tx = decide::begin(state).await?;
    // Counted and recorded inside the transaction, before the code is
    // looked at, so attempts made at once can't all slip under the limit.
    // Only attempts with a code count: one without (the app checking the
    // attendee list as someone opens the page) guesses nothing.
    if refused_lately(&mut tx, &conference.space, did).await? >= MAX_REFUSED {
        tx.rollback().await.map_err(|e| e.to_string())?;
        return Ok(Joined::SlowDown);
    }
    let guessing = code.is_some_and(|c| !c.trim().is_empty());
    let at = now_ms();
    if guessing {
        sqlx::query("INSERT INTO join_attempts (conference, did, at) VALUES ($1, $2, $3)")
            .bind(&conference.space)
            .bind(did)
            .bind(at)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }
    let outcome = match method(&mut tx, conference, did, code).await? {
        Some(method) => {
            let action = Action::Admit { role: Role::Attendee, method: method.to_owned() };
            let outcome =
                decide::decide_in(&mut tx, &conference.space, did, action, &Actor::Subject).await?;
            if method == "code"
                && let (Outcome::Decided(_), Some(code)) = (&outcome, code)
            {
                sqlx::query(
                    "UPDATE codes SET uses = uses + 1 WHERE conference = $1 AND code_hash = $2",
                )
                .bind(&conference.space)
                .bind(code_hash(&conference.space, code))
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
            }
            outcome
        }
        None => {
            // No method admits them; a member already in is still in.
            let decisions = sqlx::query_as::<_, decide::Decision>(
                "SELECT seq, subject, action, role, method, actor, rank, decided_at FROM decisions \
                 WHERE conference = $1 AND subject = $2 ORDER BY seq",
            )
            .bind(&conference.space)
            .bind(did)
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
            if decide::standing(&decisions, did, None).is_member() {
                Outcome::Unchanged
            } else {
                Outcome::Refused(decide::Refusal::NotAMember)
            }
        }
    };
    // Only refusals count against the limit.
    if guessing && !matches!(outcome, Outcome::Refused(_)) {
        sqlx::query("DELETE FROM join_attempts WHERE conference = $1 AND did = $2 AND at = $3")
            .bind(&conference.space)
            .bind(did)
            .bind(at)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    match outcome {
        Outcome::Decided(_) => {
            decide::committed();
            Ok(Joined::Joined)
        }
        Outcome::Unchanged => Ok(Joined::Joined),
        Outcome::Refused(_) => Ok(Joined::Refused),
    }
}

/// The method that admits a person, if any: a valid code they entered, the
/// attendee list, or open joining, among the methods that are on.
async fn method(
    tx: &mut Transaction<'static, Any>,
    conference: &Conference,
    did: &str,
    code: Option<&str>,
) -> Result<Option<&'static str>, String> {
    if conference.has_method("code")
        && let Some(code) = code.filter(|c| !c.trim().is_empty())
    {
        let found = sqlx::query_as::<_, (Option<i64>, Option<i64>, i64)>(
            "SELECT expires_at, max_uses, uses FROM codes WHERE conference = $1 AND code_hash = $2",
        )
        .bind(&conference.space)
        .bind(code_hash(&conference.space, code))
        .fetch_optional(&mut **tx)
        .await
        .map_err(|e| e.to_string())?;
        if let Some((expires_at, max_uses, uses)) = found {
            let live = expires_at.is_none_or(|at| at > now_ms());
            let left = max_uses.is_none_or(|max| uses < max);
            if live && left {
                return Ok(Some("code"));
            }
        }
    }
    if conference.has_method("list") {
        let listed = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM attendee_list WHERE conference = $1 AND did = $2",
        )
        .bind(&conference.space)
        .bind(did)
        .fetch_one(&mut **tx)
        .await
        .map_err(|e| e.to_string())?;
        if listed > 0 {
            return Ok(Some("list"));
        }
    }
    if conference.has_method("open") {
        return Ok(Some("open"));
    }
    Ok(None)
}

/// How many times a person was refused lately, inside the join's
/// transaction. Attempts older than the window no longer count, and are
/// dropped.
async fn refused_lately(
    tx: &mut Transaction<'static, Any>,
    conference: &str,
    did: &str,
) -> Result<i64, String> {
    let since = now_ms() - ATTEMPT_WINDOW_MS;
    sqlx::query("DELETE FROM join_attempts WHERE at <= $1")
        .bind(since)
        .execute(&mut **tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM join_attempts WHERE conference = $1 AND did = $2 AND at > $3",
    )
    .bind(conference)
    .bind(did)
    .bind(since)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    /// A directory removed when the test ends, pass or fail.
    struct TempDir(std::path::PathBuf);

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    async fn state() -> (AppState, TempDir) {
        let dir = TempDir(
            std::env::temp_dir().join(format!("eventside-join-{}", crate::keys::random_token(8))),
        );
        let local = "http://127.0.0.1:1".to_owned();
        let config = Config {
            port: 0,
            atproto_url: local.clone(),
            public_url: None,
            database_url: format!("sqlite://{}/eventside.db?mode=rwc", dir.0.display()),
            signing_key: None,
            scopes: vec!["atproto".into()],
            signup_pds_url: local.clone(),
            plc_url: local.clone(),
            handle_resolver_url: local,
            allow_private_network: true,
            session_idle_timeout: std::time::Duration::from_secs(60),
            token_renew_interval: std::time::Duration::from_secs(60),
            token_refresh_skew: std::time::Duration::from_secs(60),
        };
        let state = AppState::build(config, "http://127.0.0.1:3100".into()).await.unwrap();
        (state, dir)
    }

    #[tokio::test]
    async fn tc_16_wrong_codes_tried_at_once_are_still_capped() {
        let (state, _dir) = state().await;
        let space = super::super::SpaceUri::conference("did:plc:aaaaaaaaaaaaaaaaaaaaaaaa", "3kx");
        sqlx::query(
            "INSERT INTO conferences (space, org, rkey, event, name, starts_at, ends_at, city, theme, \
             methods, created_by, created_at) VALUES ($1, 'did:plc:aaaaaaaaaaaaaaaaaaaaaaaa', '3kx', \
             'at://x/y/3kx', 'Conf', '2027-04-29T07:00:00.000Z', '2027-05-02T16:00:00.000Z', \
             'Amsterdam', '{}', 'code', 'did:plc:aaaaaaaaaaaaaaaaaaaaaaaa', 0)",
        )
        .bind(&space)
        .execute(&state.db)
        .await
        .unwrap();
        let conference = super::super::load(&state.db, &space).await.unwrap().unwrap();
        let mallory = "did:plc:mmmmmmmmmmmmmmmmmmmmmmmm";
        let tries = (0..40).map(|i| {
            let (state, conference) = (state.clone(), conference.clone());
            tokio::spawn(async move {
                join(&state, &conference, mallory, Some(&format!("wrong-{i}"))).await.unwrap()
            })
        });
        let mut refused = 0;
        let mut slowed = 0;
        for answer in tries.collect::<Vec<_>>() {
            match answer.await.unwrap() {
                Joined::Refused => refused += 1,
                Joined::SlowDown => slowed += 1,
                Joined::Joined => panic!("a wrong code admitted someone"),
            }
        }
        assert_eq!(refused, MAX_REFUSED);
        assert_eq!(slowed, 40 - MAX_REFUSED);
    }
}
