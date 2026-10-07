//! Write notifications, the writer set, syncer registrations, and revoking
//! credentials at writers' PDSes.

use std::collections::BTreeMap;

use axum::Json;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use super::credential::{self, ip_allowed};
use super::{SpaceUri, authority, host_audience, index, internal, invalid, refuse, sync};
use crate::AppState;
use crate::crypto::{Jwt, tid_after, tid_micros, tid_now};
use crate::db::now_ms;
use crate::keys::now_secs;

const NOTIFY_WRITE: &str = "com.atproto.space.notifyWrite";
const REVOKED: &str = "com.atproto.space.notifyCredentialRevoked";
/// How far ahead of our clock a revision may be.
const FUTURE_MS: i64 = 5 * 60_000;
/// How long a syncer's registration lasts.
const REGISTRATION_MS: i64 = 24 * 60 * 60_000;
/// How long a writer's PDS keeps a revocation, so how far back a newly seen
/// PDS is sent them.
const REVOCATIONS_KEPT_MS: i64 = 60 * 60_000;

/// `com.atproto.space.notifyWrite`: a writer's PDS saying their repo in one
/// of our spaces moved on. Non-members are refused (a deliberate 403, which
/// the PDS doesn't retry) before anything is resolved.
pub async fn notify_write(
    State(state): State<AppState>,
    super::ClientIp(ip): super::ClientIp,
    headers: HeaderMap,
    body: Option<Json<Value>>,
) -> Response {
    let body = body.map(|Json(b)| b).unwrap_or_default();
    let field = |name: &str| body.get(name).and_then(Value::as_str);
    let (Some(space), Some(repo), Some(repo_rev), Some(hash)) = (
        field("space").and_then(SpaceUri::parse),
        field("repo").filter(|d| crate::identity::is_valid_did(d)),
        field("repoRev"),
        body.get("hash").and_then(|h| h.get("$bytes")).and_then(Value::as_str),
    ) else {
        return invalid(
            "InvalidRequest",
            "Input must have \"space\", \"repo\", \"repoRev\" and \"hash\"",
        );
    };
    if let Some(ip) = ip
        && !ip_allowed(&state, "notify", ip)
    {
        return refuse(StatusCode::TOO_MANY_REQUESTS, "RateLimitExceeded", "Too many requests");
    }
    let org = match index::load_for_space(&state, &space).await {
        Ok(Some(org)) if org.knows(&space) => org,
        Ok(_) => return refuse(StatusCode::NOT_FOUND, "SpaceNotFound", "Space not found"),
        Err(why) => return internal(why),
    };
    if !org.can_write(&space, repo) {
        return refuse(StatusCode::FORBIDDEN, "Forbidden", "notifyWrite writer is not authorized");
    }
    if let Err(why) = verify_service_auth(&state, &headers, &space, repo).await {
        return refuse(StatusCode::UNAUTHORIZED, "InvalidToken", &why);
    }
    // Counted only once it's really from them, so nobody else can use up
    // a writer's allowance.
    if !state.host.allow(&format!("notify:{repo}"), 600, 60_000) {
        return refuse(StatusCode::TOO_MANY_REQUESTS, "RateLimitExceeded", "Too many requests");
    }
    let Some(rev_us) = tid_micros(repo_rev) else {
        return invalid("InvalidRequest", &format!("Invalid repoRev: {repo_rev}"));
    };
    if (rev_us / 1000) as i64 > now_ms() + FUTURE_MS {
        return invalid("FutureRev", "Repo revision is in the future");
    }
    let space_uri = space.to_string();
    let recorded = match record_writer(&state, &space_uri, repo, repo_rev, hash).await {
        Ok(recorded) => recorded,
        Err(why) => return internal(why),
    };
    if let Some((space_rev, first)) = recorded {
        let background = state.clone();
        let (space_uri, repo, repo_rev, hash) =
            (space_uri.clone(), repo.to_owned(), repo_rev.to_owned(), hash.to_owned());
        tokio::spawn(async move {
            // A write from any app (an admin's ban, a member's leave) can take
            // someone's access away: whoever lost it is revoked.
            let before = index::load(&background, &space.authority).await.ok().flatten();
            if let Err(why) = sync::sync_repo(&background, &space_uri, &repo).await {
                eprintln!("notifyWrite: could not read {repo} in {space_uri}: {why}");
            }
            if let Some(before) = before
                && let Ok(Some(after)) = index::load(&background, &space.authority).await
            {
                let before = crate::conference::access(&before);
                crate::conference::revoke_lost(&background, &before, &after).await;
            }
            if first {
                send_recent_revocations(&background, &space_uri, &repo).await;
            }
            forward(&background, &space_uri, &repo, &repo_rev, &hash, &space_rev).await;
        });
    }
    StatusCode::OK.into_response()
}

