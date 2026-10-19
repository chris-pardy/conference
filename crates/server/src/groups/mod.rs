//! Groups of people, their feeds, and the cards in those feeds. Everything
//! is stored here; atproto is only how people sign in.
//!
//! A card is a card-kit UI (`ui`), the reducers that fold actions into its
//! data, and an optional wasm module the client runs on each action.
//!
//! - Up: a client sends an action as a generic payload,
//!   `{cardId, blockId, actionId, value, item?}`.
//! - Down: the group's stream sends generic updates,
//!   `{type: "card", feedId, card}`, `{type: "cardRemoved", feedId, cardId}`
//!   and `{type: "feed", feed}`. A card is sent whole, with its data as the
//!   receiving viewer sees it.

// Handlers answer with a Response as their error, and rows come as tuples.
#![allow(clippy::result_large_err, clippy::type_complexity)]

pub mod reduce;
pub mod seed;

use std::convert::Infallible;

use axum::Json;
use axum::extract::{Query, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::broadcast;

use crate::AppState;
use crate::auth::{CurrentUser, xrpc_error};
use crate::db::{Db, now_ms};
use crate::keys::random_token;

/// Something in a group changed. Each stream turns it into a payload for its viewer.
#[derive(Debug, Clone)]
pub enum Change {
    Card { group_id: String, feed_id: String, card_id: String },
    CardRemoved { group_id: String, feed_id: String, card_id: String },
    Feed { group_id: String, feed_id: String },
}

impl Change {
    fn group_id(&self) -> &str {
        match self {
            Self::Card { group_id, .. }
            | Self::CardRemoved { group_id, .. }
            | Self::Feed { group_id, .. } => group_id,
        }
    }
}

pub fn channel() -> broadcast::Sender<Change> {
    broadcast::channel(1024).0
}

type Result<T> = std::result::Result<T, Response>;

fn internal(err: sqlx::Error) -> Response {
    eprintln!("groups: {err}");
    xrpc_error(StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError", "Something went wrong.")
}

fn bad(message: &str) -> Response {
    xrpc_error(StatusCode::BAD_REQUEST, "InvalidRequest", message)
}

fn not_found() -> Response {
    xrpc_error(StatusCode::NOT_FOUND, "NotFound", "Not found, or you're not a member.")
}

fn new_id() -> String {
    random_token(9)
}

fn invite_code() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    random_token(6).bytes().map(|b| ALPHABET[b as usize % ALPHABET.len()] as char).take(6).collect()
}

/// Records who the person is, for person blocks, and puts them in the demo group.
async fn arrive(state: &AppState, user: &CurrentUser) -> Result<()> {
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
    .execute(&state.db)
    .await
    .map_err(internal)?;
    join(&state.db, seed::DEMO_GROUP, &user.did, "member").await
}

async fn join(db: &Db, group_id: &str, did: &str, role: &str) -> Result<()> {
    sqlx::query(
        "INSERT INTO group_members (group_id, did, role, joined_at) VALUES ($1, $2, $3, $4) \
         ON CONFLICT (group_id, did) DO NOTHING",
    )
    .bind(group_id)
    .bind(did)
    .bind(role)
    .bind(now_ms())
    .execute(db)
    .await
    .map_err(internal)?;
    Ok(())
}

/// The person's role in a group, or NotFound when they aren't in it.
async fn membership(db: &Db, group_id: &str, did: &str) -> Result<String> {
    let role: Option<(String,)> =
        sqlx::query_as("SELECT role FROM group_members WHERE group_id = $1 AND did = $2")
            .bind(group_id)
            .bind(did)
            .fetch_optional(db)
            .await
            .map_err(internal)?;
    role.map(|r| r.0).ok_or_else(not_found)
}

/// The group a feed is in, if the person is a member.
async fn feed_group(db: &Db, feed_id: &str, did: &str) -> Result<String> {
    let row: Option<(String,)> = sqlx::query_as("SELECT group_id FROM feeds WHERE id = $1")
        .bind(feed_id)
        .fetch_optional(db)
        .await
        .map_err(internal)?;
    let group_id = row.ok_or_else(not_found)?.0;
    membership(db, &group_id, did).await?;
    Ok(group_id)
}

/// A card's feed and group, if the person is a member.
async fn card_place(db: &Db, card_id: &str, did: &str) -> Result<(String, String)> {
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT c.feed_id, f.group_id FROM cards c JOIN feeds f ON f.id = c.feed_id WHERE c.id = $1",
    )
    .bind(card_id)
    .fetch_optional(db)
    .await
    .map_err(internal)?;
    let (feed_id, group_id) = row.ok_or_else(not_found)?;
    membership(db, &group_id, did).await?;
    Ok((feed_id, group_id))
}

