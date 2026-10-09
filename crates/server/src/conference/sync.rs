//! Space sync (the `space-sync` feature, folded in by design round 1):
//! eventside reads every writer's records in a conference's space into its
//! records index, with its own read access (a credential delegated by the
//! organization; see [`repo::credential`]).
//!
//! - **Write notifications:** eventside registers at the space's host for
//!   them (`registerNotify`, at its `#eventside_access` service), and reads
//!   the writer's new ops when one arrives ([`notify_write`]).
//! - **Backfill:** the server reads every conference's writers on start and
//!   every so often after ([`spawn`]), and `records list` reads them first.
//! - **Ingest:** each record version goes through [`counts`], the rule other
//!   features rely on: a member's record counts if they were a member at the
//!   revision their PDS gave it; the organization's records count, and the
//!   ones eventside signs only with a valid signature; nobody else's
//!   membership records count. Then every hook registered with
//!   [`on_ingest`] (feeds, later) sees it.
//!
//! Serving also needs the author to be a member *now* ([`counted_records`]).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};

use super::decide;
use super::{APPS, BAN, Conference, MEMBER, attest, eventside_did, repo};
use crate::AppState;
use crate::auth::xrpc_error;
use crate::crypto::tid_ms;
use crate::db::now_ms;

/// Records eventside signs: they count only from the organization's repo,
/// with a valid signature.
const SIGNED: &[&str] = &[MEMBER, BAN, APPS];

/// How often the server backfills every conference.
const BACKFILL: Duration = Duration::from_secs(30);
/// How often eventside registers again for write notifications (they last a day).
const REGISTER_MS: i64 = 6 * 60 * 60 * 1000;

/// A record version, as the index took it in.
#[derive(Debug, Clone)]
pub struct Ingested<'a> {
    pub space: &'a str,
    pub repo: &'a str,
    pub collection: &'a str,
    pub rkey: &'a str,
    /// The revision the writer's PDS gave the commit that wrote it.
    pub rev: &'a str,
    /// `None` when the record was deleted.
    pub value: Option<&'a Value>,
    /// Whether it counts, by [`counts`].
    pub counted: bool,
}