/// Checks a write notification's service auth: from the writer, to us as
/// the authority's space host, for this method.
async fn verify_service_auth(
    state: &AppState,
    headers: &HeaderMap,
    space: &SpaceUri,
    repo: &str,
) -> Result<(), String> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split_once(' '))
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
        .map(|(_, t)| t.trim().to_owned())
        .ok_or("service auth required")?;
    let jwt = Jwt::decode(&token)?;
    if jwt.claim_str("iss") != Some(repo) {
        return Err("notifyWrite iss does not match the writer".into());
    }
    let aud = jwt.claim_str("aud").unwrap_or_default();
    if aud != host_audience(&space.authority) && aud != space.authority {
        return Err("the service token isn't for this space host".into());
    }
    if jwt.claim_str("lxm").is_some_and(|lxm| lxm != NOTIFY_WRITE) {
        return Err("the service token is for another method".into());
    }
    if jwt.expired(now_secs()) {
        return Err("the service token has expired".into());
    }
    let key = state.host.signing_key(state, repo, jwt.header_str("kid")).await?;
    jwt.verify(&key)
}

/// Records a writer's new revision with the next space revision. Returns the
/// space revision and whether the writer is new, or `None` for a stale one.
async fn record_writer(
    state: &AppState,
    space: &str,
    did: &str,
    repo_rev: &str,
    hash: &str,
) -> Result<Option<(String, bool)>, String> {
    let _sequence = state.host.sequence.lock().await;
    let existing = sqlx::query_scalar::<_, String>(
        "SELECT repo_rev FROM space_writers WHERE space = $1 AND did = $2",
    )
    .bind(space)
    .bind(did)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    if existing.as_deref().is_some_and(|rev| rev >= repo_rev) {
        return Ok(None);
    }
    let latest = sqlx::query_scalar::<_, Option<String>>(
        "SELECT MAX(space_rev) FROM space_writers WHERE space = $1",
    )
    .bind(space)
    .fetch_one(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    let space_rev = match latest {
        Some(latest) => {
            let next = tid_now();
            if next > latest { next } else { tid_after(&latest) }
        }
        None => tid_now(),
    };
    sqlx::query(
        "INSERT INTO space_writers (space, did, repo_rev, hash, space_rev, first_seen_at) VALUES ($1, $2, $3, $4, $5, $6) \
         ON CONFLICT (space, did) DO UPDATE SET repo_rev = excluded.repo_rev, hash = excluded.hash, space_rev = excluded.space_rev",
    )
    .bind(space)
    .bind(did)
    .bind(repo_rev)
    .bind(hash)
    .bind(&space_rev)
    .bind(now_ms())
    .execute(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    Ok(Some((space_rev, existing.is_none())))
}

/// Tells the syncers registered for a space about a writer's new revision.
async fn forward(
    state: &AppState,
    space: &str,
    repo: &str,
    repo_rev: &str,
    hash: &str,
    space_rev: &str,
) {
    let services = sqlx::query_scalar::<_, String>(
        "SELECT service FROM space_notify WHERE space = $1 AND expires_at > $2",
    )
    .bind(space)
    .bind(now_ms())
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    if services.is_empty() {
        return;
    }
    let Some(parsed) = SpaceUri::parse(space) else { return };
    let Ok(key) = authority::space_key(state, &parsed.authority).await else { return };
    let body = json!({ "space": space, "repo": repo, "repoRev": repo_rev, "hash": { "$bytes": hash }, "spaceRev": space_rev });
    for service in services {
        let Some(endpoint) = resolve_service(state, &service).await else { continue };
        let token = authority::service_jwt(&key, &parsed.authority, &service, NOTIFY_WRITE);
        let url = format!("{endpoint}/xrpc/{NOTIFY_WRITE}");
        let Ok(client) = state.http.guarded(&url) else { continue };
        if let Err(err) = client.post(&url).bearer_auth(token).json(&body).send().await {
            eprintln!("notifyWrite: could not forward to {service}: {err}");
        }
    }
}

/// A service ID's endpoint: `did#fragment`, from the DID's document.
async fn resolve_service(state: &AppState, service: &str) -> Option<String> {
    let (did, fragment) = service.split_once('#').unwrap_or((service, "atproto_pds"));
    let doc = state.host.did_document(state, did).await.ok()?;
    super::service_endpoint(&doc, fragment)
}

#[derive(serde::Deserialize)]
pub struct ListReposParams {
    space: String,
    cursor: Option<String>,
    limit: Option<i64>,
}

/// `com.atproto.space.listRepos`: the writer set, after an exclusive
/// space-revision cursor.
pub async fn list_repos(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ListReposParams>,
) -> Response {
    let Some(space) = SpaceUri::parse(&params.space) else {
        return invalid("InvalidRequest", "Invalid space");
    };
    match index::load_for_space(&state, &space).await {
        Ok(Some(org)) if org.knows(&space) => {}
        Ok(_) => return refuse(StatusCode::NOT_FOUND, "SpaceNotFound", "Space not found"),
        Err(why) => return internal(why),
    }
    let caller = match credential::check(&state, &headers, &space).await {
        Ok(caller) => caller,
        Err(res) => return res,
    };
    if !state.host.allow(&format!("listRepos:{}", caller.key_id), 600, 60_000) {
        return refuse(StatusCode::TOO_MANY_REQUESTS, "RateLimitExceeded", "Too many requests");
    }
    let limit = params.limit.unwrap_or(100).clamp(1, 1000);
    let rows = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT did, repo_rev, hash, space_rev FROM space_writers WHERE space = $1 AND space_rev > $2 \
         ORDER BY space_rev LIMIT $3",
    )
    .bind(space.to_string())
    .bind(params.cursor.unwrap_or_default())
    .bind(limit)
    .fetch_all(&state.db)
    .await;
    let rows = match rows {
        Ok(rows) => rows,
        Err(err) => return internal(err),
    };
    let cursor = rows.last().map(|(_, _, _, space_rev)| space_rev.clone());
    let repos: Vec<Value> = rows
        .into_iter()
        .map(|(did, repo_rev, hash, space_rev)| {
            json!({ "did": did, "repoRev": repo_rev, "hash": { "$bytes": hash }, "spaceRev": space_rev })
        })
        .collect();
    let mut out = json!({ "repos": repos });
    if let Some(cursor) = cursor {
        out["cursor"] = json!(cursor);
    }
    Json(out).into_response()
}

/// `com.atproto.space.registerNotify`: a syncer asking for a space's write
/// notifications.
pub async fn register_notify(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Option<Json<Value>>,
) -> Response {
    let body = body.map(|Json(b)| b).unwrap_or_default();
    let (Some(space), Some(service)) = (
        body.get("space").and_then(Value::as_str).and_then(SpaceUri::parse),
        body.get("service").and_then(Value::as_str),
    ) else {
        return invalid("InvalidRequest", "Input must have \"space\" and \"service\"");
    };
    let caller = match credential::check(&state, &headers, &space).await {
        Ok(caller) => caller,
        Err(res) => return res,
    };
    if resolve_service(&state, service).await.is_none() {
        return invalid("ServiceNotResolvable", &format!("Could not resolve service: {service}"));
    }
    let expires_at = now_ms() + REGISTRATION_MS;
    // Kept with whose access it came through, so it ends with that access.
    let stored = sqlx::query(
        "INSERT INTO space_notify (space, service, expires_at, delegator, client_id) VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (space, service) DO UPDATE SET expires_at = excluded.expires_at, \
         delegator = excluded.delegator, client_id = excluded.client_id",
    )
    .bind(space.to_string())
    .bind(service)
    .bind(expires_at)
    .bind(caller.delegator)
    .bind(caller.client_id)
    .execute(&state.db)
    .await;
    match stored {
        Ok(_) => Json(json!({ "expiresAt": index::iso(expires_at as u64 * 1000) })).into_response(),
        Err(err) => internal(err),
    }
}

/// `com.atproto.space.unregisterNotify`.
pub async fn unregister_notify(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Option<Json<Value>>,
) -> Response {
    let body = body.map(|Json(b)| b).unwrap_or_default();
    let (Some(space), Some(service)) = (
        body.get("space").and_then(Value::as_str).and_then(SpaceUri::parse),
        body.get("service").and_then(Value::as_str),
    ) else {
        return invalid("InvalidRequest", "Input must have \"space\" and \"service\"");
    };
    if let Err(res) = credential::check(&state, &headers, &space).await {
        return res;
    }
    match sqlx::query("DELETE FROM space_notify WHERE space = $1 AND service = $2")
        .bind(space.to_string())
        .bind(service)
        .execute(&state.db)
        .await
    {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) => internal(err),
    }
}