type CardRow = (String, String, String, String, String, Option<String>, i64, i64);
const CARD_COLUMNS: &str = "id, feed_id, author_did, ui, reducers, module, version, created_at";

/// A card as one viewer sees it: its UI, and its data computed for them.
async fn card_view(db: &Db, row: CardRow, viewer: &str, viewer_handle: &str) -> Result<Value> {
    let (id, feed_id, author, ui, reducers, module, version, created_at) = row;
    let acts: Vec<(String, String, String, String, String, String, i64, Option<String>)> =
        sqlx::query_as(
            "SELECT a.id, a.did, a.block_id, a.action_id, a.value, a.item, a.created_at, p.handle \
         FROM card_actions a LEFT JOIN profiles p ON p.did = a.did \
         WHERE a.card_id = $1 ORDER BY a.created_at, a.id",
        )
        .bind(&id)
        .fetch_all(db)
        .await
        .map_err(internal)?;
    let acts: Vec<reduce::Act> = acts
        .into_iter()
        .map(|(id, did, block_id, action_id, value, item, at, handle)| reduce::Act {
            handle: handle.unwrap_or_else(|| did.clone()),
            id,
            did,
            block_id,
            action_id,
            value: serde_json::from_str(&value).unwrap_or(Value::Null),
            item: serde_json::from_str(&item).unwrap_or(Value::Null),
            at,
        })
        .collect();
    let ui: Value = serde_json::from_str(&ui).unwrap_or(Value::Null);
    let reducers: Value = serde_json::from_str(&reducers).unwrap_or(json!({}));
    let data = reduce::data(&ui, &reducers, &acts, viewer, viewer_handle);
    let module = module.map(|_| format!("/xrpc/app.eventside.card.getModule?id={id}&v={version}"));
    Ok(json!({
        "id": id, "feedId": feed_id, "author": author, "version": version,
        "createdAt": created_at, "ui": ui, "reducers": reducers, "data": data, "module": module,
    }))
}

async fn load_card(db: &Db, card_id: &str, viewer: &str, handle: &str) -> Result<Option<Value>> {
    let row: Option<CardRow> =
        sqlx::query_as(&format!("SELECT {CARD_COLUMNS} FROM cards WHERE id = $1"))
            .bind(card_id)
            .fetch_optional(db)
            .await
            .map_err(internal)?;
    match row {
        Some(row) => Ok(Some(card_view(db, row, viewer, handle).await?)),
        None => Ok(None),
    }
}

