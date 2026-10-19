//! The demo group everyone who signs in joins: AtmosphereConf 2027, with a
//! couple of feeds of sample cards and a few attendees' worth of activity.

use std::collections::HashMap;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use crate::db::{Db, now_ms};

pub const DEMO_GROUP: &str = "atmosphereconf";

const SEED: &str = include_str!("../../demo/seed.json");
const DICE: &[u8] = include_bytes!("../../demo/dice.wasm");
const QUIZ: &[u8] = include_bytes!("../../demo/quiz.wasm");

fn module(name: &str) -> Option<String> {
    match name {
        "dice" => Some(STANDARD.encode(DICE)),
        "quiz" => Some(STANDARD.encode(QUIZ)),
        _ => None,
    }
}

/// Creates the demo group, unless it's there already.
pub async fn ensure(db: &Db) -> Result<(), String> {
    let err = |e: sqlx::Error| format!("could not seed the demo group: {e}");
    let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM groups WHERE id = $1")
        .bind(DEMO_GROUP)
        .fetch_optional(db)
        .await
        .map_err(err)?;
    if exists.is_some() {
        return Ok(());
    }
    let seed: Value = serde_json::from_str(SEED).map_err(|e| format!("demo/seed.json: {e}"))?;
    let now = now_ms();
    let group = &seed["group"];
    sqlx::query(
        "INSERT INTO groups (id, name, description, invite, owner_did, created_at) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(DEMO_GROUP)
    .bind(group["name"].as_str().unwrap_or("Demo"))
    .bind(group["description"].as_str().unwrap_or(""))
    .bind(group["invite"].as_str().unwrap_or("DEMO"))
    .bind("")
    .bind(now)
    .execute(db)
    .await
    .map_err(err)?;

    for person in seed["people"].as_array().into_iter().flatten() {
        let did = person["did"].as_str().unwrap_or_default();
        sqlx::query(
            "INSERT INTO profiles (did, handle, display_name, avatar, updated_at) VALUES ($1, $2, $3, NULL, $4) \
             ON CONFLICT (did) DO NOTHING",
        )
        .bind(did)
        .bind(person["handle"].as_str().unwrap_or(did))
        .bind(person["displayName"].as_str())
        .bind(now)
        .execute(db)
        .await
        .map_err(err)?;
        sqlx::query(
            "INSERT INTO group_members (group_id, did, role, joined_at) VALUES ($1, $2, 'member', $3) \
             ON CONFLICT (group_id, did) DO NOTHING",
        )
        .bind(DEMO_GROUP)
        .bind(did)
        .bind(now)
        .execute(db)
        .await
        .map_err(err)?;
    }

    let fail = |_| "could not seed the demo group".to_owned();
    for (f, feed) in seed["feeds"].as_array().into_iter().flatten().enumerate() {
        let feed_id = format!("{DEMO_GROUP}-{}", feed["slug"].as_str().unwrap_or("feed"));
        sqlx::query("INSERT INTO feeds (id, group_id, name, position, created_at) VALUES ($1, $2, $3, $4, $5)")
            .bind(&feed_id)
            .bind(DEMO_GROUP)
            .bind(feed["name"].as_str().unwrap_or("Feed"))
            .bind(f as i64)
            .bind(now)
            .execute(db)
            .await
            .map_err(err)?;
        let cards = feed["cards"].as_array().cloned().unwrap_or_default();
        let count = cards.len() as i64;
        for (c, card) in cards.iter().enumerate() {
            // The feed shows newest first, so the first card is the newest.
            let at = now - (c as i64 + 1) * 60_000;
            let module = card["module"].as_str().and_then(module);
            let reducers = card.get("reducers").cloned().unwrap_or(json!({}));
            let card_id = super::insert_card(db, &feed_id, "", &card["ui"], &reducers, module, at)
                .await
                .map_err(fail)?;
            let mut named: HashMap<String, String> = HashMap::new();
            for (a, action) in card["actions"].as_array().into_iter().flatten().enumerate() {
                let item = match action["on"].as_str().and_then(|n| named.get(n)) {
                    Some(id) => json!([{ "key": id }]),
                    None => Value::Null,
                };
                let at = now - (count + 1) * 60_000 + a as i64 * 1_000;
                let id = super::record_action(
                    db,
                    &card_id,
                    action["did"].as_str().unwrap_or_default(),
                    action["blockId"].as_str().unwrap_or_default(),
                    action["actionId"].as_str().unwrap_or_default(),
                    &action["value"].to_string(),
                    &item,
                    at,
                )
                .await
                .map_err(fail)?;
                if let Some(name) = action["as"].as_str() {
                    named.insert(name.to_owned(), id);
                }
            }
        }
    }
    Ok(())
}