/// Revokes the credentials someone delegated for a space, and drops the
/// syncer registrations made with them.
pub async fn revoke(state: &AppState, space: &str, delegator: &str) -> Result<(), String> {
    sqlx::query("DELETE FROM space_notify WHERE space = $1 AND delegator = $2")
        .bind(space)
        .bind(delegator)
        .execute(&state.db)
        .await
        .map_err(|e| e.to_string())?;
    let jtis = credential::revoke_delegated(state, space, delegator).await?;
    send(state, space, &jtis).await
}

/// Revokes the credentials for a space of every app `lost` names (by client
/// ID, none for an unattested app) after a change to its app access, and
/// drops the syncer registrations made with them.
pub async fn revoke_apps(
    state: &AppState,
    space: &str,
    lost: impl Fn(Option<&str>) -> bool,
) -> Result<(), String> {
    let registered = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT service, client_id FROM space_notify WHERE space = $1",
    )
    .bind(space)
    .fetch_all(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    for (service, client_id) in registered {
        if lost(client_id.as_deref()) {
            sqlx::query("DELETE FROM space_notify WHERE space = $1 AND service = $2")
                .bind(space)
                .bind(&service)
                .execute(&state.db)
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    let jtis = credential::revoke_where(state, space, |_, client| lost(client)).await?;
    send(state, space, &jtis).await
}

/// How often undelivered revocations are sent again.
const RESEND_EVERY: std::time::Duration = std::time::Duration::from_secs(30);

/// Sends again, in the background, every revocation that hasn't reached
/// every writer's PDS yet (a PDS was down, or the admin CLI that revoked it
/// gave up), until it has or the credential expires.
pub fn spawn_resender(state: AppState) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(RESEND_EVERY);
        loop {
            tick.tick().await;
            if let Err(why) = resend(&state).await {
                eprintln!("revocation: {why}");
            }
        }
    });
}

