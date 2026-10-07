//! Conferences: the conference model on top of the space host, the app's
//! conference XRPC (`app.eventside.conference.*`), membership and roles for
//! later features, and the admin CLI.
//!
//! A conference is a space (`app.eventside.conference`) under its
//! organization's authority, with an intake space for joining and leaving.
//! A public one has a `community.lexicon.calendar.event` and an
//! `app.eventside.conference` sidecar in its super admin's public repo; an
//! invite-only one has them inside the space.

pub mod admin;
pub mod cli;

use axum::Json;
use axum::Router;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use reqwest::Method;
use serde_json::{Value, json};

use crate::AppState;
use crate::auth::session::{self, Lookup};
use crate::auth::{CurrentUser, xrpc_error};
use crate::crypto::{tid_micros, tid_now};
use crate::db::now_ms;
use crate::spacehost::index::{self, Conference, JOIN, LEAVE, Org, Via};
use crate::spacehost::{CONFERENCE_TYPE, ClientIp, SpaceUri, authority, notify, sync};

pub const EVENT: &str = "community.lexicon.calendar.event";
pub const SIDECAR: &str = "app.eventside.conference";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/xrpc/app.eventside.conference.getConference", get(get_conference))
        .route("/xrpc/app.eventside.conference.listMyConferences", get(list_my_conferences))
        .route("/xrpc/app.eventside.conference.listRecords", get(list_records))
        .route("/xrpc/app.eventside.conference.join", post(join))
        .route("/xrpc/app.eventside.conference.leave", post(leave))
}

/// A conference's public facts, cached from its event and settings.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Row {
    pub space: String,
    pub org: String,
    pub intake: String,
    pub super_admin: String,
    pub event: Option<String>,
    pub invite_only: i64,
    pub info: String,
}

impl Row {
    pub fn info(&self) -> Value {
        serde_json::from_str(&self.info).unwrap_or_else(|_| json!({}))
    }
}

const ROW: &str =
    "SELECT space, org, intake, super_admin, event, invite_only, info FROM conferences";

pub async fn row(state: &AppState, space: &str) -> Result<Option<Row>, String> {
    sqlx::query_as::<_, Row>(&format!("{ROW} WHERE space = $1"))
        .bind(space)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| e.to_string())
}

async fn row_by_event(state: &AppState, event: &str) -> Result<Option<Row>, String> {
    sqlx::query_as::<_, Row>(&format!("{ROW} WHERE event = $1"))
        .bind(event)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| e.to_string())
}

