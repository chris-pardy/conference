//! The demo: AtmosphereConf 2027 with its feeds and plans inside it, and a
//! meetup, from `demo/seed.json`. Seeded actions go through the cards' real
//! server reducers, so the shared state is what live use would have built.

use std::collections::HashMap;

use serde_json::{Map, Value, json};

use super::{EventRow, NewCard, apply, create, insert_card, insert_feed, put_rsvp, runtime};
use crate::AppState;
use crate::db::now_ms;

const SEED: &str = include_str!("../../demo/seed.json");

/// Epoch ms of an RFC 3339 time with an offset, like `2027-04-29T09:00:00+02:00`.
fn parse_time(s: &str) -> Option<i64> {
    let num = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, min, sec) = (num(11..13)?, num(14..16)?, num(17..19)?);
    let offset = match s.get(19..)? {
        "Z" => 0,
        o => {
            let sign = if o.starts_with('-') { -1 } else { 1 };
            sign * (o.get(1..3)?.parse::<i64>().ok()? * 60 + o.get(4..6)?.parse::<i64>().ok()?)
        }
    };
    // Days since 1970-01-01 (Howard Hinnant's days_from_civil).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(((days * 86_400 + h * 3600 + min * 60 + sec) - offset * 60) * 1000)
}

/// Stores the built-in modules and, unless it's there already, the demo.
pub async fn ensure(state: &AppState) -> Result<(), String> {
    let db = &state.db;
    for name in ["std", "dice", "quiz", "event"] {
        let bytes = runtime::builtin(name).expect("a built-in module");
        runtime::store(db, bytes)
            .await
            .map_err(|e| format!("could not store the {name} module: {e}"))?;
    }
    let exists: Option<(String,)> =
        sqlx::query_as("SELECT id FROM events WHERE id = 'atmosphereconf'")
            .fetch_optional(db)
            .await
            .map_err(|e| format!("could not check for the demo: {e}"))?;
    if exists.is_some() {
        return Ok(());
    }
    seed(state).await.map_err(|e| format!("could not seed the demo: {e}"))
}

async fn seed(state: &AppState) -> Result<(), String> {
    let db = &state.db;
    let failed = |what: &str| format!("{what} failed");
    let seed: Value = serde_json::from_str(SEED).map_err(|e| format!("demo/seed.json: {e}"))?;
    let now = now_ms();

    let mut people: HashMap<String, Value> = HashMap::new();
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
        .map_err(|e| e.to_string())?;
        people.insert(
            did.to_owned(),
            json!({ "did": did, "handle": person["handle"], "displayName": person["displayName"] }),
        );
    }

    // Parents come before their sub-events in the file, so their feeds exist
    // when a sub-event's card is posted.
    let mut feed_ids: HashMap<(String, String), String> = HashMap::new();
    for e in seed["events"].as_array().into_iter().flatten() {
        let str_of = |k: &str| e[k].as_str().unwrap_or_default().to_owned();
        let event = EventRow {
            id: str_of("id"),
            parent_id: e["parent"].as_str().map(str::to_owned),
            title: str_of("title"),
            description: str_of("description"),
            starts_at: parse_time(e["startsAt"].as_str().unwrap_or_default()).unwrap_or(now),
            ends_at: e["endsAt"].as_str().and_then(parse_time),
            time_zone: e["timeZone"].as_str().unwrap_or("Europe/Amsterdam").to_owned(),
            location: str_of("location"),
            host: str_of("host"),
        };
        let post_to = match (&event.parent_id, e["postTo"].as_str()) {
            (Some(parent), Some(slug)) => feed_ids.get(&(parent.clone(), slug.to_owned())).cloned(),
            _ => None,
        };

        // The event's own feeds first, so its cards are older than the
        // card it gets in its parent.
        let feeds = e["feeds"].as_array().cloned().unwrap_or_default();
        create(state, &event, post_to.as_deref(), feeds.is_empty())
            .await
            .map_err(|_| failed("creating an event"))?;
        for status in ["going", "maybe"] {
            for did in e[status].as_array().into_iter().flatten().filter_map(Value::as_str) {
                put_rsvp(db, &event.id, did, status).await.map_err(|_| failed("an RSVP"))?;
            }
        }
        for feed in &feeds {
            let slug = feed["slug"].as_str().unwrap_or("feed").to_owned();
            let feed_id = insert_feed(db, &event.id, feed["name"].as_str().unwrap_or("Feed"))
                .await
                .map_err(|_| failed("a feed"))?;
            feed_ids.insert((event.id.clone(), slug), feed_id.clone());
            let cards = feed["cards"].as_array().cloned().unwrap_or_default();
            for (c, card) in cards.iter().enumerate() {
                // The feed shows newest first, so the first card is the newest.
                let at = now - (c as i64 + 1) * 60_000;
                let module_hash = runtime::builtin_hash(card["module"].as_str().unwrap_or("std"));
                let config = card.get("config").cloned().unwrap_or(json!({}));
                let card_id = insert_card(
                    state,
                    NewCard {
                        feed_id: &feed_id,
                        author: &event.host,
                        ui: &card["ui"],
                        config: &config,
                        module_hash: &module_hash,
                        event_id: None,
                        at,
                    },
                )
                .await
                .map_err(|_| failed("a card"))?;
                let mut named: HashMap<String, String> = HashMap::new();
                for action in card["actions"].as_array().into_iter().flatten() {
                    let did = action["did"].as_str().unwrap_or_default();
                    let who = people.get(did).cloned().unwrap_or(json!({ "did": did }));
                    let mut meta = Map::new();
                    if let Some(block) = action["blockId"].as_str() {
                        meta.insert("blockId".into(), json!(block));
                    }
                    if let Some(id) = action["on"].as_str().and_then(|n| named.get(n)) {
                        meta.insert("item".into(), json!([{ "key": id }]));
                    }
                    let kind = action["type"].as_str().unwrap_or_default();
                    let id = apply(
                        state,
                        &card_id,
                        &who,
                        kind,
                        action["payload"].clone(),
                        meta,
                        "client",
                    )
                    .await
                    .map_err(|_| failed("a seeded action"))?;
                    if let Some(name) = action["as"].as_str() {
                        named.insert(name.to_owned(), id);
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_time;

    #[test]
    fn parses_times_with_offsets() {
        assert_eq!(parse_time("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_time("1970-01-01T02:00:00+02:00"), Some(0));
        assert_eq!(parse_time("2027-04-29T09:00:00+02:00"), Some(1_808_982_000_000));
    }
}