/// One pass over the undelivered revocations, by space.
pub async fn resend(state: &AppState) -> Result<(), String> {
    let pending = sqlx::query_as::<_, (String, String)>(
        "SELECT space, jti FROM space_credentials WHERE revoked_at IS NOT NULL \
         AND revocation_sent_at IS NULL AND expires_at > $1 ORDER BY space",
    )
    .bind(now_ms())
    .fetch_all(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    let mut by_space: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (space, jti) in pending {
        by_space.entry(space).or_default().push(jti);
    }
    for (space, jtis) in by_space {
        if let Err(why) = send(state, &space, &jtis).await {
            eprintln!("revocation: will try again: {why}");
        }
    }
    Ok(())
}

/// Sends revocations to every writer's PDS: once per PDS, addressed to one
/// of its writers, at most 100 at a time. Once every PDS has them, they're
/// marked sent; until then the background loop sends them again.
async fn send(state: &AppState, space: &str, jtis: &[String]) -> Result<(), String> {
    if jtis.is_empty() {
        return Ok(());
    }
    deliver(state, space, jtis).await?;
    let now = now_ms();
    for jti in jtis {
        sqlx::query("UPDATE space_credentials SET revocation_sent_at = $1 WHERE jti = $2")
            .bind(now)
            .bind(jti)
            .execute(&state.db)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Sends revocations to each writer's PDS, addressed to one of its writers.
/// If a PDS refuses that one (its account was deleted or moved, say), the
/// writer is remembered as gone there for a while, and the next writer on it
/// is tried, up to [`WRITERS_PER_PDS`] a pass, so one gone account can't keep
/// a PDS from ever hearing of revocations. Any other refusal would be the
/// same for every writer, so it isn't sent again; a PDS that doesn't take
/// revocations at all is left alone for a while.
async fn deliver(state: &AppState, space: &str, jtis: &[String]) -> Result<(), String> {
    let mut by_pds: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for writer in sync::writers(state, space).await? {
        match state.host.pds_of(state, &writer).await {
            Ok(pds) => by_pds.entry(pds).or_default().push(writer),
            Err(why) => eprintln!("revocation: {why}"),
        }
    }
    let mut failures = Vec::new();
    for (pds, writers) in by_pds {
        if let Some(until) = resting_until(&pds) {
            failures.push(format!("{pds} doesn't take revocations; trying again after {until}"));
            continue;
        }
        let mut tried = Vec::new();
        let mut delivered = false;
        let order = writers_to_try(&pds, space, writers);
        if order.is_empty() {
            failures.push(format!("every writer on {pds} is gone there for now"));
            continue;
        }
        for writer in order {
            match send_revocations(state, space, &pds, &writer, jtis).await {
                Ok(()) => {
                    remember_writer(&pds, space, &writer);
                    delivered = true;
                    break;
                }
                Err(failure) => {
                    tried.push(failure.why);
                    match failure.kind {
                        Refusal::Writer => {
                            mark_gone(&pds, space, &writer);
                            continue;
                        }
                        Refusal::Unsupported => rest(&pds),
                        Refusal::Other => {}
                    }
                    break;
                }
            }
        }
        if !delivered {
            failures.push(tried.join("; "));
        }
    }
    if failures.is_empty() { Ok(()) } else { Err(failures.join("; ")) }
}

/// At most this many writers are tried on one PDS per pass. A writer's
/// refusal is a cheap 4xx, and later passes go on with the next ones.
const WRITERS_PER_PDS: usize = 10;
/// How long a PDS that doesn't take revocations is left alone.
const REST_MS: i64 = 10 * 60 * 1000;
/// How long a writer a PDS refused as gone is skipped there.
const GONE_MS: i64 = 60 * 60 * 1000;

type Memo<T> = std::sync::Mutex<std::collections::HashMap<String, T>>;

/// A memo key for a writer list: one PDS's writers in one space.
fn pds_in(pds: &str, space: &str) -> String {
    format!("{pds} {space}")
}

/// The writer each PDS last took a space's revocations for.
fn last_writers() -> &'static Memo<String> {
    static MEMO: std::sync::OnceLock<Memo<String>> = std::sync::OnceLock::new();
    MEMO.get_or_init(Default::default)
}

/// Writers a PDS refused as gone, by PDS and writer, until when they're
/// skipped there.
fn gone() -> &'static Memo<i64> {
    static MEMO: std::sync::OnceLock<Memo<i64>> = std::sync::OnceLock::new();
    MEMO.get_or_init(Default::default)
}

/// Remembers a writer as gone from a PDS, and forgets them as the writer it
/// last took a space's revocations for.
fn mark_gone(pds: &str, space: &str, writer: &str) {
    {
        let mut memo = gone().lock().expect("the gone memo isn't poisoned");
        if memo.len() >= 10_000 {
            memo.clear();
        }
        memo.insert(format!("{pds} {writer}"), now_ms() + GONE_MS);
    }
    let mut last = last_writers().lock().expect("the writer memo isn't poisoned");
    let key = pds_in(pds, space);
    if last.get(&key).is_some_and(|w| w == writer) {
        last.remove(&key);
    }
}

fn is_gone(pds: &str, writer: &str) -> bool {
    gone()
        .lock()
        .expect("the gone memo isn't poisoned")
        .get(&format!("{pds} {writer}"))
        .is_some_and(|until| *until > now_ms())
}

/// When each PDS that doesn't take revocations is tried again.
fn resting() -> &'static Memo<i64> {
    static MEMO: std::sync::OnceLock<Memo<i64>> = std::sync::OnceLock::new();
    MEMO.get_or_init(Default::default)
}