/// Stores (or refreshes) a conference's cached facts.
pub async fn save(state: &AppState, row: &Row) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO conferences (space, org, intake, super_admin, event, invite_only, info, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) ON CONFLICT (space) DO UPDATE SET info = excluded.info, \
         event = excluded.event, super_admin = excluded.super_admin, invite_only = excluded.invite_only",
    )
    .bind(&row.space)
    .bind(&row.org)
    .bind(&row.intake)
    .bind(&row.super_admin)
    .bind(&row.event)
    .bind(row.invite_only)
    .bind(&row.info)
    .bind(now_ms())
    .execute(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// The facts a page shows, from an event record's value and the settings.
pub fn info_from_event(event: &Value, theme: Option<&Value>) -> Value {
    let mut info = json!({
        "name": event.get("name").cloned().unwrap_or(json!("")),
        "startsAt": event.get("startsAt").cloned().unwrap_or(Value::Null),
        "endsAt": event.get("endsAt").cloned().unwrap_or(Value::Null),
    });
    for field in ["locations", "description"] {
        if let Some(value) = event.get(field) {
            info[field] = value.clone();
        }
    }
    if let Some(theme) = theme {
        info["theme"] = theme.clone();
    }
    info
}

/// A conference named by its space URI, or by its event's AT-URI (with the
/// super admin's DID or handle).
async fn find(state: &AppState, conference: &str) -> Result<Option<Row>, String> {
    if let Some(space) = SpaceUri::parse(conference) {
        return if space.kind == CONFERENCE_TYPE { row(state, conference).await } else { Ok(None) };
    }
    let Some(rest) = conference.strip_prefix("at://") else { return Ok(None) };
    let mut parts = rest.split('/');
    let (Some(actor), Some(collection), Some(rkey), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Ok(None);
    };
    let did = if actor.starts_with("did:") {
        actor.to_owned()
    } else {
        match state.resolver.resolve_handle(actor).await {
            Ok(did) => did,
            Err(_) => return Ok(None),
        }
    };
    row_by_event(state, &format!("at://{did}/{collection}/{rkey}")).await
}

/// Who's asking, if anyone is signed in (an ended session is no one).
async fn viewer(state: &AppState, headers: &HeaderMap) -> Option<String> {
    match session::lookup(state, headers).await {
        Ok(Lookup::Live(row)) => Some(row.did),
        _ => None,
    }
}

fn not_found() -> Response {
    xrpc_error(StatusCode::NOT_FOUND, "ConferenceNotFound", "No such conference.")
}

fn failed(why: impl std::fmt::Display) -> Response {
    eprintln!("conference: {why}");
    xrpc_error(StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError", "something went wrong")
}

/// A conference and its organization, by row.
async fn load(state: &AppState, row: &Row) -> Result<Option<Org>, String> {
    let org = index::load(state, &row.org).await?;
    Ok(org.filter(|org| org.conference(&row.space).is_some()))
}

/// The page's view of a conference, with the viewer's place in it.
fn view(row: &Row, org: &Org, conference: &Conference, viewer: Option<&str>) -> Value {
    let mut body = row.info();
    body["space"] = json!(row.space);
    body["intake"] = json!(row.intake);
    body["inviteOnly"] = json!(row.invite_only != 0);
    if let Some(event) = &row.event {
        body["event"] = json!(event);
    }
    let methods: Vec<&String> = conference.settings.methods.iter().collect();
    body["join"] = json!({ "methods": methods });
    if let Some(did) = viewer {
        let member = conference.is_member(did);
        let mut place = json!({ "member": member });
        if member && let Some(role) = conference.role_of(org, did) {
            place["role"] = json!(role);
        }
        if !member {
            if conference.pending.contains_key(did) {
                place["request"] = json!("pending");
            } else if conference.denied.contains(did) || conference.banned.contains(did) {
                place["request"] = json!("denied");
            }
        }
        body["viewer"] = place;
    }
    body
}

#[derive(serde::Deserialize)]
pub struct ConferenceParams {
    conference: String,
}

/// `app.eventside.conference.getConference`: a public conference's page for
/// anyone, and an invite-only one's only for its members.
pub async fn get_conference(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ConferenceParams>,
) -> Response {
    let row = match find(&state, &params.conference).await {
        Ok(Some(row)) => row,
        Ok(None) => return not_found(),
        Err(why) => return failed(why),
    };
    let org = match load(&state, &row).await {
        Ok(Some(org)) => org,
        Ok(None) => return not_found(),
        Err(why) => return failed(why),
    };
    let conference = org.conference(&row.space).expect("loaded with its conference");
    let viewer = viewer(&state, &headers).await;
    if row.invite_only != 0 && !viewer.as_deref().is_some_and(|did| conference.is_member(did)) {
        return not_found();
    }
    let mut res = Json(view(&row, &org, conference, viewer.as_deref())).into_response();
    res.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    res
}

/// `app.eventside.conference.listMyConferences`: every conference the
/// signed-in person is a member of.
pub async fn list_my_conferences(State(state): State<AppState>, user: CurrentUser) -> Response {
    let rows = match sqlx::query_as::<_, Row>(&format!("{ROW} ORDER BY created_at"))
        .fetch_all(&state.db)
        .await
    {
        Ok(rows) => rows,
        Err(err) => return failed(err),
    };
    let mut orgs: std::collections::BTreeMap<String, Option<Org>> = Default::default();
    let mut out = Vec::new();
    for row in rows {
        if !orgs.contains_key(&row.org) {
            match index::load(&state, &row.org).await {
                Ok(org) => orgs.insert(row.org.clone(), org),
                Err(why) => return failed(why),
            };
        }
        let Some(Some(org)) = orgs.get(&row.org) else { continue };
        if let Some(conference) = org.conference(&row.space)
            && conference.is_member(&user.did)
        {
            out.push(view(&row, org, conference, Some(&user.did)));
        }
    }
    Json(json!({ "conferences": out })).into_response()
}

#[derive(serde::Deserialize)]
pub struct RecordsParams {
    conference: String,
    collection: String,
}

/// `app.eventside.conference.listRecords`: a collection's records in the
/// conference space, as members are served them. A record counts only if its
/// commit fell inside one of its author's membership periods, and only if the
/// rules let its author write that collection. Role and rules records count
/// only from the conference's super admin.
pub async fn list_records(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(params): Query<RecordsParams>,
) -> Response {
    let row = match find(&state, &params.conference).await {
        Ok(Some(row)) => row,
        Ok(None) => return not_found(),
        Err(why) => return failed(why),
    };
    let org = match load(&state, &row).await {
        Ok(Some(org)) => org,
        Ok(None) => return not_found(),
        Err(why) => return failed(why),
    };
    let conference = org.conference(&row.space).expect("loaded with its conference");
    if !conference.is_member(&user.did) {
        return xrpc_error(
            StatusCode::FORBIDDEN,
            "NotAMember",
            "Only members can see inside this conference.",
        );
    }
    let rows = sqlx::query_as::<_, (String, String, String, Option<String>, String)>(
        "SELECT repo, rkey, rev, cid, value FROM space_records WHERE space = $1 AND collection = $2 AND value IS NOT NULL",
    )
    .bind(&row.space)
    .bind(&params.collection)
    .fetch_all(&state.db)
    .await;
    let rows = match rows {
        Ok(rows) => rows,
        Err(err) => return failed(err),
    };
    let only_super = matches!(params.collection.as_str(), index::ROLE | index::RULES);
    let admins_only = conference.writers_of(&params.collection) == "admins";
    let mut records: Vec<(String, Value)> = rows
        .into_iter()
        .filter_map(|(repo, rkey, rev, cid, value)| {
            let us = tid_micros(&rev)?;
            if !conference.was_member_at(&repo, us) {
                return None;
            }
            if only_super && Some(repo.as_str()) != conference.super_admin() {
                return None;
            }
            if admins_only && !may_post_as_admin(&org, conference, &repo) {
                return None;
            }
            let value: Value = serde_json::from_str(&value).ok()?;
            let record = json!({
                "uri": format!("{}/{repo}/{}/{rkey}", row.space, params.collection),
                "author": repo,
                "cid": cid,
                "value": value,
            });
            Some((rev, record))
        })
        .collect();
    records.sort_by(|a, b| b.0.cmp(&a.0));
    Json(json!({ "records": records.into_iter().map(|(_, r)| r).collect::<Vec<_>>() }))
        .into_response()
}

/// Whether someone may write what the rules keep for owners and staff: an
/// admin of the organization, or someone the super admin made owner or staff.
pub fn may_post_as_admin(org: &Org, conference: &Conference, did: &str) -> bool {
    org.is_admin(did)
        || matches!(conference.roles.get(did).map(String::as_str), Some("owner" | "staff"))
}

#[derive(serde::Deserialize)]
pub struct JoinInput {
    conference: Option<String>,
    code: Option<String>,
    #[serde(default)]
    request: bool,
}

fn answer(status: &str) -> Value {
    json!({ "status": status })
}

/// `app.eventside.conference.join`. In order: a ban refuses; a member is
/// already in; a pre-assigned role or the attendee list (by DID) admits; a
/// code admits, or is refused as invalid; an open conference admits; a list
/// with emails asks for the email step; requests wait; anything else is
/// refused. Admissions and requests are the person's own record in the
/// intake space; nothing is written when they're refused.
pub async fn join(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    user: CurrentUser,
    Json(input): Json<JoinInput>,
) -> Response {
    let limited = !state.host.allow(&format!("join:{}", user.did), 10, 60_000)
        || ip.is_some_and(|ip| {
            !ip.is_loopback() && !state.host.allow(&format!("join:ip:{ip}"), 30, 60_000)
        });
    if limited {
        return xrpc_error(
            StatusCode::TOO_MANY_REQUESTS,
            "RateLimitExceeded",
            "Too many attempts. Wait a minute.",
        );
    }
    let code = input.code.as_deref().map(str::trim).filter(|c| !c.is_empty());
    let code_hash = code.map(|c| state.secrets.code_hmac(c));
    let invalid_code =
        || xrpc_error(StatusCode::BAD_REQUEST, "InvalidCode", "That code isn't valid.");

    let row = match &input.conference {
        Some(conference) => find(&state, conference).await,
        None => match &code_hash {
            Some(hash) => by_code(&state, hash).await,
            None => {
                return xrpc_error(
                    StatusCode::BAD_REQUEST,
                    "InvalidRequest",
                    "Name a conference or a code.",
                );
            }
        },
    };
    let row = match row {
        Ok(Some(row)) => row,
        Ok(None) if input.conference.is_none() => return invalid_code(),
        Ok(None) => return not_found(),
        Err(why) => return failed(why),
    };
    let org = match load(&state, &row).await {
        Ok(Some(org)) => org,
        Ok(None) => return not_found(),
        Err(why) => return failed(why),
    };
    let conference = org.conference(&row.space).expect("loaded with its conference");
    let did = user.did.as_str();
    let joined = |org: &Org, conference: &Conference| {
        let mut out = answer("joined");
        out["conference"] = json!(row.space);
        if let Some(role) = conference.role_of(org, did) {
            out["role"] = json!(role);
        }
        Json(out).into_response()
    };

    if conference.banned.contains(did) {
        return Json(answer("refused")).into_response();
    }
    if conference.is_member(did) {
        return joined(&org, conference);
    }
    let now = tid_micros(&tid_now()).unwrap_or_default();
    let settings = &conference.settings;
    let admission = match conference.would_admit(did, code_hash.as_deref(), now) {
        Some(Via::Code) => Some(Via::Code),
        // A code that doesn't admit is refused, whatever else is on, unless
        // the person is let in by their role or the list anyway.
        _ if code.is_some()
            && !matches!(conference.would_admit(did, None, now), Some(Via::Role | Via::List)) =>
        {
            return invalid_code();
        }
        other => other,
    };
    let write = match admission {
        Some(_) => true,
        None => {
            // A handle on the list that didn't resolve when it was imported,
            // which names this person now.
            if settings.has("list") && unresolved_listed(&state, conference, did).await {
                return match admit_by_super_admin(&state, &org, &row.space, did, "list").await {
                    Ok(()) => reload_answer(&state, &row, did).await,
                    Err(why) => failed(why),
                };
            }
            if settings.has("list") && conference.has_email_rows() && !input.request {
                let mut out = answer("emailNeeded");
                out["canRequest"] = json!(settings.has("request"));
                out["conference"] = json!(row.space);
                out["verifyUrl"] = json!(format!(
                    "/oauth/email?{}",
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("conference", &row.space)
                        .finish()
                ));
                return Json(out).into_response();
            }
            settings.has("request")
        }
    };
    if !write {
        return Json(answer("refused")).into_response();
    }
    let mut record = json!({});
    if admission == Some(Via::Code)
        && let Some(code) = code
    {
        record["code"] = json!(code);
    }
    if let Err(res) = write_intake(&state, &user, &row.intake, JOIN, record).await {
        return res;
    }
    reload_answer(&state, &row, did).await
}

/// The answer after a join was written: what the index now makes of it.
async fn reload_answer(state: &AppState, row: &Row, did: &str) -> Response {
    let org = match load(state, row).await {
        Ok(Some(org)) => org,
        Ok(None) => return not_found(),
        Err(why) => return failed(why),
    };
    let conference = org.conference(&row.space).expect("loaded with its conference");
    let mut out = if conference.is_member(did) {
        let mut out = answer("joined");
        if let Some(role) = conference.role_of(&org, did) {
            out["role"] = json!(role);
        }
        out
    } else if conference.pending.contains_key(did) {
        answer("pending")
    } else {
        answer("refused")
    };
    out["conference"] = json!(row.space);
    Json(out).into_response()
}

/// Writes the person's own join or leave record into the intake space, and
/// reads it into the index.
async fn write_intake(
    state: &AppState,
    user: &CurrentUser,
    intake: &str,
    collection: &str,
    mut value: Value,
) -> Result<(), Response> {
    let client = user.pds_client(state).await.map_err(|e| {
        xrpc_error(
            StatusCode::BAD_GATEWAY,
            "UpstreamFailure",
            &format!("Your PDS couldn't be reached: {e:?}"),
        )
    })?;
    value["$type"] = json!(collection);
    value["createdAt"] = json!(index::iso(now_ms() as u64 * 1000));
    let body = json!({
        "space": intake,
        "repo": user.did,
        "collection": collection,
        "rkey": tid_now(),
        "record": value,
    });
    let res = client
        .send(Method::POST, "/xrpc/com.atproto.space.createRecord", Some(&body))
        .await
        .map_err(|e| xrpc_error(StatusCode::BAD_GATEWAY, "UpstreamFailure", &format!("{e:?}")))?;
    if !res.status().is_success() {
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        eprintln!(
            "conference: {}'s PDS refused their {collection} record ({status}): {text}",
            user.did
        );
        return Err(xrpc_error(
            StatusCode::BAD_GATEWAY,
            "UpstreamFailure",
            "Your PDS didn't take the record.",
        ));
    }
    sync::sync_repo(state, intake, &user.did).await.map_err(failed)?;
    Ok(())
}

/// The conference a code belongs to.
async fn by_code(state: &AppState, code_hash: &str) -> Result<Option<Row>, String> {
    for authority in authority::all(&state.db).await? {
        let Some(org) = index::load(state, &authority.did).await? else { continue };
        if let Some(conference) = org.conferences.values().find(|c| c.has_code(code_hash)) {
            return row(state, conference.space()).await;
        }
    }
    Ok(None)
}

/// Whether a list entry with only a handle (it didn't resolve on import)
/// resolves to this person now.
async fn unresolved_listed(state: &AppState, conference: &Conference, did: &str) -> bool {
    for handle in
        conference.list.iter().filter(|e| e.did.is_none()).filter_map(|e| e.handle.as_deref())
    {
        if state.resolver.resolve_handle(handle).await.is_ok_and(|resolved| resolved == did) {
            return true;
        }
    }
    false
}

/// Admits someone with a `member` record written as the super admin: for the
/// admissions that can't be derived from records (a verified email, a list
/// handle that resolved only at join time).
async fn admit_by_super_admin(
    state: &AppState,
    org: &Org,
    space: &str,
    did: &str,
    via: &str,
) -> Result<(), String> {
    let handle = match state.resolver.resolve_did(&org.super_admin).await {
        Ok(identity) => identity.handle,
        Err(_) => org.super_admin.clone(),
    };
    let acting = admin::Acting::new(state, &org.super_admin, &handle).await?;
    acting
        .create_in(
            state,
            &SpaceUri::admin(&org.did).to_string(),
            index::MEMBER,
            None,
            json!({ "space": space, "subject": did, "via": via, "since": index::iso(now_ms() as u64 * 1000) }),
        )
        .await?;
    Ok(())
}

/// `app.eventside.conference.leave`: the person's own leave record. Their
/// credentials for the space are revoked.
pub async fn leave(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(input): Json<Value>,
) -> Response {
    let Some(conference) = input.get("conference").and_then(Value::as_str) else {
        return xrpc_error(StatusCode::BAD_REQUEST, "InvalidRequest", "Name a conference.");
    };
    let row = match find(&state, conference).await {
        Ok(Some(row)) => row,
        Ok(None) => return not_found(),
        Err(why) => return failed(why),
    };
    let org = match load(&state, &row).await {
        Ok(Some(org)) => org,
        Ok(None) => return not_found(),
        Err(why) => return failed(why),
    };
    if !org.conference(&row.space).is_some_and(|c| c.is_member(&user.did)) {
        return xrpc_error(
            StatusCode::BAD_REQUEST,
            "NotAMember",
            "You aren't a member of this conference.",
        );
    }
    if let Err(res) = write_intake(&state, &user, &row.intake, LEAVE, json!({})).await {
        return res;
    }
    let background = state.clone();
    let (space, did) = (row.space.clone(), user.did.clone());
    tokio::spawn(async move {
        if let Err(why) = notify::revoke(&background, &space, &did).await {
            eprintln!("leave: couldn't revoke {did}'s credentials: {why}");
        }
    });
    Json(json!({})).into_response()
}

/// The email step's end: the person's PDS says their verified email, which
/// is matched (as an HMAC) against the conference's list. A match admits
/// them, as a `member` record from the super admin. The email isn't kept.
pub async fn email_step(state: &AppState, id_hash: &str, space: &str) {
    if let Err(why) = try_email_step(state, id_hash, space).await {
        eprintln!("email step: {why}");
    }
}

async fn try_email_step(state: &AppState, id_hash: &str, space: &str) -> Result<(), String> {
    let row = row(state, space).await?.ok_or("no such conference")?;
    let org = load(state, &row).await?.ok_or("no such conference")?;
    let conference = org.conference(space).ok_or("no such conference")?;
    let session =
        session::load(&state.db, id_hash).await.map_err(|e| e.to_string())?.ok_or("no session")?;
    if conference.is_member(&session.did) || conference.banned.contains(&session.did) {
        return Ok(());
    }
    let client = crate::auth::pds::PdsClient::for_session(state, id_hash)
        .await
        .map_err(|e| format!("{e:?}"))?;
    let res = client
        .send(Method::GET, "/xrpc/com.atproto.server.getSession", None)
        .await
        .map_err(|e| format!("{e:?}"))?;
    if !res.status().is_success() {
        return Err(format!("getSession answered {}", res.status()));
    }
    let body: Value = res.json().await.map_err(|e| e.to_string())?;
    if body.get("emailConfirmed").and_then(Value::as_bool) != Some(true) {
        return Ok(());
    }
    let Some(email) = body.get("email").and_then(Value::as_str) else { return Ok(()) };
    let hmac = state.secrets.email_hmac(email);
    if !conference.settings.has("list")
        || !conference.list.iter().any(|e| e.email_hmac.as_deref() == Some(&hmac))
    {
        return Ok(());
    }
    admit_by_super_admin(state, &org, space, &session.did, "email").await
}

/// Who may read each of an organization's spaces now, so the ones who lose
/// access can have their credentials revoked.
pub fn readers(org: &Org) -> Vec<(String, std::collections::BTreeSet<String>)> {
    org.spaces()
        .into_iter()
        .map(|space| {
            let readers = org.readers(&space);
            (space, readers)
        })
        .collect()
}

/// Revokes the credentials of everyone who could read a space before and
/// can't now.
pub async fn revoke_lost(
    state: &AppState,
    before: &[(String, std::collections::BTreeSet<String>)],
    after: &Org,
) {
    for (space, readers) in before {
        let now = after.readers(space);
        for did in readers.difference(&now) {
            if let Err(why) = notify::revoke(state, space, did).await {
                eprintln!("warning: couldn't revoke {did}'s credentials for {space}: {why}");
            }
        }
    }
}