fn feed_json(id: String, name: String, group_id: String) -> Value {
    json!({ "id": id, "name": name, "groupId": group_id })
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

// Groups

pub async fn list_groups(State(state): State<AppState>, user: CurrentUser) -> Result<Response> {
    arrive(&state, &user).await?;
    let rows: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT g.id, g.name, g.description, m.role FROM groups g \
         JOIN group_members m ON m.group_id = g.id AND m.did = $1 ORDER BY g.created_at",
    )
    .bind(&user.did)
    .fetch_all(&state.db)
    .await
    .map_err(internal)?;
    let mut groups = vec![];
    for (id, name, description, role) in rows {
        let (members,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM group_members WHERE group_id = $1")
                .bind(&id)
                .fetch_one(&state.db)
                .await
                .map_err(internal)?;
        let (feeds,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM feeds WHERE group_id = $1")
            .bind(&id)
            .fetch_one(&state.db)
            .await
            .map_err(internal)?;
        groups.push(json!({
            "id": id, "name": name, "description": description, "role": role,
            "memberCount": members, "feedCount": feeds,
        }));
    }
    Ok(no_store(json!({ "groups": groups })))
}

#[derive(Deserialize)]
pub struct CreateGroup {
    name: String,
    #[serde(default)]
    description: String,
}

pub async fn create_group(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<CreateGroup>,
) -> Result<Response> {
    let name = body.name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err(bad("A group needs a name of up to 100 characters."));
    }
    let id = new_id();
    let now = now_ms();
    sqlx::query(
        "INSERT INTO groups (id, name, description, invite, owner_did, created_at) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(&id)
    .bind(name)
    .bind(body.description.trim())
    .bind(invite_code())
    .bind(&user.did)
    .bind(now)
    .execute(&state.db)
    .await
    .map_err(internal)?;
    join(&state.db, &id, &user.did, "owner").await?;
    insert_feed(&state.db, &id, "General").await?;
    Ok(no_store(json!({ "id": id })))
}

#[derive(Deserialize)]
pub struct JoinGroup {
    invite: String,
}

pub async fn join_group(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<JoinGroup>,
) -> Result<Response> {
    let row: Option<(String,)> = sqlx::query_as("SELECT id FROM groups WHERE invite = $1")
        .bind(body.invite.trim().to_uppercase())
        .fetch_optional(&state.db)
        .await
        .map_err(internal)?;
    let Some((id,)) = row else {
        return Err(xrpc_error(
            StatusCode::NOT_FOUND,
            "NotFound",
            "No group has that invite code.",
        ));
    };
    arrive(&state, &user).await?;
    join(&state.db, &id, &user.did, "member").await?;
    Ok(no_store(json!({ "id": id })))
}

pub async fn get_group(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(q): Query<IdQuery>,
) -> Result<Response> {
    let role = membership(&state.db, &q.id, &user.did).await?;
    let (name, description, invite): (String, String, String) =
        sqlx::query_as("SELECT name, description, invite FROM groups WHERE id = $1")
            .bind(&q.id)
            .fetch_one(&state.db)
            .await
            .map_err(internal)?;
    let feeds: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, name FROM feeds WHERE group_id = $1 ORDER BY position, created_at",
    )
    .bind(&q.id)
    .fetch_all(&state.db)
    .await
    .map_err(internal)?;
    let members: Vec<(String,)> = sqlx::query_as(
        "SELECT did FROM group_members WHERE group_id = $1 ORDER BY joined_at DESC LIMIT 12",
    )
    .bind(&q.id)
    .fetch_all(&state.db)
    .await
    .map_err(internal)?;
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM group_members WHERE group_id = $1")
        .bind(&q.id)
        .fetch_one(&state.db)
        .await
        .map_err(internal)?;
    Ok(no_store(json!({
        "group": {
            "id": q.id, "name": name, "description": description, "invite": invite,
            "role": role, "memberCount": count,
            "members": members.into_iter().map(|m| m.0).collect::<Vec<_>>(),
        },
        "feeds": feeds.into_iter().map(|(id, name)| feed_json(id, name, q.id.clone())).collect::<Vec<_>>(),
    })))
}

/// The group's live updates, as server-sent events of generic payloads.
pub async fn subscribe(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(q): Query<IdQuery>,
) -> Result<Response> {
    membership(&state.db, &q.id, &user.did).await?;
    let rx = state.events.subscribe();
    let hello = Event::default().data(json!({ "type": "hello" }).to_string());
    let start = futures_util::stream::once(async move { Ok::<_, Infallible>(hello) });
    let updates = futures_util::stream::unfold(
        (rx, state, user.did, user.handle, q.id),
        |(mut rx, state, did, handle, group_id)| async move {
            loop {
                let payload = match rx.recv().await {
                    Ok(change) if change.group_id() == group_id => {
                        match payload_for(&state.db, change, &did, &handle).await {
                            Some(p) => p,
                            None => continue,
                        }
                    }
                    Ok(_) => continue,
                    // Too far behind: the client reloads the feed.
                    Err(broadcast::error::RecvError::Lagged(_)) => json!({ "type": "resync" }),
                    Err(broadcast::error::RecvError::Closed) => return None,
                };
                let event = Event::default().data(payload.to_string());
                return Some((Ok::<_, Infallible>(event), (rx, state, did, handle, group_id)));
            }
        },
    );
    use futures_util::StreamExt;
    Ok(Sse::new(start.chain(updates)).keep_alive(KeepAlive::default()).into_response())
}

async fn payload_for(db: &Db, change: Change, did: &str, handle: &str) -> Option<Value> {
    match change {
        Change::Card { feed_id, card_id, .. } => {
            let card = load_card(db, &card_id, did, handle).await.ok()??;
            Some(json!({ "type": "card", "feedId": feed_id, "card": card }))
        }
        Change::CardRemoved { feed_id, card_id, .. } => {
            Some(json!({ "type": "cardRemoved", "feedId": feed_id, "cardId": card_id }))
        }
        Change::Feed { group_id, feed_id } => {
            let (name,): (String,) = sqlx::query_as("SELECT name FROM feeds WHERE id = $1")
                .bind(&feed_id)
                .fetch_one(db)
                .await
                .ok()?;
            Some(json!({ "type": "feed", "feed": feed_json(feed_id, name, group_id) }))
        }
    }
}

// Feeds

async fn insert_feed(db: &Db, group_id: &str, name: &str) -> Result<String> {
    let id = new_id();
    let now = now_ms();
    sqlx::query(
        "INSERT INTO feeds (id, group_id, name, position, created_at) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&id)
    .bind(group_id)
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
    group_id: String,
    name: String,
}

pub async fn create_feed(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<CreateFeed>,
) -> Result<Response> {
    membership(&state.db, &body.group_id, &user.did).await?;
    let name = body.name.trim();
    if name.is_empty() || name.len() > 60 {
        return Err(bad("A feed needs a name of up to 60 characters."));
    }
    let id = insert_feed(&state.db, &body.group_id, name).await?;
    state.events.send(Change::Feed { group_id: body.group_id.clone(), feed_id: id.clone() }).ok();
    Ok(no_store(feed_json(id, name.to_owned(), body.group_id)))
}

pub async fn get_cards(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(q): Query<IdQuery>,
) -> Result<Response> {
    feed_group(&state.db, &q.id, &user.did).await?;
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

fn check_ui(ui: &Value) -> Result<()> {
    let blocks = ui.get("blocks").and_then(Value::as_array);
    match blocks {
        Some(b) if !b.is_empty() && b.len() <= 50 => Ok(()),
        _ => Err(bad("A card's ui needs 1 to 50 blocks.")),
    }
}

fn decode_module(module: Option<&str>) -> Result<Option<String>> {
    let Some(b64) = module.filter(|m| !m.is_empty()) else { return Ok(None) };
    let bytes = STANDARD.decode(b64).map_err(|_| bad("The module isn't valid base64."))?;
    if !bytes.starts_with(b"\0asm") {
        return Err(bad("The module isn't a wasm binary."));
    }
    Ok(Some(b64.to_owned()))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCard {
    feed_id: String,
    ui: Value,
    #[serde(default)]
    reducers: Option<Value>,
    /// A wasm module, base64.
    #[serde(default)]
    module: Option<String>,
}

pub async fn create_card(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<CreateCard>,
) -> Result<Response> {
    let group_id = feed_group(&state.db, &body.feed_id, &user.did).await?;
    check_ui(&body.ui)?;
    let module = decode_module(body.module.as_deref())?;
    let id = insert_card(
        &state.db,
        &body.feed_id,
        &user.did,
        &body.ui,
        body.reducers.as_ref().unwrap_or(&json!({})),
        module,
        now_ms(),
    )
    .await?;
    state.events.send(Change::Card { group_id, feed_id: body.feed_id, card_id: id.clone() }).ok();
    let card = load_card(&state.db, &id, &user.did, &user.handle).await?;
    Ok(no_store(json!({ "card": card })))
}

pub(crate) async fn insert_card(
    db: &Db,
    feed_id: &str,
    author: &str,
    ui: &Value,
    reducers: &Value,
    module: Option<String>,
    at: i64,
) -> Result<String> {
    let id = new_id();
    sqlx::query(
        "INSERT INTO cards (id, feed_id, author_did, ui, reducers, module, version, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, 1, $7)",
    )
    .bind(&id)
    .bind(feed_id)
    .bind(author)
    .bind(ui.to_string())
    .bind(reducers.to_string())
    .bind(module)
    .bind(at)
    .execute(db)
    .await
    .map_err(internal)?;
    Ok(id)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCard {
    card_id: String,
    ui: Option<Value>,
    reducers: Option<Value>,
}

pub async fn update_card(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<UpdateCard>,
) -> Result<Response> {
    let (feed_id, group_id) = card_place(&state.db, &body.card_id, &user.did).await?;
    if let Some(ui) = &body.ui {
        check_ui(ui)?;
        sqlx::query("UPDATE cards SET ui = $1, version = version + 1 WHERE id = $2")
            .bind(ui.to_string())
            .bind(&body.card_id)
            .execute(&state.db)
            .await
            .map_err(internal)?;
    }
    if let Some(reducers) = &body.reducers {
        sqlx::query("UPDATE cards SET reducers = $1, version = version + 1 WHERE id = $2")
            .bind(reducers.to_string())
            .bind(&body.card_id)
            .execute(&state.db)
            .await
            .map_err(internal)?;
    }
    state.events.send(Change::Card { group_id, feed_id, card_id: body.card_id.clone() }).ok();
    let card = load_card(&state.db, &body.card_id, &user.did, &user.handle).await?;
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
    let (feed_id, group_id) = card_place(&state.db, &body.card_id, &user.did).await?;
    for sql in ["DELETE FROM card_actions WHERE card_id = $1", "DELETE FROM cards WHERE id = $1"] {
        sqlx::query(sql).bind(&body.card_id).execute(&state.db).await.map_err(internal)?;
    }
    state.events.send(Change::CardRemoved { group_id, feed_id, card_id: body.card_id }).ok();
    Ok(no_store(json!({})))
}

/// An action, as a generic payload from any block on any card.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    card_id: String,
    block_id: String,
    action_id: String,
    #[serde(default)]
    value: Value,
    #[serde(default)]
    item: Value,
}

pub async fn act(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<Action>,
) -> Result<Response> {
    let (feed_id, group_id) = card_place(&state.db, &body.card_id, &user.did).await?;
    if body.action_id.is_empty() || body.action_id.len() > 64 || body.block_id.len() > 64 {
        return Err(bad("An action needs an action id of up to 64 characters."));
    }
    let value = body.value.to_string();
    if value.len() > 10_000 {
        return Err(bad("That's too much to send."));
    }
    arrive(&state, &user).await?;
    record_action(
        &state.db,
        &body.card_id,
        &user.did,
        &body.block_id,
        &body.action_id,
        &value,
        &body.item,
        now_ms(),
    )
    .await?;
    state.events.send(Change::Card { group_id, feed_id, card_id: body.card_id.clone() }).ok();
    let card = load_card(&state.db, &body.card_id, &user.did, &user.handle).await?;
    Ok(no_store(json!({ "card": card })))
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn record_action(
    db: &Db,
    card_id: &str,
    did: &str,
    block_id: &str,
    action_id: &str,
    value: &str,
    item: &Value,
    at: i64,
) -> Result<String> {
    let id = new_id();
    sqlx::query(
        "INSERT INTO card_actions (id, card_id, did, block_id, action_id, value, item, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(&id)
    .bind(card_id)
    .bind(did)
    .bind(block_id)
    .bind(action_id)
    .bind(value)
    .bind(item.to_string())
    .bind(at)
    .execute(db)
    .await
    .map_err(internal)?;
    Ok(id)
}

pub async fn get_module(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(q): Query<IdQuery>,
) -> Result<Response> {
    card_place(&state.db, &q.id, &user.did).await?;
    let row: Option<(Option<String>,)> = sqlx::query_as("SELECT module FROM cards WHERE id = $1")
        .bind(&q.id)
        .fetch_optional(&state.db)
        .await
        .map_err(internal)?;
    let Some((Some(b64),)) = row else { return Err(not_found()) };
    let bytes = STANDARD.decode(b64).map_err(|_| not_found())?;
    let mut res = bytes.into_response();
    let headers = res.headers_mut();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/wasm"));
    // The URL carries the card's version, so a cached module is never stale.
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("private, max-age=86400"));
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
