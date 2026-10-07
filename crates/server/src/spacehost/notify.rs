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
    if !state.host.allow(&format!("notify:{repo}"), 600, 60_000) {
        return refuse(StatusCode::TOO_MANY_REQUESTS, "RateLimitExceeded", "Too many requests");
    }
    if let Err(why) = verify_service_auth(&state, &headers, &space, repo).await {
        return refuse(StatusCode::UNAUTHORIZED, "InvalidToken", &why);
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
            if let Err(why) = sync::sync_repo(&background, &space_uri, &repo).await {
                eprintln!("notifyWrite: could not read {repo} in {space_uri}: {why}");
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
    if let Err(res) = credential::check(&state, &headers, &space).await {
        return res;
    }
    if resolve_service(&state, service).await.is_none() {
        return invalid("ServiceNotResolvable", &format!("Could not resolve service: {service}"));
    }
    let expires_at = now_ms() + REGISTRATION_MS;
    let stored = sqlx::query(
        "INSERT INTO space_notify (space, service, expires_at) VALUES ($1, $2, $3) \
         ON CONFLICT (space, service) DO UPDATE SET expires_at = excluded.expires_at",
    )
    .bind(space.to_string())
    .bind(service)
    .bind(expires_at)
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

/// Revokes the credentials someone delegated for a space, at every writer's
/// PDS: once per PDS, addressed to one of its writers, at most 100 at a time.
pub async fn revoke(state: &AppState, space: &str, delegator: &str) -> Result<(), String> {
    let jtis = credential::revoke_delegated(state, space, delegator).await?;
    if jtis.is_empty() {
        return Ok(());
    }
    let mut by_pds: BTreeMap<String, String> = BTreeMap::new();
    for writer in sync::writers(state, space).await? {
        match state.host.pds_of(state, &writer).await {
            Ok(pds) => {
                by_pds.entry(pds).or_insert(writer);
            }
            Err(why) => eprintln!("revocation: {why}"),
        }
    }
    let mut failures = Vec::new();
    for (pds, writer) in by_pds {
        if let Err(why) = send_revocations(state, space, &pds, &writer, &jtis).await {
            failures.push(why);
        }
    }
    if failures.is_empty() { Ok(()) } else { Err(failures.join("; ")) }
}

/// Sends revocations to one PDS, retrying a few times.
async fn send_revocations(
    state: &AppState,
    space: &str,
    pds: &str,
    writer: &str,
    jtis: &[String],
) -> Result<(), String> {
    let parsed = SpaceUri::parse(space).ok_or("not a space")?;
    let key = authority::space_key(state, &parsed.authority).await?;
    let url = format!("{pds}/xrpc/{REVOKED}");
    let client = state.http.guarded(&url)?;
    for chunk in jtis.chunks(100) {
        let mut last = String::new();
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
                    last = format!(
                        "{url} answered {}: {}",
                        res.status(),
                        res.text().await.unwrap_or_default()
                    )
                }
                Err(err) => last = format!("{url}: {err}"),
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
    if let Err(why) = send_revocations(state, space, &pds, writer, &jtis).await {
        eprintln!("revocation: {why}");
    }
}
