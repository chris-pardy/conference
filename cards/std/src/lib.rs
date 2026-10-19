//! The standard card module: the default for any card without its own.
//!
//! The card's `config` maps action types to reducers; a type it doesn't
//! name is guessed from the block that declares it (buttons, button groups
//! and selects vote; anything else appends).
//!
//! - `vote`: one value per person, the latest wins.
//! - `append`: every action is an entry.
//! - `upvote:<type>`: toggles the actor's vote on one of `<type>`'s
//!   entries, named by the list item the action came from.
//! - `leaderboard`: each person's best numeric value.
//! - `count`: adds up numeric values (or 1).
//! - `none`: ignored.
//!
//! On the client, every ui action is forwarded to the server unchanged. On
//! the server, it's folded into the shared state, one entry per action type
//! (each tagged with its `kind`). The view computes each viewer's data in
//! the shapes card templates bind to.

use std::collections::BTreeMap;

use eventside_card::{
    Env, Map, Value, action_type, actor_did, actor_handle, card_module, env, json, meta, out,
    payload, unchanged,
};

card_module! { reduce: reducer, view: render }

const DEFS: &str = "app.eventside.block.defs#";
const MAX_KEPT: usize = 200;
const MAX_SHOWN: usize = 50;

pub fn reducer(input: &Value) -> Value {
    let kind = action_type(input);
    match (env(input), kind) {
        (Env::Server, "@init") => out(json!({}), vec![]),
        (Env::Client, "@init") => out(Value::Null, vec![]),
        (_, k) if k.starts_with('@') => unchanged(),
        (Env::Client, _) => {
            // Only taps on this device go up; actions from the server stop here.
            if meta(input)["origin"] == "server" {
                return unchanged();
            }
            let m = meta(input);
            let mut up_meta = json!({});
            for key in ["blockId", "item"] {
                if !m[key].is_null() {
                    up_meta[key] = m[key].clone();
                }
            }
            let action = json!({ "type": kind, "payload": payload(input), "meta": up_meta });
            json!({ "actions": [action] })
        }
        (Env::Server, _) => {
            let mut state = match &input["state"] {
                s @ Value::Object(_) => s.clone(),
                _ => json!({}),
            };
            let reducer = reducer_for(&input["config"], &input["base"], kind);
            fold(&mut state, kind, &reducer, input);
            out(state, vec![])
        }
    }
}

/// The reducer for an action type: the config's choice, else a guess from
/// the block that declares it.
fn reducer_for(config: &Value, base: &Value, kind: &str) -> String {
    if let Some(r) = config.get(kind).and_then(Value::as_str) {
        return r.to_owned();
    }
    match declared_actions(base).get(kind).map(|d| d.0.as_str()) {
        Some("button" | "buttonGroup" | "select") => "vote".into(),
        _ => "append".into(),
    }
}

fn fold(state: &mut Value, kind: &str, reducer: &str, input: &Value) {
    let did = actor_did(input).to_owned();
    if did.is_empty() {
        return;
    }
    let handle = actor_handle(input).to_owned();
    let at = meta(input)["at"].as_i64().unwrap_or(0);
    let value = payload(input);

    if let Some(target) = reducer.strip_prefix("upvote:") {
        let Some(key) = item_key(&meta(input)["item"]) else { return };
        let Some(items) = state[target]["items"].as_array_mut() else { return };
        let Some(item) = items.iter_mut().find(|i| i["id"].as_str() == Some(key.as_str())) else {
            return;
        };
        let mut voters: Vec<Value> = item["voters"].as_array().cloned().unwrap_or_default();
        if let Some(at) = voters.iter().position(|v| v == &json!(did)) {
            voters.remove(at);
        } else {
            voters.push(json!(did));
        }
        item["voters"] = Value::Array(voters);
        return;
    }

    let slot = state
        .as_object_mut()
        .expect("an object")
        .entry(kind.to_owned())
        .or_insert_with(|| json!({ "kind": reducer }));
    if !slot.is_object() {
        *slot = json!({ "kind": reducer });
    }
    slot["kind"] = json!(reducer);
    match reducer {
        "vote" => {
            let v = text_of(value);
            let votes = slot.as_object_mut().unwrap().entry("votes").or_insert_with(|| json!({}));
            let votes = votes.as_object_mut().unwrap();
            if v.is_empty() {
                votes.remove(&did);
            } else {
                votes.insert(did, json!(v));
            }
        }
        "leaderboard" => {
            let Some(n) = number_of(value) else { return };
            let best = slot.as_object_mut().unwrap().entry("best").or_insert_with(|| json!({}));
            let better = best[&did]["score"].as_f64().is_none_or(|old| n > old);
            if better {
                best[&did] = json!({ "score": n, "handle": handle, "at": at });
            }
        }
        "count" => {
            let n = number_of(value).unwrap_or(1.0);
            slot["total"] = json!(slot["total"].as_f64().unwrap_or(0.0) + n);
            if !slot["by"].is_object() {
                slot["by"] = json!({});
            }
            slot["by"][&did] = json!(slot["by"][&did].as_f64().unwrap_or(0.0) + n);
        }
        "none" => {}
        _ => {
            let id = meta(input)["id"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("{did}-{at}"));
            let mut item = json!({
                "id": id, "did": did, "handle": handle, "at": at,
                "value": text_of(value), "voters": [],
            });
            // A submitted form's fields are there by name too.
            if let Value::Object(fields) = value {
                for (k, v) in fields {
                    item.as_object_mut().unwrap().entry(k.clone()).or_insert(v.clone());
                }
            }
            if !slot["items"].is_array() {
                slot["items"] = json!([]);
            }
            let items = slot["items"].as_array_mut().unwrap();
            items.push(item);
            if items.len() > MAX_KEPT {
                let extra = items.len() - MAX_KEPT;
                items.drain(..extra);
            }
        }
    }
}

