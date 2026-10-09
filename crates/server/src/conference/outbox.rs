//! The outbox: what decisions, new conferences and changes of apps still
//! have to write into the organization's repo in the space. Entries are
//! added in the same transaction as what caused them, and applied after it
//! commits, in order, by one drain at a time: within a process a mutex
//! keeps drains apart, and between processes (the server and the CLI) a
//! lease in the database does, held by a token per drain. The CLI applies
//! its own entries before it exits; the server applies whatever is left, on
//! start, whenever a request [`kick`]s it, and every half second.
//!
//! Applying a member entry writes the subject's records as the decisions
//! log has them *now*, so entries can be applied twice, or late, and the
//! records still end up matching the last decision.

use std::collections::HashSet;
use std::time::Duration;

use tokio::sync::{Mutex, Notify};

use serde_json::{Value, json};

use super::decide::{self, Decision};
use super::{APPS, BAN, FEED, MEMBER, attest, iso, repo};
use crate::AppState;
use crate::db::now_ms;
use crate::keys::random_token;

/// Write a person's member and ban records as their standing is now.
pub const MEMBER_ENTRY: &str = "member";
/// A conference was created: write its main feed.
pub const CREATED_ENTRY: &str = "conference.created";
/// A conference's allowed apps changed: write its apps record.
pub const APPS_ENTRY: &str = "apps";

/// How long a process holds the lease without renewing it.
const LEASE_MS: i64 = 30_000;
/// How often the server looks for entries to apply.
const POLL: Duration = Duration::from_millis(500);
/// Entries looked at in one go.
const BATCH: i64 = 50;
/// The longest wait before an entry that failed is tried again.
const MAX_RETRY_MS: i64 = 60_000;

/// One drain at a time in this process.
static DRAINING: Mutex<()> = Mutex::const_new(());
/// Wakes the server's drain loop.
static WAKE: Notify = Notify::const_new();

