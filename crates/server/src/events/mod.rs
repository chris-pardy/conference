//! Events, their feeds, and the cards in those feeds. Everything is stored
//! here; atproto is only how people sign in. See DEMO.md.
//!
//! - An event is a group of people with event details. Anyone can see a
//!   top-level event and RSVP; going opens its feeds, people and plans.
//! - A sub-event is an event made inside another. Only people going to the
//!   parent can see it.
//! - A card is redux: `(state, action) -> (state', actions[])`, run by its
//!   wasm module here (`env: "server"`, the shared state) and in each
//!   browser (`env: "client"`, local state). Clients dispatch actions up;
//!   the event's stream sends cards and server-emitted actions down.

// Handlers answer with a Response as their error, and rows come as tuples.
#![allow(clippy::result_large_err, clippy::type_complexity)]

pub mod runtime;
pub mod seed;

use std::collections::HashMap;
use std::convert::Infallible;

use axum::Json;
use axum::extract::{Query, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rand_core::{OsRng, RngCore};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use tokio::sync::broadcast;

use crate::AppState;
use crate::auth::{CurrentUser, xrpc_error};
use crate::db::{Db, now_ms};
use crate::keys::random_token;

/// Something in an event changed. Each stream turns it into a payload for its viewer.
#[derive(Debug, Clone)]
pub enum Change {
    Card {
        event_id: String,
        feed_id: String,
        card_id: String,
    },
    CardRemoved {
        event_id: String,
        feed_id: String,
        card_id: String,
    },
    Feed {
        event_id: String,
        feed_id: String,
    },
    /// Actions a card's server reducer sent down to clients.
    Actions {
        event_id: String,
        card_id: String,
        actions: Vec<Value>,
    },
    /// An event's details or RSVPs changed, or it was created.
    Event {
        event_id: String,
        parent_id: Option<String>,
    },
}

pub fn channel() -> broadcast::Sender<Change> {
    broadcast::channel(1024).0
}

type Result<T> = std::result::Result<T, Response>;

fn internal(err: sqlx::Error) -> Response {
    eprintln!("events: {err}");
    xrpc_error(StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError", "Something went wrong.")
}

fn bad(message: &str) -> Response {
    xrpc_error(StatusCode::BAD_REQUEST, "InvalidRequest", message)
}

fn not_found() -> Response {
    xrpc_error(StatusCode::NOT_FOUND, "NotFound", "Not found.")
}

fn not_going() -> Response {
    xrpc_error(StatusCode::FORBIDDEN, "NotGoing", "Say you're going to see this.")
}

fn module_error(message: String) -> Response {
    eprintln!("card module: {message}");
    xrpc_error(
        StatusCode::UNPROCESSABLE_ENTITY,
        "ModuleError",
        &format!("The card's module failed: {message}"),
    )
}

fn new_id() -> String {
    random_token(9)
}

fn no_store(value: Value) -> Response {
    let mut res = Json(value).into_response();
    res.headers_mut().insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

#[derive(Deserialize)]
pub struct IdQuery {
    id: String,
}

/// Records who the person is, for person blocks and action metadata.
async fn arrive(db: &Db, user: &CurrentUser) -> Result<()> {
    sqlx::query(
        "INSERT INTO profiles (did, handle, display_name, avatar, updated_at) VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (did) DO UPDATE SET handle = excluded.handle, display_name = excluded.display_name, \
         avatar = excluded.avatar, updated_at = excluded.updated_at",
    )
    .bind(&user.did)
    .bind(&user.handle)
    .bind(user.display_name())
    .bind(user.avatar())
    .bind(now_ms())
    .execute(db)
    .await
    .map_err(internal)?;
    Ok(())
}

fn actor(user: &CurrentUser) -> Value {
    json!({ "did": user.did, "handle": user.handle, "displayName": user.display_name() })
}

// Events and who can see them

#[derive(Debug, Clone)]
pub struct EventRow {
    pub id: String,
    pub parent_id: Option<String>,
    pub title: String,
    pub description: String,
    pub starts_at: i64,
    pub ends_at: Option<i64>,
    pub time_zone: String,
    pub location: String,
    pub host: String,
}

type EventTuple =
    (String, Option<String>, String, String, i64, Option<i64>, String, String, String);
const EVENT_COLUMNS: &str =
    "id, parent_id, title, description, starts_at, ends_at, time_zone, location, host_did";

impl From<EventTuple> for EventRow {
    fn from(t: EventTuple) -> Self {
        let (id, parent_id, title, description, starts_at, ends_at, time_zone, location, host) = t;
        Self { id, parent_id, title, description, starts_at, ends_at, time_zone, location, host }
    }
}

async fn load_event(db: &Db, id: &str) -> Result<Option<EventRow>> {
    let row: Option<EventTuple> =
        sqlx::query_as(&format!("SELECT {EVENT_COLUMNS} FROM events WHERE id = $1"))
            .bind(id)
            .fetch_optional(db)
            .await
            .map_err(internal)?;
    Ok(row.map(EventRow::from))
}

async fn rsvp_of(db: &Db, event_id: &str, did: &str) -> Result<Option<String>> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT status FROM rsvps WHERE event_id = $1 AND did = $2")
            .bind(event_id)
            .bind(did)
            .fetch_optional(db)
            .await
            .map_err(internal)?;
    Ok(row.map(|r| r.0))
}

/// What one person can do with one event.
pub struct Access {
    pub event: EventRow,
    pub rsvp: Option<String>,
    /// Whether they can see it at all: it's top-level, or they're going to
    /// every event it's inside.
    pub can_see: bool,
}

impl Access {
    /// Whether they can see inside it: they can see it, and they're going.
    pub fn can_view(&self) -> bool {
        self.can_see && self.rsvp.as_deref() == Some("going")
    }
}

async fn access(db: &Db, id: &str, did: &str) -> Result<Option<Access>> {
    let Some(event) = load_event(db, id).await? else { return Ok(None) };
    let mut can_see = true;
    let mut parent = event.parent_id.clone();
    for _ in 0..16 {
        let Some(p) = parent else { break };
        if rsvp_of(db, &p, did).await?.as_deref() != Some("going") {
            can_see = false;
            break;
        }
        parent = load_event(db, &p).await?.and_then(|e| e.parent_id);
    }
    let rsvp = rsvp_of(db, id, did).await?;
    Ok(Some(Access { event, rsvp, can_see }))
}

/// The event, if the person can see it.
async fn visible(db: &Db, id: &str, did: &str) -> Result<Access> {
    match access(db, id, did).await? {
        Some(a) if a.can_see => Ok(a),
        _ => Err(not_found()),
    }
}

/// The event, if the person can see inside it.
async fn viewable(db: &Db, id: &str, did: &str) -> Result<Access> {
    let a = visible(db, id, did).await?;
    if a.can_view() { Ok(a) } else { Err(not_going()) }
}

async fn count(db: &Db, sql: &str, id: &str) -> Result<i64> {
    let (n,): (i64,) = sqlx::query_as(sql).bind(id).fetch_one(db).await.map_err(internal)?;
    Ok(n)
}

/// An event as one person sees it in a list.
async fn summary(db: &Db, a: &Access) -> Result<Value> {
    let e = &a.event;
    let going =
        count(db, "SELECT COUNT(*) FROM rsvps WHERE event_id = $1 AND status = 'going'", &e.id)
            .await?;
    let maybe =
        count(db, "SELECT COUNT(*) FROM rsvps WHERE event_id = $1 AND status = 'maybe'", &e.id)
            .await?;
    let subs = count(db, "SELECT COUNT(*) FROM events WHERE parent_id = $1", &e.id).await?;
    let parent = match &e.parent_id {
        Some(p) => load_event(db, p).await?.map(|p| json!({ "id": p.id, "title": p.title })),
        None => None,
    };
    Ok(json!({
        "id": e.id, "title": e.title, "description": e.description,
        "startsAt": e.starts_at, "endsAt": e.ends_at, "timeZone": e.time_zone,
        "location": e.location, "host": e.host, "parent": parent,
        "going": going, "maybe": maybe, "rsvp": a.rsvp,
        "canView": a.can_view(), "subEventCount": if a.can_view() { subs } else { 0 },
        "url": format!("/e/{}", e.id),
    }))
}

/// An event's page: its summary and who's going.
async fn detail(db: &Db, a: &Access) -> Result<Value> {
    let mut out = summary(db, a).await?;
    let going: Vec<(String,)> = sqlx::query_as(
        "SELECT did FROM rsvps WHERE event_id = $1 AND status = 'going' ORDER BY updated_at DESC LIMIT 24",
    )
    .bind(&a.event.id)
    .fetch_all(db)
    .await
    .map_err(internal)?;
    out["attendees"] = json!(going.into_iter().map(|r| r.0).collect::<Vec<_>>());
    Ok(out)
}

pub async fn list_events(State(state): State<AppState>, user: CurrentUser) -> Result<Response> {
    arrive(&state.db, &user).await?;
    let rows: Vec<EventTuple> =
        sqlx::query_as(&format!("SELECT {EVENT_COLUMNS} FROM events ORDER BY starts_at, id"))
            .fetch_all(&state.db)
            .await
            .map_err(internal)?;
    let events: HashMap<String, EventRow> =
        rows.into_iter().map(EventRow::from).map(|e| (e.id.clone(), e)).collect();
    let mine: HashMap<String, String> =
        sqlx::query_as::<_, (String, String)>("SELECT event_id, status FROM rsvps WHERE did = $1")
            .bind(&user.did)
            .fetch_all(&state.db)
            .await
            .map_err(internal)?
            .into_iter()
            .collect();
    let can_see = |e: &EventRow| {
        let mut parent = e.parent_id.as_deref();
        for _ in 0..16 {
            let Some(p) = parent else { return true };
            if mine.get(p).map(String::as_str) != Some("going") {
                return false;
            }
            parent = events.get(p).and_then(|e| e.parent_id.as_deref());
        }
        false
    };
    let mut seen: Vec<&EventRow> = events.values().filter(|e| can_see(e)).collect();
    seen.sort_by(|a, b| a.starts_at.cmp(&b.starts_at).then(a.id.cmp(&b.id)));
    let mut out = vec![];
    for e in seen {
        let a = Access { event: e.clone(), rsvp: mine.get(&e.id).cloned(), can_see: true };
        out.push(summary(&state.db, &a).await?);
    }
    Ok(no_store(json!({ "events": out })))
}

pub async fn get_event(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(q): Query<IdQuery>,
) -> Result<Response> {
    let a = visible(&state.db, &q.id, &user.did).await?;
    let event = detail(&state.db, &a).await?;
    let (feeds, subs) = if a.can_view() {
        (feeds_of(&state.db, &q.id).await?, sub_events(&state.db, &q.id, &user.did).await?)
    } else {
        (vec![], vec![])
    };
    Ok(no_store(json!({ "event": event, "feeds": feeds, "subEvents": subs })))
}

async fn feeds_of(db: &Db, event_id: &str) -> Result<Vec<Value>> {
    let feeds: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, name FROM feeds WHERE event_id = $1 ORDER BY position, created_at",
    )
    .bind(event_id)
    .fetch_all(db)
    .await
    .map_err(internal)?;
    Ok(feeds.into_iter().map(|(id, name)| feed_json(&id, &name, event_id)).collect())
}

async fn sub_events(db: &Db, event_id: &str, did: &str) -> Result<Vec<Value>> {
    let rows: Vec<EventTuple> = sqlx::query_as(&format!(
        "SELECT {EVENT_COLUMNS} FROM events WHERE parent_id = $1 ORDER BY starts_at, id"
    ))
    .bind(event_id)
    .fetch_all(db)
    .await
    .map_err(internal)?;
    let mut out = vec![];
    for row in rows {
        let event = EventRow::from(row);
        let rsvp = rsvp_of(db, &event.id, did).await?;
        out.push(summary(db, &Access { event, rsvp, can_see: true }).await?);
    }
    Ok(out)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateEvent {
    title: String,
    #[serde(default)]
    description: String,
    starts_at: i64,
    ends_at: Option<i64>,
    time_zone: Option<String>,
    #[serde(default)]
    location: String,
    parent_id: Option<String>,
    /// Where in the parent to post the event's card.
    feed_id: Option<String>,
}

pub async fn create_event(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<CreateEvent>,
) -> Result<Response> {
    arrive(&state.db, &user).await?;
    let title = body.title.trim();
    if title.is_empty() || title.chars().count() > 100 {
        return Err(bad("An event needs a title of up to 100 characters."));
    }
    if body.description.chars().count() > 2000 || body.location.chars().count() > 200 {
        return Err(bad("That description or location is too long."));
    }
    if body.ends_at.is_some_and(|end| end < body.starts_at) {
        return Err(bad("An event can't end before it starts."));
    }
    if let Some(parent) = &body.parent_id {
        viewable(&state.db, parent, &user.did).await?;
    }
    let event = EventRow {
        id: new_id(),
        parent_id: body.parent_id.clone(),
        title: title.to_owned(),
        description: body.description.trim().to_owned(),
        starts_at: body.starts_at,
        ends_at: body.ends_at,
        time_zone: body
            .time_zone
            .filter(|tz| !tz.is_empty() && tz.len() < 64)
            .unwrap_or_else(|| "Europe/Amsterdam".into()),
        location: body.location.trim().to_owned(),
        host: user.did.clone(),
    };
    let card = create(&state, &event, body.feed_id.as_deref(), true).await?;
    let a = visible(&state.db, &event.id, &user.did).await?;
    Ok(no_store(json!({ "event": summary(&state.db, &a).await?, "cardId": card })))
}

/// Creates an event, the host's RSVP and, with `general`, a first feed. A
/// sub-event also gets a card in its parent (in `feed_id`, or the parent's
/// first feed), whose id is returned.
pub(crate) async fn create(
    state: &AppState,
    event: &EventRow,
    feed_id: Option<&str>,
    general: bool,
) -> Result<Option<String>> {
    let db = &state.db;
    sqlx::query(&format!(
        "INSERT INTO events ({EVENT_COLUMNS}, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"
    ))
    .bind(&event.id)
    .bind(&event.parent_id)
    .bind(&event.title)
    .bind(&event.description)
    .bind(event.starts_at)
    .bind(event.ends_at)
    .bind(&event.time_zone)
    .bind(&event.location)
    .bind(&event.host)
    .bind(now_ms())
    .execute(db)
    .await
    .map_err(internal)?;
    put_rsvp(db, &event.id, &event.host, "going").await?;
    if general {
        insert_feed(db, &event.id, "General").await?;
    }

    let mut card = None;
    if let Some(parent) = &event.parent_id {
        let feeds = feeds_of(db, parent).await?;
        let feed = feeds
            .iter()
            .filter_map(|f| f["id"].as_str())
            .find(|f| Some(*f) == feed_id)
            .or_else(|| feeds.first().and_then(|f| f["id"].as_str()));
        if let Some(feed) = feed {
            let ui = json!({
                "$type": "app.eventside.block.card",
                "blocks": [{ "$type": "app.eventside.block.defs#header", "text": event.title }],
                "fallbackText": event.title,
                "timeZone": event.time_zone,
                "createdAt": iso_now(),
            });
            let id = insert_card(
                state,
                NewCard {
                    feed_id: feed,
                    author: &event.host,
                    ui: &ui,
                    config: &json!({}),
                    module_hash: &runtime::builtin_hash("event"),
                    event_id: Some(&event.id),
                    at: now_ms(),
                },
            )
            .await?;
            state
                .changes
                .send(Change::Card {
                    event_id: parent.clone(),
                    feed_id: feed.to_owned(),
                    card_id: id.clone(),
                })
                .ok();
            card = Some(id);
        }
    }
    state
        .changes
        .send(Change::Event { event_id: event.id.clone(), parent_id: event.parent_id.clone() })
        .ok();
    Ok(card)
}

fn iso_now() -> String {
    // The card kit wants an ISO timestamp; the exact second doesn't matter.
    let secs = now_ms() / 1000;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.000Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

async fn put_rsvp(db: &Db, event_id: &str, did: &str, status: &str) -> Result<()> {
    sqlx::query(
        "INSERT INTO rsvps (event_id, did, status, updated_at) VALUES ($1, $2, $3, $4) \
         ON CONFLICT (event_id, did) DO UPDATE SET status = excluded.status, updated_at = excluded.updated_at",
    )
    .bind(event_id)
    .bind(did)
    .bind(status)
    .bind(now_ms())
    .execute(db)
    .await
    .map_err(internal)?;
    Ok(())
}

/// Records an RSVP and tells everyone who shows the event: its own page,
/// its parent's plans, and the cards that link to it.
pub(crate) async fn set_rsvp(
    state: &AppState,
    event: &EventRow,
    did: &str,
    status: &str,
) -> Result<()> {
    if !matches!(status, "going" | "maybe" | "no") {
        return Err(bad("An RSVP is going, maybe or no."));
    }
    if event.host == did && status != "going" {
        return Err(bad("The host is always going."));
    }
    put_rsvp(&state.db, &event.id, did, status).await?;
    state
        .changes
        .send(Change::Event { event_id: event.id.clone(), parent_id: event.parent_id.clone() })
        .ok();
    let cards: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT c.id, c.feed_id, f.event_id FROM cards c JOIN feeds f ON f.id = c.feed_id WHERE c.event_id = $1",
    )
    .bind(&event.id)
    .fetch_all(&state.db)
    .await
    .map_err(internal)?;
    for (card_id, feed_id, event_id) in cards {
        state.changes.send(Change::Card { event_id, feed_id, card_id }).ok();
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct Rsvp {
    id: String,
    status: String,
}

pub async fn rsvp(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<Rsvp>,
) -> Result<Response> {
    arrive(&state.db, &user).await?;
    let a = visible(&state.db, &body.id, &user.did).await?;
    set_rsvp(&state, &a.event, &user.did, &body.status).await?;
    let a = visible(&state.db, &body.id, &user.did).await?;
    Ok(no_store(json!({ "event": detail(&state.db, &a).await? })))
}

/// The event's live updates, as server-sent events of generic payloads.
pub async fn subscribe(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(q): Query<IdQuery>,
) -> Result<Response> {
    viewable(&state.db, &q.id, &user.did).await?;
    let rx = state.changes.subscribe();
    let hello = Event::default().data(json!({ "type": "hello" }).to_string());
    let start = futures_util::stream::once(async move { Ok::<_, Infallible>(hello) });
    let updates = futures_util::stream::unfold(
        (rx, state, user.did, user.handle, q.id),
        |(mut rx, state, did, handle, event_id)| async move {
            loop {
                let payload = match rx.recv().await {
                    Ok(change) => {
                        // Stop as soon as the person can't see inside any more.
                        match access(&state.db, &event_id, &did).await {
                            Ok(Some(a)) if a.can_view() => {}
                            _ => return None,
                        }
                        match payload_for(&state.db, change, &event_id, &did, &handle).await {
                            Some(p) => p,
                            None => continue,
                        }
                    }
                    // Too far behind: the client reloads.
                    Err(broadcast::error::RecvError::Lagged(_)) => json!({ "type": "resync" }),
                    Err(broadcast::error::RecvError::Closed) => return None,
                };
                let event = Event::default().data(payload.to_string());
                return Some((Ok::<_, Infallible>(event), (rx, state, did, handle, event_id)));
            }
        },
    );
    use futures_util::StreamExt;
    Ok(Sse::new(start.chain(updates)).keep_alive(KeepAlive::default()).into_response())
}

async fn payload_for(
    db: &Db,
    change: Change,
    mine: &str,
    did: &str,
    handle: &str,
) -> Option<Value> {
    match change {
        Change::Card { event_id, feed_id, card_id } if event_id == mine => {
            let card = load_card(db, &card_id, did, handle).await.ok()??;
            Some(json!({ "type": "card", "feedId": feed_id, "card": card }))
        }
        Change::CardRemoved { event_id, feed_id, card_id } if event_id == mine => {
            Some(json!({ "type": "cardRemoved", "feedId": feed_id, "cardId": card_id }))
        }
        Change::Feed { event_id, feed_id } if event_id == mine => {
            let (name,): (String,) = sqlx::query_as("SELECT name FROM feeds WHERE id = $1")
                .bind(&feed_id)
                .fetch_one(db)
                .await
                .ok()?;
            Some(json!({ "type": "feed", "feed": feed_json(&feed_id, &name, &event_id) }))
        }
        Change::Actions { event_id, card_id, actions } if event_id == mine => {
            let actions: Vec<Value> = actions
                .into_iter()
                .filter(|a| a.get("to").and_then(Value::as_str).is_none_or(|to| to == did))
                .collect();
            if actions.is_empty() {
                return None;
            }
            Some(json!({ "type": "actions", "cardId": card_id, "actions": actions }))
        }
        Change::Event { event_id, .. } if event_id == mine => {
            let a = access(db, &event_id, did).await.ok()??;
            Some(json!({ "type": "event", "event": detail(db, &a).await.ok()? }))
        }
        Change::Event { event_id, parent_id } if parent_id.as_deref() == Some(mine) => {
            let a = access(db, &event_id, did).await.ok()??;
            Some(json!({ "type": "subEvent", "event": summary(db, &a).await.ok()? }))
        }
        _ => None,
    }
}

// Feeds

fn feed_json(id: &str, name: &str, event_id: &str) -> Value {
    json!({ "id": id, "name": name, "eventId": event_id })
}

pub(crate) async fn insert_feed(db: &Db, event_id: &str, name: &str) -> Result<String> {
    let id = new_id();
    let now = now_ms();
    sqlx::query(
        "INSERT INTO feeds (id, event_id, name, position, created_at) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&id)
    .bind(event_id)
    .bind(name)
    .bind(now)
    .bind(now)
    .execute(db)
    .await
    .map_err(internal)?;
    Ok(id)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateFeed {
    event_id: String,
    name: String,
}

pub async fn create_feed(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<CreateFeed>,
) -> Result<Response> {
    viewable(&state.db, &body.event_id, &user.did).await?;
    let name = body.name.trim();
    if name.is_empty() || name.chars().count() > 60 {
        return Err(bad("A feed needs a name of up to 60 characters."));
    }
    let id = insert_feed(&state.db, &body.event_id, name).await?;
    state.changes.send(Change::Feed { event_id: body.event_id.clone(), feed_id: id.clone() }).ok();
    Ok(no_store(feed_json(&id, name, &body.event_id)))
}

/// The event a feed is in, if the person can see inside it.
async fn feed_event(db: &Db, feed_id: &str, did: &str) -> Result<Access> {
    let row: Option<(String,)> = sqlx::query_as("SELECT event_id FROM feeds WHERE id = $1")
        .bind(feed_id)
        .fetch_optional(db)
        .await
        .map_err(internal)?;
    viewable(db, &row.ok_or_else(not_found)?.0, did).await
}

pub async fn get_cards(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(q): Query<IdQuery>,
) -> Result<Response> {
    feed_event(&state.db, &q.id, &user.did).await?;
    let rows: Vec<CardRow> = sqlx::query_as(&format!(
        "SELECT {CARD_COLUMNS} FROM cards WHERE feed_id = $1 ORDER BY created_at DESC, id"
    ))
    .bind(&q.id)
    .fetch_all(&state.db)
    .await
    .map_err(internal)?;
    let mut cards = vec![];
    for row in rows {
        cards.push(card_view(&state.db, row, &user.did, &user.handle).await?);
    }
    Ok(no_store(json!({ "cards": cards })))
}

// Cards

type CardRow = (String, String, String, String, String, String, String, Option<String>, i64, i64);
const CARD_COLUMNS: &str =
    "id, feed_id, author_did, ui, config, state, module_hash, event_id, version, created_at";

fn parse(s: &str) -> Value {
    serde_json::from_str(s).unwrap_or(Value::Null)
}

/// Where a client fetches a module. The default module goes by its name,
/// so clients can tell a plain card-kit card from one with its own logic.
fn module_url(hash: &str) -> String {
    let hash = if hash == runtime::builtin_hash("std") { "std" } else { hash };
    format!("/xrpc/app.eventside.card.getModule?hash={hash}")
}

/// What the platform tells a card about this viewer: who they are, and the
/// event an event card shows.
async fn context_for(db: &Db, event_id: Option<&str>, did: &str, handle: &str) -> Result<Value> {
    let mut context = json!({ "viewer": { "did": did, "handle": handle } });
    if let Some(id) = event_id
        && let Some(a) = access(db, id, did).await?
    {
        context["event"] = summary(db, &a).await?;
    }
    Ok(context)
}

/// A card as one viewer sees it.
async fn card_view(db: &Db, row: CardRow, viewer: &str, handle: &str) -> Result<Value> {
    let (id, feed_id, author, ui, config, state, module_hash, event_id, version, created_at) = row;
    let context = context_for(db, event_id.as_deref(), viewer, handle).await?;
    Ok(json!({
        "id": id, "feedId": feed_id, "author": author, "version": version,
        "createdAt": created_at, "ui": parse(&ui), "config": parse(&config),
        "state": parse(&state), "context": context, "module": module_url(&module_hash),
        "eventId": event_id,
    }))
}

async fn card_row(db: &Db, card_id: &str) -> Result<Option<CardRow>> {
    sqlx::query_as(&format!("SELECT {CARD_COLUMNS} FROM cards WHERE id = $1"))
        .bind(card_id)
        .fetch_optional(db)
        .await
        .map_err(internal)
}

async fn load_card(db: &Db, card_id: &str, viewer: &str, handle: &str) -> Result<Option<Value>> {
    match card_row(db, card_id).await? {
        Some(row) => Ok(Some(card_view(db, row, viewer, handle).await?)),
        None => Ok(None),
    }
}

/// A card's feed and event, if the person can see inside the event.
async fn card_place(db: &Db, card_id: &str, did: &str) -> Result<(String, Access)> {
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT c.feed_id, f.event_id FROM cards c JOIN feeds f ON f.id = c.feed_id WHERE c.id = $1",
    )
    .bind(card_id)
    .fetch_optional(db)
    .await
    .map_err(internal)?;
    let (feed_id, event_id) = row.ok_or_else(not_found)?;
    Ok((feed_id, viewable(db, &event_id, did).await?))
}

fn check_ui(ui: &Value) -> Result<()> {
    let blocks = ui.get("blocks").and_then(Value::as_array);
    match blocks {
        Some(b) if !b.is_empty() && b.len() <= 50 => Ok(()),
        _ => Err(bad("A card's ui needs 1 to 50 blocks.")),
    }
}

/// Runs a module's server reducer.
async fn reduce(state: &AppState, module_hash: &str, input: Value) -> Result<Value> {
    let bytes = if state.runtime.has(module_hash) {
        None
    } else {
        runtime::bytes(&state.db, module_hash).await.map_err(internal)?
    };
    let runtime = state.runtime.clone();
    let hash = module_hash.to_owned();
    tokio::task::spawn_blocking(move || runtime.call(&hash, bytes.as_deref(), "reduce", &input))
        .await
        .map_err(|e| module_error(e.to_string()))?
        .map_err(module_error)
}

fn seed() -> u32 {
    OsRng.next_u32()
}

pub(crate) struct NewCard<'a> {
    pub feed_id: &'a str,
    pub author: &'a str,
    pub ui: &'a Value,
    pub config: &'a Value,
    pub module_hash: &'a str,
    pub event_id: Option<&'a str>,
    pub at: i64,
}

/// Stores a card, its shared state from the server reducer's `@init`.
pub(crate) async fn insert_card(state: &AppState, card: NewCard<'_>) -> Result<String> {
    let id = new_id();
    let now = now_ms();
    let input = json!({
        "env": "server", "state": null, "shared": null,
        "action": { "type": "@init", "payload": null, "meta": {
            "id": new_id(), "actor": { "did": card.author }, "at": now, "origin": "platform",
        }},
        "config": card.config, "base": card.ui, "context": {}, "seed": seed(), "now": now,
    });
    let out = reduce(state, card.module_hash, input).await?;
    let initial = out.get("state").cloned().unwrap_or(Value::Null);
    sqlx::query(&format!(
        "INSERT INTO cards ({CARD_COLUMNS}) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 1, $9)"
    ))
    .bind(&id)
    .bind(card.feed_id)
    .bind(card.author)
    .bind(card.ui.to_string())
    .bind(card.config.to_string())
    .bind(initial.to_string())
    .bind(card.module_hash)
    .bind(card.event_id)
    .bind(card.at)
    .execute(&state.db)
    .await
    .map_err(internal)?;
    Ok(id)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCard {
    feed_id: String,
    ui: Value,
    #[serde(default)]
    config: Option<Value>,
    /// A wasm module, base64.
    #[serde(default)]
    module: Option<String>,
}

pub async fn create_card(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<CreateCard>,
) -> Result<Response> {
    let a = feed_event(&state.db, &body.feed_id, &user.did).await?;
    check_ui(&body.ui)?;
    let module_hash = match body.module.as_deref().filter(|m| !m.is_empty()) {
        Some(b64) => {
            let bytes = STANDARD.decode(b64).map_err(|_| bad("The module isn't valid base64."))?;
            runtime::check(&state.runtime, &bytes).map_err(|e| bad(&e))?;
            runtime::store(&state.db, &bytes).await.map_err(internal)?
        }
        None => runtime::builtin_hash("std"),
    };
    let config = body.config.unwrap_or(json!({}));
    let id = insert_card(
        &state,
        NewCard {
            feed_id: &body.feed_id,
            author: &user.did,
            ui: &body.ui,
            config: &config,
            module_hash: &module_hash,
            event_id: None,
            at: now_ms(),
        },
    )
    .await?;
    state
        .changes
        .send(Change::Card { event_id: a.event.id, feed_id: body.feed_id, card_id: id.clone() })
        .ok();
    let card = load_card(&state.db, &id, &user.did, &user.handle).await?;
    Ok(no_store(json!({ "card": card })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteCard {
    card_id: String,
}

pub async fn delete_card(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<DeleteCard>,
) -> Result<Response> {
    let (feed_id, a) = card_place(&state.db, &body.card_id, &user.did).await?;
    let (author,): (String,) = sqlx::query_as("SELECT author_did FROM cards WHERE id = $1")
        .bind(&body.card_id)
        .fetch_one(&state.db)
        .await
        .map_err(internal)?;
    if author != user.did && a.event.host != user.did {
        return Err(xrpc_error(
            StatusCode::FORBIDDEN,
            "Forbidden",
            "Only its author or the host can delete a card.",
        ));
    }
    for sql in ["DELETE FROM card_actions WHERE card_id = $1", "DELETE FROM cards WHERE id = $1"] {
        sqlx::query(sql).bind(&body.card_id).execute(&state.db).await.map_err(internal)?;
    }
    state
        .changes
        .send(Change::CardRemoved { event_id: a.event.id, feed_id, card_id: body.card_id })
        .ok();
    Ok(no_store(json!({})))
}

/// An action a client sends up.
#[derive(Deserialize)]
pub struct Dispatched {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    payload: Value,
    #[serde(default)]
    meta: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dispatch {
    card_id: String,
    actions: Vec<Dispatched>,
}

pub async fn dispatch(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<Dispatch>,
) -> Result<Response> {
    card_place(&state.db, &body.card_id, &user.did).await?;
    if body.actions.is_empty() || body.actions.len() > 10 {
        return Err(bad("Send 1 to 10 actions at a time."));
    }
    for a in &body.actions {
        if a.kind.is_empty() || a.kind.len() > 64 {
            return Err(bad("An action needs a type of up to 64 characters."));
        }
        // Platform effects come only from server reducers.
        if a.kind.starts_with('@') {
            return Err(bad("A client can't send @ actions."));
        }
        if a.payload.to_string().len() > 10_000 {
            return Err(bad("That's too much to send."));
        }
    }
    arrive(&state.db, &user).await?;
    let who = actor(&user);
    for a in body.actions {
        // A client may say which block and list item an action came from; the rest of meta is ours.
        let mut extra = Map::new();
        for key in ["blockId", "item"] {
            if let Some(v) = a.meta.get(key).filter(|v| !v.is_null()) {
                extra.insert(key.into(), v.clone());
            }
        }
        apply(&state, &body.card_id, &who, &a.kind, a.payload, extra, "client").await?;
    }
    let card = load_card(&state.db, &body.card_id, &user.did, &user.handle).await?;
    Ok(no_store(json!({ "card": card })))
}

/// Reduces one action into a card's shared state, then runs the effects
/// the reducer emitted and sends the rest down to clients.
pub(crate) async fn apply(
    state: &AppState,
    card_id: &str,
    actor: &Value,
    kind: &str,
    payload: Value,
    extra_meta: Map<String, Value>,
    origin: &str,
) -> Result<String> {
    let did = actor["did"].as_str().unwrap_or_default().to_owned();
    let mut saved = None;
    // Optimistic: if someone else's action landed first, reduce again on top of it.
    for _ in 0..8 {
        let Some(row) = card_row(&state.db, card_id).await? else { return Err(not_found()) };
        let (_, feed_id, _, ui, config, shared, module_hash, event_id, version, _) = row;
        let now = now_ms();
        let mut meta = extra_meta.clone();
        meta.insert("id".into(), json!(new_id()));
        meta.insert("actor".into(), actor.clone());
        meta.insert("at".into(), json!(now));
        meta.insert("origin".into(), json!(origin));
        let action = json!({ "type": kind, "payload": payload, "meta": meta });
        let shared = parse(&shared);
        let input = json!({
            "env": "server", "state": shared, "shared": shared, "action": action,
            "config": parse(&config), "base": parse(&ui), "context": {}, "seed": seed(), "now": now,
        });
        let out = reduce(state, &module_hash, input).await?;
        let next = out.get("state").cloned().unwrap_or(shared);
        let updated = sqlx::query(
            "UPDATE cards SET state = $1, version = version + 1 WHERE id = $2 AND version = $3",
        )
        .bind(next.to_string())
        .bind(card_id)
        .bind(version)
        .execute(&state.db)
        .await
        .map_err(internal)?;
        if updated.rows_affected() == 0 {
            continue;
        }
        sqlx::query(
            "INSERT INTO card_actions (id, card_id, did, type, payload, meta, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(action["meta"]["id"].as_str().unwrap_or_default())
        .bind(card_id)
        .bind(&did)
        .bind(kind)
        .bind(action["payload"].to_string())
        .bind(action["meta"].to_string())
        .bind(now)
        .execute(&state.db)
        .await
        .map_err(internal)?;
        let emitted = out.get("actions").and_then(Value::as_array).cloned().unwrap_or_default();
        saved = Some((
            feed_id,
            event_id,
            emitted,
            action["meta"]["id"].as_str().unwrap_or_default().to_owned(),
        ));
        break;
    }
    let Some((feed_id, linked_event, emitted, action_id)) = saved else {
        return Err(xrpc_error(StatusCode::CONFLICT, "Busy", "The card is busy. Try again."));
    };
    let place: Option<(String,)> = sqlx::query_as("SELECT event_id FROM feeds WHERE id = $1")
        .bind(&feed_id)
        .fetch_optional(&state.db)
        .await
        .map_err(internal)?;
    let event_id = place.ok_or_else(not_found)?.0;

    let mut down = vec![];
    for e in emitted.into_iter().take(20) {
        let Some(kind) = e.get("type").and_then(Value::as_str) else { continue };
        if kind.starts_with('@') {
            effect(state, linked_event.as_deref(), &did, kind, &e["payload"]).await?;
            continue;
        }
        let mut out = json!({
            "type": kind, "payload": e.get("payload").cloned().unwrap_or(Value::Null),
            "meta": { "id": new_id(), "actor": actor, "at": now_ms(), "origin": "server" },
        });
        if let Some(to) = e.get("to").and_then(Value::as_str) {
            out["to"] = json!(to);
        }
        down.push(out);
    }
    state
        .changes
        .send(Change::Card { event_id: event_id.clone(), feed_id, card_id: card_id.to_owned() })
        .ok();
    if !down.is_empty() {
        state
            .changes
            .send(Change::Actions { event_id, card_id: card_id.to_owned(), actions: down })
            .ok();
    }
    Ok(action_id)
}

/// A platform effect a server reducer asked for, run as the actor.
async fn effect(
    state: &AppState,
    linked_event: Option<&str>,
    did: &str,
    kind: &str,
    payload: &Value,
) -> Result<()> {
    match kind {
        "@rsvp" => {
            let Some(event_id) = linked_event else {
                return Err(bad("This card isn't linked to an event."));
            };
            let a = visible(&state.db, event_id, did).await?;
            let status = payload.get("status").and_then(Value::as_str).unwrap_or_default();
            set_rsvp(state, &a.event, did, status).await
        }
        other => {
            eprintln!("card module asked for an unknown effect {other}");
            Ok(())
        }
    }
}

#[derive(Deserialize)]
pub struct HashQuery {
    hash: String,
}

pub async fn get_module(
    State(state): State<AppState>,
    _user: CurrentUser,
    Query(q): Query<HashQuery>,
) -> Result<Response> {
    // Any built-in module by name, e.g. the composer's preview asks for `std`.
    let hash = match runtime::builtin(&q.hash) {
        Some(_) => runtime::builtin_hash(&q.hash),
        None => q.hash.clone(),
    };
    let bytes = runtime::bytes(&state.db, &hash).await.map_err(internal)?.ok_or_else(not_found)?;
    let mut res = bytes.into_response();
    let headers = res.headers_mut();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/wasm"));
    let cache = if hash == q.hash { "private, max-age=31536000, immutable" } else { "no-cache" };
    headers.insert(CACHE_CONTROL, HeaderValue::from_static(cache));
    Ok(res)
}

#[derive(Deserialize)]
pub struct DidQuery {
    did: String,
}

pub async fn get_profile(
    State(state): State<AppState>,
    _user: CurrentUser,
    Query(q): Query<DidQuery>,
) -> Result<Response> {
    let row: Option<(String, Option<String>, Option<String>)> =
        sqlx::query_as("SELECT handle, display_name, avatar FROM profiles WHERE did = $1")
            .bind(&q.did)
            .fetch_optional(&state.db)
            .await
            .map_err(internal)?;
    let Some((handle, display_name, avatar)) = row else { return Err(not_found()) };
    let mut res = Json(json!({
        "did": q.did, "handle": handle, "displayName": display_name, "avatar": avatar,
    }))
    .into_response();
    res.headers_mut().insert(CACHE_CONTROL, HeaderValue::from_static("private, max-age=300"));
    Ok(res)
}