pub fn render(input: &Value) -> Value {
    let shared = &input["shared"];
    let base = &input["base"];
    let config = &input["config"];
    let viewer = input["viewer"]["did"].as_str().unwrap_or("");
    let declared = declared_actions(base);

    // Every action the card can send has data from the start, so bindings
    // show zeros and empty lists rather than "unavailable".
    let mut kinds: BTreeMap<String, String> = BTreeMap::new();
    for kind in declared.keys() {
        kinds.insert(kind.clone(), reducer_for(config, base, kind));
    }
    if let Some(named) = config.as_object() {
        for (kind, r) in named {
            if let Some(r) = r.as_str() {
                kinds.insert(kind.clone(), r.to_owned());
            }
        }
    }
    if let Some(slots) = shared.as_object() {
        for (kind, slot) in slots {
            if let Some(r) = slot["kind"].as_str() {
                kinds.entry(kind.clone()).or_insert_with(|| r.to_owned());
            }
        }
    }

    let mut data = Map::new();
    data.insert("viewer".into(), input["viewer"].clone());
    data.insert("context".into(), input["context"].clone());
    for (kind, reducer) in &kinds {
        let slot = &shared[kind];
        let value = match reducer.as_str() {
            "vote" => {
                let options = declared.get(kind).map_or(&[][..], |d| &d.1[..]);
                vote(slot, viewer, options)
            }
            "leaderboard" => leaderboard(slot, viewer),
            "count" => count(slot, viewer),
            "none" => continue,
            r if r.starts_with("upvote:") => continue,
            _ => append(slot, viewer),
        };
        data.insert(kind.clone(), value);
    }
    json!({ "data": data })
}

fn vote(slot: &Value, viewer: &str, options: &[String]) -> Value {
    let empty = Map::new();
    let votes = slot["votes"].as_object().unwrap_or(&empty);
    let total = votes.len() as u64;
    let mut counts: BTreeMap<String, u64> = options.iter().map(|o| (o.clone(), 0)).collect();
    for v in votes.values().filter_map(Value::as_str) {
        *counts.entry(v.to_owned()).or_default() += 1;
    }
    let percent = |n: u64| if total == 0 { 0 } else { (n * 100 + total / 2) / total };
    let mine = votes.get(viewer).and_then(Value::as_str).map(str::to_owned);
    let mut sorted: Vec<(&String, &u64)> = counts.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    json!({
        "total": total,
        "mine": mine,
        "counts": counts,
        "percent": counts.iter().map(|(k, n)| (k.clone(), json!(percent(*n)))).collect::<Map<_, _>>(),
        "options": sorted.iter().map(|(value, n)| json!({
            "value": value, "count": n, "percent": percent(**n),
            "mine": mine.as_deref() == Some(value.as_str()),
        })).collect::<Vec<_>>(),
    })
}

