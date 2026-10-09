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

/// How many refused attempts a person gets in [`ATTEMPT_WINDOW_MS`] before
/// they're told to wait.
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
    if refused_lately(state, &conference.space, did).await? >= MAX_REFUSED {
        return Ok(Joined::SlowDown);
    }
    let mut tx = decide::begin(state).await?;
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
    tx.commit().await.map_err(|e| e.to_string())?;
    match outcome {
        Outcome::Decided(_) => {
            decide::committed();
            Ok(Joined::Joined)
        }
        Outcome::Unchanged => Ok(Joined::Joined),
        Outcome::Refused(_) => {
            sqlx::query("INSERT INTO join_attempts (conference, did, at) VALUES ($1, $2, $3)")
                .bind(&conference.space)
                .bind(did)
                .bind(now_ms())
                .execute(&state.db)
                .await
                .map_err(|e| e.to_string())?;
            Ok(Joined::Refused)
        }
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

/// How many times a person was refused lately.
async fn refused_lately(state: &AppState, conference: &str, did: &str) -> Result<i64, String> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM join_attempts WHERE conference = $1 AND did = $2 AND at > $3",
    )
    .bind(conference)
    .bind(did)
    .bind(now_ms() - ATTEMPT_WINDOW_MS)
    .fetch_one(&state.db)
    .await
    .map_err(|e| e.to_string())
}