/// Asks the server's drain loop to apply the outbox now, rather than at its
/// next poll.
pub fn kick() {
    WAKE.notify_one();
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct Entry {
    seq: i64,
    conference: String,
    kind: String,
    subject: Option<String>,
    attempts: i64,
}

async fn take_lease(state: &AppState, holder: &str) -> Result<bool, String> {
    let now = now_ms();
    let taken = sqlx::query(
        "UPDATE outbox_lease SET holder = $1, until_ms = $2 \
         WHERE slot = 1 AND (until_ms < $3 OR holder = $1)",
    )
    .bind(holder)
    .bind(now + LEASE_MS)
    .bind(now)
    .execute(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    Ok(taken.rows_affected() == 1)
}

async fn release_lease(state: &AppState, holder: &str) {
    let released = sqlx::query(
        "UPDATE outbox_lease SET holder = NULL, until_ms = 0 WHERE slot = 1 AND holder = $1",
    )
    .bind(holder)
    .execute(&state.db)
    .await;
    if let Err(err) = released {
        eprintln!("outbox: could not release the lease: {err}");
    }
}

/// Applies every entry that's due, unless another process is. Waits for any
/// drain already running in this process. Returns whether it held the lease.
pub async fn drain(state: &AppState) -> Result<bool, String> {
    let _one = DRAINING.lock().await;
    let holder = random_token(12);
    if !take_lease(state, &holder).await? {
        return Ok(false);
    }
    let result = drain_leased(state, &holder).await;
    release_lease(state, &holder).await;
    result.map(|()| true)
}

async fn drain_leased(state: &AppState, holder: &str) -> Result<(), String> {
    // A conference whose entry failed waits for its turn again, so its
    // records are still written in order.
    let mut failed: HashSet<String> = HashSet::new();
    loop {
        let entries = sqlx::query_as::<_, Entry>(
            "SELECT seq, conference, kind, subject, attempts FROM outbox \
             WHERE applied_at IS NULL AND (retry_at IS NULL OR retry_at <= $1) ORDER BY seq LIMIT $2",
        )
        .bind(now_ms())
        .bind(BATCH)
        .fetch_all(&state.db)
        .await
        .map_err(|e| e.to_string())?;
        let entries: Vec<Entry> =
            entries.into_iter().filter(|e| !failed.contains(&e.conference)).collect();
        if entries.is_empty() {
            return Ok(());
        }
        for (i, entry) in entries.iter().enumerate() {
            if failed.contains(&entry.conference) {
                continue;
            }
            if !take_lease(state, holder).await? {
                return Ok(());
            }
            // A later entry about the same thing writes the same state.
            let superseded = entries[i + 1..].iter().any(|later| {
                later.conference == entry.conference
                    && later.kind == entry.kind
                    && later.subject == entry.subject
            });
            let applied = if superseded { Ok(()) } else { apply(state, entry).await };
            match applied {
                Ok(()) => {
                    sqlx::query(
                        "UPDATE outbox SET applied_at = $1, last_error = NULL WHERE seq = $2",
                    )
                    .bind(now_ms())
                    .bind(entry.seq)
                    .execute(&state.db)
                    .await
                    .map_err(|e| e.to_string())?;
                }
                Err(why) => {
                    eprintln!(
                        "outbox: entry {} ({} in {}) failed: {why}",
                        entry.seq, entry.kind, entry.conference
                    );
                    let wait = (1000_i64 << entry.attempts.clamp(0, 16)).min(MAX_RETRY_MS);
                    sqlx::query(
                        "UPDATE outbox SET attempts = attempts + 1, retry_at = $1, last_error = $2 WHERE seq = $3",
                    )
                    .bind(now_ms() + wait)
                    .bind(&why)
                    .bind(entry.seq)
                    .execute(&state.db)
                    .await
                    .map_err(|e| e.to_string())?;
                    failed.insert(entry.conference.clone());
                }
            }
        }
    }
}

/// Applies the outbox until every entry there now for a conference is
/// applied, for a process (the CLI) that's about to exit: whoever holds the
/// lease, the entries get applied. Gives up after `timeout`, with the last
/// error. Other conferences' entries, which may be failing for reasons of
/// their own, don't hold it up.
pub async fn settle(state: &AppState, conference: &str, timeout: Duration) -> Result<(), String> {
    let target = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(MAX(seq), 0) FROM outbox WHERE conference = $1",
    )
    .bind(conference)
    .fetch_one(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    let until = now_ms() + timeout.as_millis() as i64;
    loop {
        drain(state).await?;
        let pending = sqlx::query_as::<_, (i64, Option<String>)>(
            "SELECT seq, last_error FROM outbox WHERE applied_at IS NULL AND conference = $1 AND seq <= $2 \
             ORDER BY seq LIMIT 1",
        )
        .bind(conference)
        .bind(target)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| e.to_string())?;
        match pending {
            None => return Ok(()),
            Some((_, error)) if now_ms() > until => {
                return Err(error.unwrap_or_else(|| "it's still waiting to be written".into()));
            }
            Some(_) => tokio::time::sleep(Duration::from_millis(200)).await,
        }
    }
}

/// Applies the outbox in the background for as long as the server runs,
/// starting with whatever an earlier run (or a CLI that stopped) left.
pub fn spawn(state: AppState) {
    tokio::spawn(async move {
        loop {
            if let Err(err) = drain(&state).await {
                eprintln!("outbox: {err}");
            }
            let _ = tokio::time::timeout(POLL, WAKE.notified()).await;
        }
    });
}

async fn apply(state: &AppState, entry: &Entry) -> Result<(), String> {
    match entry.kind.as_str() {
        MEMBER_ENTRY => {
            let subject = entry.subject.as_deref().ok_or("a member entry names nobody")?;
            write_member(state, &entry.conference, subject).await
        }
        CREATED_ENTRY => write_main_feed(state, &entry.conference).await,
        APPS_ENTRY => write_apps(state, &entry.conference).await,
        other => Err(format!("unknown outbox entry {other}")),
    }
}

/// Signs a record as eventside, for the organization's repo.
async fn signed(state: &AppState, space: &str, mut record: Value) -> Result<Value, String> {
    let org =
        super::SpaceUri::parse(space).ok_or_else(|| format!("{space} isn't a space"))?.authority;
    let (fragment, key) = attest::current(&state.db).await?;
    let key_id = format!("{}#{fragment}", super::eventside_did(&state.oauth.public_url));
    attest::sign(&mut record, &key_id, &key, &org)?;
    Ok(record)
}

/// A person's member and ban records, as their standing is now.
async fn write_member(state: &AppState, conference: &str, subject: &str) -> Result<(), String> {
    let decisions = decide::decisions_about(&state.db, conference, subject).await?;
    let standing = decide::standing(&decisions, subject, None);
    match (standing.role, &standing.membership) {
        (Some(role), Some(decision)) => {
            let admitted = decisions.iter().rev().find(|d| d.action == "admit");
            let mut record = decided(MEMBER, subject, decision);
            record["role"] = json!(role.as_str());
            if let Some(method) = admitted.and_then(|d| d.method.as_deref()) {
                record["method"] = json!(method);
            }
            let record = signed(state, conference, record).await?;
            repo::put(state, conference, MEMBER, subject, record).await?;
        }
        _ if decisions.iter().any(|d| d.action == "admit") => {
            repo::delete(state, conference, MEMBER, subject).await?;
        }
        _ => {}
    }
    match &standing.ban {
        Some(decision) => {
            let record = signed(state, conference, decided(BAN, subject, decision)).await?;
            repo::put(state, conference, BAN, subject, record).await?;
        }
        None if decisions.iter().any(|d| d.action == "ban") => {
            repo::delete(state, conference, BAN, subject).await?;
        }
        None => {}
    }
    Ok(())
}

/// The fields every decision's record carries.
fn decided(collection: &str, subject: &str, decision: &Decision) -> Value {
    json!({
        "$type": collection,
        "subject": subject,
        "decidedBy": decision.actor,
        "decidedRank": decision.rank,
        "seq": decision.seq,
        "decidedAt": iso(decision.decided_at),
        "createdAt": iso(decision.decided_at),
    })
}

/// The conference's main feed, from its template (`feeds` builds on it).
async fn write_main_feed(state: &AppState, conference: &str) -> Result<(), String> {
    let record = json!({
        "$type": FEED,
        "name": "Main",
        "template": "main",
        "createdAt": iso(now_ms()),
    });
    repo::put(state, conference, FEED, "main", record).await
}

/// The conference's allowed apps, signed.
async fn write_apps(state: &AppState, conference: &str) -> Result<(), String> {
    let rows = sqlx::query_as::<_, (String, String)>(
        "SELECT client_id, uses FROM conference_apps WHERE conference = $1 ORDER BY client_id",
    )
    .bind(conference)
    .fetch_all(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    let apps: Vec<Value> = rows
        .into_iter()
        .map(|(client, uses)| json!({ "client": client, "uses": super::split_methods(&uses) }))
        .collect();
    let record = json!({ "$type": APPS, "apps": apps, "createdAt": iso(now_ms()) });
    let record = signed(state, conference, record).await?;
    repo::put(state, conference, APPS, "self", record).await
}