fn append(slot: &Value, viewer: &str) -> Value {
    let items = slot["items"].as_array().cloned().unwrap_or_default();
    let mut shown: Vec<(i64, u64, Value)> = items
        .into_iter()
        .map(|mut item| {
            let voters = item["voters"].as_array().cloned().unwrap_or_default();
            let votes = voters.len() as u64;
            let o = item.as_object_mut().expect("an item");
            o.remove("voters");
            o.insert("votes".into(), json!(votes));
            o.insert("voted".into(), json!(voters.iter().any(|v| v == viewer)));
            o.insert("mine".into(), json!(o.get("did").and_then(Value::as_str) == Some(viewer)));
            (item["at"].as_i64().unwrap_or(0), votes, item)
        })
        .collect();
    // Most upvoted first, then newest.
    shown.sort_by(|a, b| b.1.cmp(&a.1).then(b.0.cmp(&a.0)));
    let total = shown.len();
    shown.truncate(MAX_SHOWN);
    json!({ "total": total, "items": shown.into_iter().map(|i| i.2).collect::<Vec<_>>() })
}

fn leaderboard(slot: &Value, viewer: &str) -> Value {
    let empty = Map::new();
    let best = slot["best"].as_object().unwrap_or(&empty);
    let mut items: Vec<(&String, f64, i64, &Value)> = best
        .iter()
        .map(|(did, b)| (did, b["score"].as_f64().unwrap_or(0.0), b["at"].as_i64().unwrap_or(0), b))
        .collect();
    items.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.2.cmp(&b.2)));
    let mine = best.get(viewer).and_then(|b| b["score"].as_f64());
    json!({
        "total": items.len(),
        "mine": mine.map(fmt_number),
        "items": items.iter().take(MAX_SHOWN).enumerate().map(|(rank, (did, score, _, b))| json!({
            "id": did, "did": did, "handle": b["handle"], "rank": rank + 1,
            "score": score, "value": fmt_number(*score), "mine": did.as_str() == viewer,
        })).collect::<Vec<_>>(),
    })
}

fn count(slot: &Value, viewer: &str) -> Value {
    let total = slot["total"].as_f64().unwrap_or(0.0);
    let mine = slot["by"][viewer].as_f64().unwrap_or(0.0);
    json!({ "total": fmt_number(total), "mine": fmt_number(mine) })
}

/// The actions a card's blocks can send: each action type, the type of the
/// block that sends it, and the values it offers.
fn declared_actions(ui: &Value) -> BTreeMap<String, (String, Vec<String>)> {
    fn walk(v: &Value, out: &mut BTreeMap<String, (String, Vec<String>)>) {
        match v {
            Value::Object(o) => {
                let t = o
                    .get("$type")
                    .and_then(Value::as_str)
                    .map(|t| t.strip_prefix(DEFS).unwrap_or(t));
                // A select's action is its id.
                let action = o.get("action").and_then(Value::as_str).or(match t {
                    Some("select") => o.get("id").and_then(Value::as_str),
                    _ => None,
                });
                if let (Some(t), Some(action)) = (t, action) {
                    let entry = out.entry(action.to_owned()).or_insert((t.to_owned(), vec![]));
                    let values = o.get("buttons").or(o.get("options")).and_then(Value::as_array);
                    let values = values.into_iter().flatten().filter_map(|b| b.get("value"));
                    let own = o.get("value").filter(|_| t == "button");
                    for value in values.chain(own).filter_map(Value::as_str) {
                        if !entry.1.iter().any(|v| v == value) {
                            entry.1.push(value.to_owned());
                        }
                    }
                }
                o.values().for_each(|v| walk(v, out));
            }
            Value::Array(a) => a.iter().for_each(|v| walk(v, out)),
            _ => {}
        }
    }
    let mut out = BTreeMap::new();
    walk(ui, &mut out);
    out
}