fn remember_writer(pds: &str, space: &str, writer: &str) {
    let mut memo = last_writers().lock().expect("the writer memo isn't poisoned");
    if memo.len() >= 10_000 {
        memo.clear();
    }
    memo.insert(pds_in(pds, space), writer.to_owned());
}

fn rest(pds: &str) {
    let mut memo = resting().lock().expect("the rest memo isn't poisoned");
    if memo.len() >= 10_000 {
        memo.clear();
    }
    memo.insert(pds.to_owned(), now_ms() + REST_MS);
}

/// When a resting PDS is tried again, if it's resting.
fn resting_until(pds: &str) -> Option<String> {
    let until = *resting().lock().expect("the rest memo isn't poisoned").get(pds)?;
    (until > now_ms()).then(|| crate::spacehost::index::iso(until as u64 * 1000))
}

/// The writers to address a PDS's revocations for a space to, at most
/// [`WRITERS_PER_PDS`]: the one it last took first, then the others, leaving
/// out those it refused as gone lately, so no run of gone writers keeps a
/// PDS from hearing of revocations for good.
fn writers_to_try(pds: &str, space: &str, mut writers: Vec<String>) -> Vec<String> {
    writers.retain(|w| !is_gone(pds, w));
    let last = last_writers()
        .lock()
        .expect("the writer memo isn't poisoned")
        .get(&pds_in(pds, space))
        .cloned();
    let first =
        last.and_then(|last| writers.iter().position(|w| *w == last)).map(|at| writers.remove(at));
    let mut order: Vec<String> = first.into_iter().chain(writers).collect();
    order.truncate(WRITERS_PER_PDS);
    order
}

