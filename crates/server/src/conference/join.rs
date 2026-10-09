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
///
/// `on_open` is the app joining someone as they open the conference's page
/// (for the attendee list): it only ever admits someone the conference has
/// never decided anything about. Someone who left, or was removed, rejoins
/// only by asking.
pub async fn join(
    state: &AppState,
    conference: &Conference,
    did: &str,
    code: Option<&str>,
    on_open: bool,
) -> Result<Joined, String> {
    let mut tx = decide::begin(state).await?;
    if on_open {
        let decided = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM decisions WHERE conference = $1 AND subject = $2",
        )
        .bind(&conference.space)
        .bind(did)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        if decided > 0 {
            tx.rollback().await.map_err(|e| e.to_string())?;
            return Ok(Joined::Refused);
        }
    }
    // Only attempts with a code are limited: one without guesses nothing.
    // They're counted and recorded inside the transaction, before the code
    // is looked at, so attempts made at once can't all slip under the limit.
    let guessing = code.is_some_and(|c| !c.trim().is_empty());
    if guessing && refused_lately(&mut tx, &conference.space, did).await? >= MAX_REFUSED {
        tx.rollback().await.map_err(|e| e.to_string())?;
        return Ok(Joined::SlowDown);
    }
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
    let outcome = match method(&mut tx, conference, did, code, on_open).await? {
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
/// attendee list, or open joining, among the methods that are on. When the
/// app joins someone as they open the page, only the list.
async fn method(
    tx: &mut Transaction<'static, Any>,
    conference: &Conference,
    did: &str,
    code: Option<&str>,
    on_open: bool,
) -> Result<Option<&'static str>, String> {
    // Opening the page only checks the attendee list: an open conference
    // still takes someone asking to join.
    if !on_open
        && conference.has_method("code")
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
    if conference.has_method("open") && !on_open {
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
    use crate::conference::test_support::{conference, state};

    const ANA: &str = "did:plc:nnnnnnnnnnnnnnnnnnnnnnnn";

    async fn listed(state: &AppState, conference: &Conference, did: &str) {
        sqlx::query("INSERT INTO attendee_list (conference, did, handle, imported_at) VALUES ($1, $2, 'ana.test', 0)")
            .bind(&conference.space)
            .bind(did)
            .execute(&state.db)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn opening_the_page_joins_a_listed_attendee_only_the_first_time() {
        let (state, _dir) = state().await;
        let conference = conference(&state, "list").await;
        listed(&state, &conference, ANA).await;
        assert_eq!(join(&state, &conference, ANA, None, true).await.unwrap(), Joined::Joined);
        let left = decide::decide(&state, &conference.space, ANA, Action::Leave, &Actor::Subject)
            .await
            .unwrap();
        assert!(matches!(left, Outcome::Decided(_)));
        // Opening the page again doesn't bring her back…
        assert_eq!(join(&state, &conference, ANA, None, true).await.unwrap(), Joined::Refused);
        assert!(!decide::is_member_now(&state.db, &conference.space, ANA).await.unwrap());
        // …asking does.
        assert_eq!(join(&state, &conference, ANA, None, false).await.unwrap(), Joined::Joined);
    }

    #[tokio::test]
    async fn opening_an_open_conference_doesnt_join_anyone_off_the_list() {
        let (state, _dir) = state().await;
        let conference = conference(&state, "list,open").await;
        assert_eq!(join(&state, &conference, ANA, None, true).await.unwrap(), Joined::Refused);
        assert!(!decide::is_member_now(&state.db, &conference.space, ANA).await.unwrap());
        // Asking still joins her.
        assert_eq!(join(&state, &conference, ANA, None, false).await.unwrap(), Joined::Joined);
    }

    #[tokio::test]
    async fn a_join_without_a_code_isnt_slowed_by_wrong_codes() {
        let (state, _dir) = state().await;
        let conference = conference(&state, "code").await;
        for i in 0..MAX_REFUSED {
            let answer =
                join(&state, &conference, ANA, Some(&format!("wrong-{i}")), false).await.unwrap();
            assert_eq!(answer, Joined::Refused);
        }
        assert_eq!(
            join(&state, &conference, ANA, Some("wrong"), false).await.unwrap(),
            Joined::SlowDown
        );
        // Without a code it's an answer, not a wait.
        assert_eq!(join(&state, &conference, ANA, None, false).await.unwrap(), Joined::Refused);
    }

    #[tokio::test]
    async fn tc_16_wrong_codes_tried_at_once_are_still_capped() {
        let (state, _dir) = state().await;
        let conference = conference(&state, "code").await;
        let mallory = "did:plc:mmmmmmmmmmmmmmmmmmmmmmmm";
        let tries = (0..40).map(|i| {
            let (state, conference) = (state.clone(), conference.clone());
            tokio::spawn(async move {
                join(&state, &conference, mallory, Some(&format!("wrong-{i}")), false)
                    .await
                    .unwrap()
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