/// Something another feature does with each record the index takes in.
pub type Hook = Arc<dyn Fn(&AppState, &Ingested<'_>) + Send + Sync>;

static HOOKS: Mutex<Vec<Hook>> = Mutex::new(Vec::new());

/// Registers an ingest hook: it sees every record version as it's indexed.
pub fn on_ingest(hook: Hook) {
    HOOKS.lock().expect("the ingest hooks aren't poisoned").push(hook);
}

/// The ingest rule: whether a record version in a conference's space counts.
pub async fn counts(
    state: &AppState,
    conference: &Conference,
    repo: &str,
    collection: &str,
    rev: &str,
    value: &Value,
) -> Result<bool, String> {
    if repo == conference.org {
        if !SIGNED.contains(&collection) {
            return Ok(true);
        }
        let keys: Vec<_> = attest::keys(&state.db)
            .await?
            .into_iter()
            .map(|(fragment, key)| (fragment, key.verifying_key()))
            .collect();
        let signer = eventside_did(&state.oauth.public_url);
        return Ok(!attest::verify(value, &conference.org, &signer, &keys).is_empty());
    }
    // Only eventside, in the organization's repo, says who's in.
    if SIGNED.contains(&collection) {
        return Ok(false);
    }
    match tid_ms(rev) {
        Some(at) => decide::is_member_at(&state.db, &conference.space, repo, at).await,
        None => Ok(false),
    }
}

/// Reads a writer's new ops in a conference's space into the index.
/// Returns how many record versions it took in. Safe to run twice at once: a
/// record only ever moves to a later revision.
pub async fn sync_repo(
    state: &AppState,
    conference: &Conference,
    writer: &str,
) -> Result<usize, String> {
    let since = sqlx::query_scalar::<_, String>(
        "SELECT synced_rev FROM space_repos WHERE space = $1 AND repo = $2",
    )
    .bind(&conference.space)
    .bind(writer)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    let ops = repo::repo_ops(state, &conference.space, writer, since.as_deref()).await?;
    let Some(latest) = ops.iter().map(|op| op.rev.clone()).max() else { return Ok(0) };
    // The last op on each record is the version to keep.
    let mut last: BTreeMap<(&str, &str), &repo::Op> = BTreeMap::new();
    for op in &ops {
        last.insert((op.collection.as_str(), op.rkey.as_str()), op);
    }
    let mut taken = 0;
    for ((collection, rkey), op) in last {
        let value = match (&op.cid, &op.value) {
            (None, _) => None,
            (Some(_), Some(value)) => Some(value),
            // Changed again since: a later read carries it.
            (Some(_), None) => continue,
        };
        let counted = match value {
            Some(value) => counts(state, conference, writer, collection, &op.rev, value).await?,
            None => false,
        };
        let stored = sqlx::query(
            "INSERT INTO space_records (space, repo, collection, rkey, rev, value, counted, indexed_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
             ON CONFLICT (space, repo, collection, rkey) DO UPDATE SET rev = excluded.rev, \
             value = excluded.value, counted = excluded.counted, indexed_at = excluded.indexed_at \
             WHERE space_records.rev < excluded.rev",
        )
        .bind(&conference.space)
        .bind(writer)
        .bind(collection)
        .bind(rkey)
        .bind(&op.rev)
        .bind(value.map(Value::to_string))
        .bind(i64::from(counted))
        .bind(now_ms())
        .execute(&state.db)
        .await
        .map_err(|e| e.to_string())?;
        if stored.rows_affected() == 0 {
            continue;
        }
        taken += 1;
        let ingested = Ingested {
            space: &conference.space,
            repo: writer,
            collection,
            rkey,
            rev: &op.rev,
            value,
            counted,
        };
        let hooks = HOOKS.lock().expect("the ingest hooks aren't poisoned").clone();
        for hook in hooks {
            hook(state, &ingested);
        }
    }
    sqlx::query(
        "INSERT INTO space_repos (space, repo, synced_rev, synced_at) VALUES ($1, $2, $3, $4) \
         ON CONFLICT (space, repo) DO UPDATE SET synced_rev = excluded.synced_rev, synced_at = excluded.synced_at \
         WHERE space_repos.synced_rev < excluded.synced_rev",
    )
    .bind(&conference.space)
    .bind(writer)
    .bind(&latest)
    .bind(now_ms())
    .execute(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    Ok(taken)
}

/// Backfills a conference: every writer the space's host lists, everyone
/// ever admitted (whose writes the host may have refused), and the
/// organization. A writer that can't be read is skipped this time.
pub async fn sync_space(state: &AppState, conference: &Conference) -> Result<(), String> {
    let mut writers = vec![conference.org.clone()];
    let decisions = decide::decisions(&state.db, &conference.space).await?;
    for d in decisions.iter().filter(|d| d.action == "admit") {
        if !writers.contains(&d.subject) {
            writers.push(d.subject.clone());
        }
    }
    match repo::writers(state, &conference.space).await {
        Ok(listed) => {
            for did in listed {
                if !writers.contains(&did) {
                    writers.push(did);
                }
            }
        }
        Err(why) => return Err(format!("couldn't list {}'s writers: {why}", conference.space)),
    }
    for writer in writers {
        if let Err(why) = sync_repo(state, conference, &writer).await {
            eprintln!("sync: couldn't read {writer} in {}: {why}", conference.space);
        }
    }
    sqlx::query(
        "INSERT INTO space_sync (space, registered_at, synced_at) VALUES ($1, 0, $2) \
         ON CONFLICT (space) DO UPDATE SET synced_at = excluded.synced_at",
    )
    .bind(&conference.space)
    .bind(now_ms())
    .execute(&state.db)
    .await
    .map(drop)
    .map_err(|e| e.to_string())
}

/// Registers for a conference's write notifications, unless that was done lately.
async fn ensure_registered(state: &AppState, conference: &Conference) -> Result<(), String> {
    let registered =
        sqlx::query_scalar::<_, i64>("SELECT registered_at FROM space_sync WHERE space = $1")
            .bind(&conference.space)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| e.to_string())?
            .unwrap_or(0);
    if now_ms() - registered < REGISTER_MS {
        return Ok(());
    }
    repo::register_notify(state, &conference.space).await?;
    sqlx::query(
        "INSERT INTO space_sync (space, registered_at, synced_at) VALUES ($1, $2, 0) \
         ON CONFLICT (space) DO UPDATE SET registered_at = excluded.registered_at",
    )
    .bind(&conference.space)
    .bind(now_ms())
    .execute(&state.db)
    .await
    .map(drop)
    .map_err(|e| e.to_string())
}

/// The records eventside counts in a conference's space, in one collection:
/// indexed as counting, by the organization or by someone who's a member now.
pub async fn counted_records(
    state: &AppState,
    conference: &Conference,
    collection: &str,
) -> Result<Vec<Value>, String> {
    let rows = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT repo, rkey, rev, value FROM space_records \
         WHERE space = $1 AND collection = $2 AND counted = 1 AND value IS NOT NULL ORDER BY rev",
    )
    .bind(&conference.space)
    .bind(collection)
    .fetch_all(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    let standings = decide::standings(&decide::decisions(&state.db, &conference.space).await?);
    Ok(rows
        .into_iter()
        .filter(|(repo, ..)| {
            *repo == conference.org || standings.get(repo).is_some_and(|s| s.is_member())
        })
        .map(|(repo, rkey, rev, value)| {
            json!({
                "uri": format!("{}/{repo}/{collection}/{rkey}", conference.space),
                "repo": repo,
                "collection": collection,
                "rkey": rkey,
                "rev": rev,
                "value": serde_json::from_str::<Value>(&value).unwrap_or(Value::Null),
            })
        })
        .collect())
}

/// Keeps every conference's index current for as long as the server runs:
/// registers for write notifications and backfills, on start and every
/// [`BACKFILL`] after.
pub fn spawn(state: AppState) {
    tokio::spawn(async move {
        loop {
            let spaces = sqlx::query_scalar::<_, String>(
                "SELECT space FROM conferences WHERE status = 'ready'",
            )
            .fetch_all(&state.db)
            .await
            .unwrap_or_default();
            for space in spaces {
                let Ok(Some(conference)) = super::load(&state.db, &space).await else { continue };
                if let Err(why) = ensure_registered(&state, &conference).await {
                    eprintln!("sync: {why}");
                }
                if let Err(why) = sync_space(&state, &conference).await {
                    eprintln!("sync: {why}");
                }
            }
            tokio::time::sleep(BACKFILL).await;
        }
    });
}

#[derive(Deserialize)]
pub struct WriteNotice {
    space: Option<String>,
    repo: Option<String>,
}

/// `com.atproto.space.notifyWrite`: the space's host says a writer's repo
/// moved on. Signed by the space's authority; eventside reads the new ops.
pub async fn notify_write(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(notice): Json<WriteNotice>,
) -> Response {
    let (Some(space), Some(writer)) = (notice.space, notice.repo) else {
        return xrpc_error(
            StatusCode::BAD_REQUEST,
            "InvalidRequest",
            "space and repo are required",
        );
    };
    let conference = match super::load(&state.db, &space).await {
        Ok(Some(conference)) => conference,
        Ok(None) => {
            return xrpc_error(StatusCode::NOT_FOUND, "SpaceNotFound", "not a conference here");
        }
        Err(why) => {
            eprintln!("notifyWrite: {why}");
            return xrpc_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "InternalServerError",
                "try again",
            );
        }
    };
    if let Err(why) = super::api::check_service_auth(
        &state,
        &headers,
        &conference.org,
        "com.atproto.space.notifyWrite",
    )
    .await
    {
        return xrpc_error(StatusCode::UNAUTHORIZED, "AuthenticationRequired", &why);
    }
    tokio::spawn(async move {
        if let Err(why) = sync_repo(&state, &conference, &writer).await {
            eprintln!("sync: couldn't read {writer} in {}: {why}", conference.space);
        }
    });
    Json(json!({})).into_response()
}