/// Why a PDS didn't take revocations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Refusal {
    /// Not for this writer (gone, moved or deactivated): another may do.
    Writer,
    /// It doesn't take revocations at all.
    Unsupported,
    /// Anything else, which would be the same for any writer.
    Other,
}

struct Failure {
    why: String,
    kind: Refusal,
}

/// What a PDS's refusal says, from its status and its XRPC error body.
fn refusal(status: StatusCode, body: &str) -> Refusal {
    let parsed: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let error = parsed.get("error").and_then(Value::as_str).unwrap_or_default();
    let message = parsed.get("message").and_then(Value::as_str).unwrap_or_default();
    if status == StatusCode::NOT_FOUND
        || status == StatusCode::NOT_IMPLEMENTED
        || error == "MethodNotImplemented"
    {
        return Refusal::Unsupported;
    }
    let writer_gone = matches!(
        error,
        "RepoNotFound"
            | "AccountNotFound"
            | "AccountDeactivated"
            | "AccountTakedown"
            | "RepoDeactivated"
            | "RepoTakendown"
    ) || (status == StatusCode::FORBIDDEN && message.contains("hosted here"));
    if writer_gone { Refusal::Writer } else { Refusal::Other }
}

/// Sends revocations to one PDS, retrying a few times.
async fn send_revocations(
    state: &AppState,
    space: &str,
    pds: &str,
    writer: &str,
    jtis: &[String],
) -> Result<(), Failure> {
    let other = |why: String| Failure { why, kind: Refusal::Other };
    let parsed = SpaceUri::parse(space).ok_or_else(|| other("not a space".into()))?;
    let key = authority::space_key(state, &parsed.authority).await.map_err(other)?;
    let url = format!("{pds}/xrpc/{REVOKED}");
    let client = state.http.guarded(&url).map_err(other)?;
    for chunk in jtis.chunks(100) {
        let mut last = other(String::new());
        let mut delivered = false;
        for attempt in 0..3 {
            if attempt > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
            }
            let token = authority::service_jwt(&key, &parsed.authority, writer, REVOKED);
            match client
                .post(&url)
                .bearer_auth(token)
                .json(&json!({ "space": space, "credentials": chunk }))
                .send()
                .await
            {
                Ok(res) if res.status().is_success() => {
                    delivered = true;
                    break;
                }
                Ok(res) => {
                    let status = res.status();
                    let body = res.text().await.unwrap_or_default();
                    last = Failure {
                        why: format!("{url} answered {status} for {writer}: {body}"),
                        kind: refusal(status, &body),
                    };
                    // A refusal won't change on a retry.
                    if status.is_client_error() && status != StatusCode::TOO_MANY_REQUESTS {
                        break;
                    }
                }
                Err(err) => last = other(format!("{url}: {err}")),
            }
        }
        if !delivered {
            return Err(last);
        }
    }
    Ok(())
}