/// The text of a value: a string, or the first string in a submitted form.
fn text_of(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Object(o) => {
            o.values().find_map(|v| v.as_str().map(str::to_owned)).unwrap_or_default()
        }
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

fn number_of(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn fmt_number(n: f64) -> String {
    if n.fract() == 0.0 { format!("{}", n as i64) } else { format!("{n:.1}") }
}

/// The key of the list item an action came from: its innermost step.
fn item_key(item: &Value) -> Option<String> {
    let step = item.as_array()?.last()?;
    match (step.get("key"), step.get("index")) {
        (Some(Value::String(s)), _) => Some(s.clone()),
        (Some(k), _) => Some(k.to_string()),
        (None, Some(i)) => Some(format!("#{i}")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs actions through the server reducer and returns the final state.
    fn server(config: &Value, base: &Value, acts: &[(&str, &str, &str, Value, Value)]) -> Value {
        let mut state =
            reducer(&json!({ "env": "server", "action": { "type": "@init" } }))["state"].clone();
        for (i, (id, did, kind, value, item)) in acts.iter().enumerate() {
            let input = json!({
                "env": "server", "state": state, "config": config, "base": base,
                "action": { "type": kind, "payload": value, "meta": {
                    "id": id, "at": i, "origin": "client", "item": item,
                    "actor": { "did": did, "handle": format!("{did}.test") },
                }},
            });
            let o = reducer(&input);
            if let Some(s) = o.get("state") {
                state = s.clone();
            }
        }
        state
    }

    fn data(config: &Value, base: &Value, shared: &Value, viewer: &str) -> Value {
        render(&json!({
            "shared": shared, "config": config, "base": base,
            "viewer": { "did": viewer, "handle": format!("{viewer}.test") },
        }))["data"]
            .clone()
    }

    #[test]
    fn votes_keep_each_persons_latest_choice() {
        let ui = json!({ "blocks": [{
            "$type": "app.eventside.block.defs#buttonGroup", "id": "poll", "action": "poll",
            "buttons": [{ "label": "X", "value": "x" }, { "label": "Y", "value": "y" }, { "label": "Z", "value": "z" }],
        }] });
        let acts = [
            ("1", "a", "poll", json!("x"), Value::Null),
            ("2", "a", "poll", json!("y"), Value::Null),
            ("3", "b", "poll", json!("y"), Value::Null),
        ];
        let state = server(&json!({}), &ui, &acts);
        let d = data(&json!({}), &ui, &state, "a");
        assert_eq!(d["poll"]["total"], 2);
        assert_eq!(d["poll"]["counts"]["y"], 2);
        assert_eq!(d["poll"]["counts"]["z"], 0);
        assert_eq!(d["poll"]["percent"]["y"], 100);
        assert_eq!(d["poll"]["mine"], "y");
    }

    #[test]
    fn upvotes_toggle_and_sort_entries() {
        let config = json!({ "ask": "append", "up": "upvote:ask" });
        let on = |k: &str| json!([{ "key": k }]);
        let acts = [
            ("q1", "a", "ask", json!({ "question": "first" }), Value::Null),
            ("q22", "b", "ask", json!({ "question": "second" }), Value::Null),
            ("u1", "a", "up", json!(null), on("q22")),
            ("u2", "c", "up", json!(null), on("q22")),
            ("u3", "c", "up", json!(null), on("q1")),
            ("u4", "c", "up", json!(null), on("q1")),
        ];
        let state = server(&config, &json!({}), &acts);
        let d = data(&config, &json!({}), &state, "a");
        let items = d["ask"]["items"].as_array().unwrap();
        assert_eq!(items[0]["question"], "second");
        assert_eq!(items[0]["votes"], 2);
        assert_eq!(items[0]["voted"], true);
        assert_eq!(items[1]["votes"], 0);
        assert!(d.get("up").is_none());
    }

    #[test]
    fn the_client_forwards_taps_and_ignores_server_actions() {
        let tap = json!({ "env": "client", "action": {
            "type": "poll", "payload": "x", "meta": { "origin": "ui", "blockId": "poll" } } });
        let o = reducer(&tap);
        assert_eq!(
            o["actions"][0],
            json!({ "type": "poll", "payload": "x", "meta": { "blockId": "poll" } })
        );
        let down =
            json!({ "env": "client", "action": { "type": "x", "meta": { "origin": "server" } } });
        assert_eq!(reducer(&down), json!({}));
    }

    #[test]
    fn counts_and_leaderboards() {
        let config = json!({ "c": "count", "s": "leaderboard" });
        let acts = [
            ("1", "a", "c", json!("1"), Value::Null),
            ("2", "b", "c", json!(2), Value::Null),
            ("3", "a", "s", json!("3"), Value::Null),
            ("4", "b", "s", json!(5), Value::Null),
            ("5", "a", "s", json!(4), Value::Null),
        ];
        let state = server(&config, &json!({}), &acts);
        let d = data(&config, &json!({}), &state, "a");
        assert_eq!(d["c"], json!({ "total": "3", "mine": "1" }));
        assert_eq!(d["s"]["mine"], "4");
        assert_eq!(d["s"]["items"][0]["did"], "b");
        assert_eq!(d["s"]["items"][1]["rank"], 2);
    }
}
