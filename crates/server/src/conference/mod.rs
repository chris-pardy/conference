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
pub mod rules;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

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
use crate::spacehost::index::{self, AppAccess, Conference, JOIN, LEAVE, Org, Via};
use crate::spacehost::{CONFERENCE_TYPE, ClientIp, SpaceUri, notify, sync};

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
async fn load(state: &AppState, row: &Row) -> Result<Option<Arc<Org>>, String> {
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
    let mut orgs: BTreeMap<String, Option<Arc<Org>>> = Default::default();
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
    limit: Option<String>,
    cursor: Option<String>,
}

/// How many records a page of `listRecords` has, unless asked for fewer.
const RECORDS_PAGE: i64 = 100;
/// The most records a page of `listRecords` has.
const RECORDS_PAGE_MAX: i64 = 500;

/// A record's place in `listRecords`'s order, newest first: when it counts
/// from, then its revision, author and key, so no two share one.
type Place = (u64, String, String, String);

fn place_cursor((us, rev, repo, rkey): &Place) -> String {
    format!("{us} {rev} {repo} {rkey}")
}

fn parse_place(cursor: &str) -> Option<Place> {
    let mut parts = cursor.split(' ');
    let (Some(us), Some(rev), Some(repo), Some(rkey), None) =
        (parts.next(), parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    Some((us.parse().ok()?, rev.to_owned(), repo.to_owned(), rkey.to_owned()))
}

/// One page of items in their places: those after the cursor, newest first,
/// at most `limit`, and the cursor for the next page if there's more.
fn page_of<T>(
    mut items: Vec<(Place, T)>,
    after: Option<&Place>,
    limit: usize,
) -> (Vec<T>, Option<String>) {
    items.retain(|(place, _)| after.is_none_or(|after| place < after));
    items.sort_by(|a, b| b.0.cmp(&a.0));
    let more = items.len() > limit;
    items.truncate(limit);
    let cursor = more.then(|| items.last().map(|(place, _)| place_cursor(place))).flatten();
    (items.into_iter().map(|(_, item)| item).collect(), cursor)
}

/// `app.eventside.conference.listRecords`: a collection's records in the
/// conference space, as members are served them. A record counts only if its
/// commit fell inside one of its author's membership periods, and only if the
/// rules let its author write that collection. Role and rules records count
/// only from the conference's super admin. Paged by `limit` and `cursor`.
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
        // An invite-only conference doesn't exist for anyone outside it.
        if row.invite_only != 0 {
            return not_found();
        }
        return xrpc_error(
            StatusCode::FORBIDDEN,
            "NotAMember",
            "Only members can see inside this conference.",
        );
    }
    // The paging is checked only for members, so a bad cursor or limit says
    // nothing about a conference that's hidden from the asker.
    let after = match params.cursor.as_deref().map(parse_place) {
        Some(None) => return xrpc_error(StatusCode::BAD_REQUEST, "InvalidRequest", "Bad cursor."),
        Some(Some(place)) => Some(place),
        None => None,
    };
    let Some(limit) =
        crate::spacehost::page_limit(params.limit.as_deref(), RECORDS_PAGE, RECORDS_PAGE_MAX)
    else {
        return crate::spacehost::bad_limit();
    };
    let rows = sqlx::query_as::<_, (String, String, String, Option<String>, String, Option<i64>)>(
        "SELECT r.repo, r.rkey, r.rev, r.cid, r.value, s.seen_at FROM space_records r \
         LEFT JOIN space_record_seen s ON s.space = r.space AND s.repo = r.repo \
         AND s.collection = r.collection AND s.rkey = r.rkey AND s.rev = r.rev \
         WHERE r.space = $1 AND r.collection = $2 AND r.value IS NOT NULL",
    )
    .bind(&row.space)
    .bind(&params.collection)
    .fetch_all(&state.db)
    .await;
    let rows = match rows {
        Ok(rows) => rows,
        Err(err) => return failed(err),
    };
    let counted: Vec<(Place, (Option<String>, String))> = rows
        .into_iter()
        .filter_map(|(repo, rkey, rev, cid, value, seen_at)| {
            let seen_us = seen_at.map(|ms| ms.max(0) as u64 * 1000);
            let us = rules::record_counts(
                &org,
                conference,
                &repo,
                &params.collection,
                tid_micros(&rev)?,
                seen_us,
            )?;
            Some(((us, rev, repo, rkey), (cid, value)))
        })
        .collect();
    // Newest first, by when each counts from (not the time its PDS claims).
    // Only the page's values are parsed.
    let (page, cursor) = page_of(
        counted.into_iter().map(|(place, item)| (place.clone(), (place, item))).collect(),
        after.as_ref(),
        limit,
    );
    let records: Vec<Value> = page
        .into_iter()
        .filter_map(|((_, _, repo, rkey), (cid, value))| {
            let value: Value = serde_json::from_str(&value).ok()?;
            Some(json!({
                "uri": format!("{}/{repo}/{}/{rkey}", row.space, params.collection),
                "author": repo,
                "cid": cid,
                "value": value,
            }))
        })
        .collect();
    let mut out = json!({ "records": records });
    if let Some(cursor) = cursor {
        out["cursor"] = json!(cursor);
    }
    Json(out).into_response()
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
        || ip.is_some_and(|ip| !state.host.allow_ip(&state, "join", ip, 30));
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
    let mut org = match load(&state, &row).await {
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
    let now = tid_micros(&tid_now()).unwrap_or_default();
    let settings = &conference.settings;
    // A handle on the list that didn't resolve when it was imported, which
    // names this person now: bound to them from now on, with its role,
    // however else they were or would be let in (a role included, so the
    // row can't later admit the handle's next holder). Then a join like any
    // other on the list. Binding is written as the super admin: if their PDS
    // fails and something else lets the person in, they're let in anyway and
    // the binding is tried again on a later join.
    if settings.has("list")
        && !conference.on_list(did)
        && let Some(entry) = unbound_listed(&state, conference, did).await
    {
        match bind_list_entry(&state, &org, conference, did, entry).await {
            Ok(()) => {
                if !conference.is_member(did)
                    && let Err(res) =
                        write_intake(&state, &user, &row.intake, JOIN, json!({})).await
                {
                    return res;
                }
                return reload_answer(&state, &row, did).await;
            }
            Err(why) => {
                // The row's role may have been given though its binding
                // wasn't: judged by the index as it is now.
                org = match load(&state, &row).await {
                    Ok(Some(org)) => org,
                    Ok(None) => return not_found(),
                    Err(err) => return failed(err),
                };
                let conference = org.conference(&row.space).expect("loaded with its conference");
                let otherwise = conference.is_member(did)
                    || conference.would_admit(did, code_hash.as_deref(), now).is_some();
                if !otherwise {
                    return failed(why);
                }
                eprintln!(
                    "conference: {did}'s list handle isn't bound yet (tried again on a later \
                     join unless its role was given): {why}"
                );
            }
        }
    }
    let conference = org.conference(&row.space).expect("loaded with its conference");
    let settings = &conference.settings;
    if conference.is_member(did) {
        return joined(&org, conference);
    }
    // Named by its address, an invite-only conference that wouldn't let this
    // person in answers as if it didn't exist, as its page does.
    let hidden = row.invite_only != 0 && input.conference.is_some();
    let admission = match conference.would_admit(did, code_hash.as_deref(), now) {
        Some(Via::Code) => Some(Via::Code),
        // A code that doesn't admit is refused, whatever else is on, unless
        // the person is let in by their role or the list anyway.
        _ if code.is_some()
            && !matches!(conference.would_admit(did, None, now), Some(Via::Role | Via::List)) =>
        {
            return if hidden { not_found() } else { invalid_code() };
        }
        other => other,
    };
    let write = match admission {
        Some(_) => true,
        None => {
            if hidden {
                return not_found();
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

/// The conference a code belongs to: only the organizations with a code
/// record by that HMAC in their admin space are loaded, so a wrong code loads
/// none.
async fn by_code(state: &AppState, code_hash: &str) -> Result<Option<Row>, String> {
    match code_conference(state, code_hash).await? {
        Some(space) => row(state, &space).await,
        None => Ok(None),
    }
}

/// The space of the conference whose code (by HMAC) this is, if any.
pub async fn code_conference(state: &AppState, code_hash: &str) -> Result<Option<String>, String> {
    let spaces = sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT space FROM space_records WHERE code_hmac = $1 AND collection = $2 \
         AND space LIKE $3 AND value IS NOT NULL",
    )
    .bind(code_hash)
    .bind(index::CODE)
    .bind(format!("at://%/space/{}/self", crate::spacehost::ADMIN_TYPE))
    .fetch_all(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    let orgs: BTreeSet<String> = spaces
        .iter()
        .filter_map(|s| SpaceUri::parse(s))
        .filter(|s| s.kind == crate::spacehost::ADMIN_TYPE)
        .map(|s| s.authority)
        .collect();
    for org in orgs {
        let Some(org) = index::load(state, &org).await? else { continue };
        if let Some(conference) = org.conferences.values().find(|c| c.has_code(code_hash)) {
            return Ok(Some(conference.space().to_owned()));
        }
    }
    Ok(None)
}

/// The list entry with only a handle (it didn't resolve on import, and
/// hasn't been bound since) that names this person now: their own handle,
/// resolved both ways. Two lookups, however long the list.
async fn unbound_listed<'a>(
    state: &AppState,
    conference: &'a Conference,
    did: &str,
) -> Option<&'a index::ListEntry> {
    let mut unbound = conference.unbound_handles().peekable();
    unbound.peek()?;
    let handle = state.resolver.resolve_did(did).await.ok()?.handle;
    let handle = crate::identity::normalize_handle(&handle)?;
    let entry = unbound.find(|e| e.handle.as_deref() == Some(handle.as_str()))?;
    let resolved = state.resolver.resolve_handle(&handle).await.ok()?;
    (resolved == did).then_some(entry)
}

/// Binds a list entry that had only a handle to the DID it first resolved
/// to: an entry with both, written as the super admin on behalf of the owner
/// who imported the handle, so it counts only while they're an owner, and a
/// handle that later changes hands doesn't take the place with it.
///
/// The role the row gives is written first (as the conference's super admin,
/// who may be someone else), and the binding last: once someone is on the
/// list by DID the binding isn't tried again, so a role that failed after it
/// would never be given.
async fn bind_list_entry(
    state: &AppState,
    org: &Org,
    conference: &Conference,
    did: &str,
    entry: &index::ListEntry,
) -> Result<(), String> {
    give_list_role(state, org, conference, did, entry).await?;
    write_binding(state, org, conference.space(), did, entry).await
}

/// Writes the binding of a handle-only row that `did` holds only by their
/// list role's claim, before that role is changed or taken away: the claim
/// lives on the role record, and the row must stay theirs without it, or
/// it would be open again to the handle's next holder.
pub async fn keep_list_claim(
    state: &AppState,
    org: &Org,
    conference: &Conference,
    did: &str,
) -> Result<(), String> {
    match conference.claimed_row(did) {
        Some(entry) => write_binding(state, org, conference.space(), did, entry).await,
        None => Ok(()),
    }
}

/// Writes `did`'s role record in a conference, as `writer` (its super
/// admin). Every role a command or a join writes goes through here, so a
/// list row `did` holds only by their current role's claim stays theirs:
/// a list role assigned by the row's own importer carries the claim
/// (`listHandle`) on, and any other role (another owner's list role
/// included, which goes when they stop being an admin) is written only
/// after the row is bound, so it no longer depends on a role.
pub async fn put_role(
    state: &AppState,
    org: &Org,
    conference: &Conference,
    writer: &admin::Acting,
    did: &str,
    mut value: Value,
) -> Result<(), String> {
    if let Some(entry) = conference.claimed_row(did) {
        let str_of = |key: &str| value.get(key).and_then(Value::as_str);
        if str_of("via") == Some("list") && str_of("assignedBy") == Some(entry.by.as_str()) {
            value["listHandle"] = json!(entry.handle);
        } else {
            write_binding(state, org, conference.space(), did, entry).await?;
        }
    }
    writer.put_in(state, conference.space(), index::ROLE, did, value).await
}

/// The binding itself: an entry with the row's handle and the DID, written
/// as the super admin on behalf of the owner who imported the handle.
async fn write_binding(
    state: &AppState,
    org: &Org,
    space: &str,
    did: &str,
    entry: &index::ListEntry,
) -> Result<(), String> {
    let handle = match state.resolver.resolve_did(&org.super_admin).await {
        Ok(identity) => identity.handle,
        Err(_) => org.super_admin.clone(),
    };
    let acting = admin::Acting::new(state, &org.super_admin, &handle).await?;
    let mut bound = json!({
        "space": space,
        "did": did,
        "handle": entry.handle,
        "onBehalfOf": entry.by,
    });
    if let Some(role) = &entry.role {
        bound["role"] = json!(role);
    }
    acting
        .create_in(state, &SpaceUri::admin(&org.did).to_string(), index::LIST_ENTRY, None, bound)
        .await?;
    Ok(())
}

/// The role a list row gives, for someone it matched only at join time (a
/// handle that resolved then, or a verified email): a role record like an
/// import's, written as the conference's super admin and assigned by the
/// owner who imported the row, so it counts only while they're an owner and
/// the list is on.
async fn give_list_role(
    state: &AppState,
    org: &Org,
    conference: &Conference,
    did: &str,
    entry: &index::ListEntry,
) -> Result<(), String> {
    let Some(role) = &entry.role else { return Ok(()) };
    if conference.roles.contains_key(did) {
        return Ok(());
    }
    let mut value = json!({ "subject": did, "role": role, "assignedBy": entry.by, "via": "list",
                            "since": index::iso(now_ms() as u64 * 1000) });
    // A handle-only row's role names the row, so it's this person's from
    // now on even if its binding isn't written.
    if entry.did.is_none()
        && let Some(handle) = &entry.handle
    {
        value["listHandle"] = json!(handle);
    }
    let writer = conference.super_admin();
    let handle = match state.resolver.resolve_did(writer).await {
        Ok(identity) => identity.handle,
        Err(_) => writer.to_owned(),
    };
    let acting = admin::Acting::new(state, writer, &handle).await?;
    put_role(state, org, conference, &acting, did, value).await
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
        if row.invite_only != 0 {
            return not_found();
        }
        return xrpc_error(
            StatusCode::BAD_REQUEST,
            "NotAMember",
            "You aren't a member of this conference.",
        );
    }
    // Admins are members because they're admins: leaving wouldn't end that,
    // only cut off their apps.
    if org.is_admin(&user.did) {
        return xrpc_error(
            StatusCode::BAD_REQUEST,
            "AdminCannotLeave",
            "Admins are members of every conference of theirs. To leave, stop being an admin.",
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
    let matched: Vec<&index::ListEntry> =
        conference.list.iter().filter(|e| e.email_hmac.as_deref() == Some(&hmac)).collect();
    if !conference.settings.has("list") || matched.is_empty() {
        return Ok(());
    }
    // The role its row gives, before the admission, so they join with it.
    if let Some(entry) = matched.iter().find(|e| e.role.is_some()) {
        give_list_role(state, &org, conference, &session.did, entry).await?;
    }
    admit_by_super_admin(state, &org, space, &session.did, "email").await
}

/// Who may read each of an organization's spaces now, and with which apps,
/// so whoever loses access (a person, or an app) can have their credentials
/// revoked.
pub struct Access {
    readers: Vec<(String, BTreeSet<String>)>,
    apps: Vec<(String, AppAccess)>,
}

pub fn access(org: &Org) -> Access {
    let spaces = org.spaces();
    Access {
        readers: spaces.iter().map(|space| (space.clone(), org.readers(space))).collect(),
        apps: spaces
            .iter()
            .filter_map(|space| Some((space.clone(), org.app_access(&SpaceUri::parse(space)?))))
            .collect(),
    }
}

/// Revokes the credentials of everyone who could read a space before and
/// can't now, and of every app that could and can't.
pub async fn revoke_lost(state: &AppState, before: &Access, after: &Org) {
    for (space, readers) in &before.readers {
        let now = after.readers(space);
        for did in readers.difference(&now) {
            if let Err(why) = notify::revoke(state, space, did).await {
                eprintln!("warning: couldn't revoke {did}'s credentials for {space}: {why}");
            }
        }
    }
    for (space, apps) in &before.apps {
        let Some(parsed) = SpaceUri::parse(space) else { continue };
        let now = after.app_access(&parsed);
        if now == *apps {
            continue;
        }
        let eventside = state.oauth.public_url.clone();
        let lost = move |client: Option<&str>| !now.allows(client, &eventside);
        if let Err(why) = notify::revoke_apps(state, space, lost).await {
            eprintln!("warning: couldn't revoke apps' credentials for {space}: {why}");
        }
    }
}

/// Rebuilds the cached facts of an organization's conferences (the
/// `conferences` table) from their records: the super admin's settings, and
/// the event and sidecar, public or inside the space. A conference whose
/// event can't be read keeps the facts it had; one that had none is
/// reported.
pub async fn rebuild_rows(state: &AppState, org: &Org) -> Vec<String> {
    let mut missing = Vec::new();
    for conference in org.conferences.values() {
        let settings = &conference.settings;
        let Some(intake) = settings.intake.clone() else { continue };
        let super_admin = conference.super_admin().to_owned();
        let facts = match &settings.event {
            Some(event) => public_facts(state, event).await,
            None => inside_facts(state, conference.space(), &super_admin).await,
        };
        let existing = row(state, conference.space()).await.ok().flatten();
        let (info, event) = match (facts, existing) {
            (Ok((info, event)), _) => (info.to_string(), event),
            (Err(_), Some(existing)) => (existing.info, existing.event),
            (Err(why), None) => {
                missing.push(format!("{}: {why}", conference.space()));
                continue;
            }
        };
        let saved = save(
            state,
            &Row {
                space: conference.space().to_owned(),
                org: org.did.clone(),
                intake,
                super_admin,
                event: if settings.invite_only { None } else { event },
                invite_only: i64::from(settings.invite_only),
                info,
            },
        )
        .await;
        if let Err(why) = saved {
            missing.push(format!("{}: {why}", conference.space()));
        }
    }
    missing
}

/// A public conference's facts: its event, and its sidecar's theme, from the
/// super admin's PDS.
async fn public_facts(state: &AppState, event: &str) -> Result<(Value, Option<String>), String> {
    let value = public_record(state, event).await?;
    let sidecar = match event.rsplit_once('/') {
        Some((_, rkey)) => {
            let repo = event.trim_start_matches("at://").split('/').next().unwrap_or_default();
            public_record(state, &format!("at://{repo}/{SIDECAR}/{rkey}")).await.ok()
        }
        None => None,
    };
    let theme = sidecar.as_ref().and_then(|s| s.get("theme"));
    Ok((info_from_event(&value, theme), Some(event.to_owned())))
}

/// An invite-only conference's facts: its event and sidecar inside the
/// space, from the index.
async fn inside_facts(
    state: &AppState,
    space: &str,
    super_admin: &str,
) -> Result<(Value, Option<String>), String> {
    let rows = sqlx::query_as::<_, (String, String, String)>(
        "SELECT collection, rkey, value FROM space_records WHERE space = $1 AND repo = $2 \
         AND collection IN ($3, $4) AND value IS NOT NULL",
    )
    .bind(space)
    .bind(super_admin)
    .bind(EVENT)
    .bind(SIDECAR)
    .fetch_all(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    let parse = |v: &str| serde_json::from_str::<Value>(v).ok();
    let sidecar =
        rows.iter().find(|(c, k, _)| c == SIDECAR && k == "self").and_then(|(_, _, v)| parse(v));
    let named = sidecar.as_ref().and_then(|s| s.get("event")).and_then(Value::as_str);
    let event = rows
        .iter()
        .filter(|(c, _, _)| c == EVENT)
        .find(|(_, rkey, _)| named.is_none_or(|uri| uri.ends_with(&format!("/{rkey}"))))
        .and_then(|(_, _, v)| parse(v))
        .ok_or("its event isn't in the space")?;
    let theme = sidecar.as_ref().and_then(|s| s.get("theme"));
    Ok((info_from_event(&event, theme), None))
}

/// A public record's value, from its repo's PDS.
pub async fn public_record(state: &AppState, uri: &str) -> Result<Value, String> {
    let rest = uri.strip_prefix("at://").ok_or_else(|| format!("{uri} isn't an AT-URI"))?;
    let [repo, collection, rkey] = rest.split('/').collect::<Vec<_>>()[..] else {
        return Err(format!("{uri} isn't a record's AT-URI"));
    };
    let pds = state.host.pds_of(state, repo).await?;
    let url = format!("{pds}/xrpc/com.atproto.repo.getRecord");
    let res = state
        .http
        .guarded(&url)?
        .get(&url)
        .query(&[("repo", repo), ("collection", collection), ("rkey", rkey)])
        .send()
        .await
        .map_err(|e| format!("{url}: {e}"))?;
    if !res.status().is_success() {
        return Err(format!("couldn't read {uri}: {}", res.status()));
    }
    let record: Value = crate::net::read_json(res).await?;
    record.get("value").cloned().ok_or_else(|| format!("{uri} has no value"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_are_paged_newest_first_by_a_cursor() {
        let place =
            |us: u64, rkey: &str| (us, "3rev".to_owned(), "did:plc:a".to_owned(), rkey.to_owned());
        let items: Vec<(Place, u64)> = [(5, "e"), (1, "a"), (3, "c"), (3, "d"), (2, "b")]
            .map(|(us, k)| (place(us, k), us))
            .to_vec();
        let (first, cursor) = page_of(items.clone(), None, 2);
        assert_eq!(first, [5, 3]);
        let cursor = cursor.expect("there's more");
        let after = parse_place(&cursor).expect("a cursor parses");
        assert_eq!(after, place(3, "d"));
        let (second, cursor) = page_of(items.clone(), Some(&after), 2);
        assert_eq!(second, [3, 2], "the other record of the same time isn't skipped");
        let (third, cursor) = page_of(items, Some(&parse_place(&cursor.unwrap()).unwrap()), 2);
        assert_eq!(third, [1]);
        assert_eq!(cursor, None, "the last page");
        assert_eq!(parse_place("12 3rev did:plc:a"), None);
    }
}