/// A writer seen for the first time may be on a PDS that missed the
/// revocations of the last hour: it's sent them.
async fn send_recent_revocations(state: &AppState, space: &str, writer: &str) {
    let jtis = sqlx::query_scalar::<_, String>(
        "SELECT jti FROM space_credentials WHERE space = $1 AND revoked_at > $2 AND expires_at > $3",
    )
    .bind(space)
    .bind(now_ms() - REVOCATIONS_KEPT_MS)
    .bind(now_ms())
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    if jtis.is_empty() {
        return;
    }
    let Ok(pds) = state.host.pds_of(state, writer).await else { return };
    if let Err(failure) = send_revocations(state, space, &pds, writer, &jtis).await {
        eprintln!("revocation: {}", failure.why);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_refusal_of_the_writer_moves_on_to_another() {
        let gone = r#"{"error":"Forbidden","message":"Revocation audience does not match a repo hosted here"}"#;
        assert_eq!(refusal(StatusCode::FORBIDDEN, gone), Refusal::Writer);
        let deactivated = r#"{"error":"AccountDeactivated","message":"gone"}"#;
        assert_eq!(refusal(StatusCode::BAD_REQUEST, deactivated), Refusal::Writer);
        let unknown = r#"{"error":"MethodNotImplemented","message":"no"}"#;
        assert_eq!(refusal(StatusCode::BAD_REQUEST, unknown), Refusal::Unsupported);
        assert_eq!(refusal(StatusCode::NOT_FOUND, ""), Refusal::Unsupported);
        let bad_jwt = r#"{"error":"AuthenticationRequired","message":"bad token"}"#;
        assert_eq!(refusal(StatusCode::UNAUTHORIZED, bad_jwt), Refusal::Other);
    }

    #[test]
    fn a_pds_hears_from_a_few_writers_the_one_it_last_took_first() {
        let pds = "https://pds.test.example";
        let space = "at://did:plc:a/space/app.eventside.conference/1";
        let writers: Vec<String> = (0..10).map(|i| format!("did:plc:w{i}")).collect();
        assert_eq!(writers_to_try(pds, space, writers.clone()), writers[..WRITERS_PER_PDS]);
        remember_writer(pds, space, "did:plc:w7");
        let order = writers_to_try(pds, space, writers.clone());
        assert_eq!(order.len(), WRITERS_PER_PDS);
        assert_eq!(order[0], "did:plc:w7");
        // Another space's writer list on the same PDS isn't affected.
        let other = "at://did:plc:a/space/app.eventside.conference/2";
        assert_eq!(writers_to_try(pds, other, writers)[0], "did:plc:w0");
    }

    #[test]
    fn a_pds_whose_first_writers_are_gone_hears_from_the_later_ones() {
        let pds = "https://gone-first.test.example";
        let space = "at://did:plc:a/space/app.eventside.conference/1";
        let other = "at://did:plc:a/space/app.eventside.conference/2";
        let writers: Vec<String> = (0..25).map(|i| format!("did:plc:w{i:02}")).collect();
        let first = writers_to_try(pds, space, writers.clone());
        assert_eq!(first, writers[..WRITERS_PER_PDS]);
        // Every one of them was refused as gone: the next pass, in this
        // space or another with the same writers, tries the next ones.
        for writer in &first {
            mark_gone(pds, space, writer);
        }
        let second = writers_to_try(pds, other, writers.clone());
        assert_eq!(second, writers[WRITERS_PER_PDS..2 * WRITERS_PER_PDS]);
        for writer in &second {
            mark_gone(pds, other, writer);
        }
        assert_eq!(writers_to_try(pds, space, writers.clone()), writers[2 * WRITERS_PER_PDS..]);
        // Once one is taken, it's tried first; once it's gone, it isn't.
        remember_writer(pds, space, &writers[22]);
        assert_eq!(writers_to_try(pds, space, writers.clone())[0], "did:plc:w22");
        mark_gone(pds, space, &writers[22]);
        let after = writers_to_try(pds, space, writers.clone());
        assert!(!after.contains(&writers[22]));
        assert_eq!(after[0], "did:plc:w20");
        // Everyone gone: nobody to try until they're tried again later.
        for writer in &writers {
            mark_gone(pds, space, writer);
        }
        assert!(writers_to_try(pds, space, writers).is_empty());
    }
}
