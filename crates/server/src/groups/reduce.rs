//! Folds a card's actions into the data its blocks bind to.
//!
//! Actions arrive as generic payloads: a block id, an action id, a value and
//! an optional list item. Each action id has a reducer, named in the card's
//! `reducers` or guessed from the block that sent it:
//!
//! - `vote`: one value per person, the latest wins (buttons, button groups,
//!   selects). Data: `{total, mine, counts, percent, options}`.
//! - `append`: every action is an entry (submits). Data: `{total, items}`.
//! - `upvote:<target>`: toggles a vote on one of `<target>`'s entries, named
//!   by the list item the action came from.
//! - `leaderboard`: each person's best numeric value. Data: `{total, mine, items}`.
//! - `count`: adds up numeric values (or 1). Data: `{total, mine}`.
//! - `none`: the server keeps the action but shows nothing.
//!
//! The data is `{viewer, <actionId>: ...}`, computed for one viewer, so
//! `mine` and `voted` are theirs.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde_json::{Map, Value, json};

const DEFS: &str = "app.eventside.block.defs#";
const MAX_ITEMS: usize = 50;

pub struct Act {
    pub id: String,
    pub did: String,
    pub handle: String,
    pub block_id: String,
    pub action_id: String,
    pub value: Value,
    pub item: Value,
    pub at: i64,
}

/// The reducer for an action id: the card's own choice, else a guess from
/// the type of the block that sent it.
fn reducer(ui: &Value, reducers: &Value, act: &Act) -> String {
    if let Some(r) = reducers.get(&act.action_id).and_then(Value::as_str) {
        return r.to_owned();
    }
    match block_type(ui, &act.block_id).as_deref() {
        Some("button" | "buttonGroup" | "select") => "vote".into(),
        _ => "append".into(),
    }
}

/// The actions a card's blocks can send: each action id, the type of the
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

/// The type of the block with this id, anywhere in the card.
fn block_type(ui: &Value, id: &str) -> Option<String> {
    fn walk(v: &Value, id: &str) -> Option<String> {
        match v {
            Value::Object(o) => {
                if o.get("id").and_then(Value::as_str) == Some(id)
                    && let Some(t) = o.get("$type").and_then(Value::as_str)
                {
                    return Some(t.strip_prefix(DEFS).unwrap_or(t).to_owned());
                }
                o.values().find_map(|v| walk(v, id))
            }
            Value::Array(a) => a.iter().find_map(|v| walk(v, id)),
            _ => None,
        }
    }
    walk(ui, id)
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

pub fn data(
    ui: &Value,
    reducers: &Value,
    acts: &[Act],
    viewer: &str,
    viewer_handle: &str,
) -> Value {
    let declared = declared_actions(ui);
    let mut kinds: BTreeMap<String, String> = BTreeMap::new();
    // Every action the card can send has data from the start, so bindings
    // show zeros and empty lists rather than "unavailable".
    for (action_id, (block_type, _)) in &declared {
        let kind = match reducers.get(action_id).and_then(Value::as_str) {
            Some(r) => r.to_owned(),
            None if matches!(block_type.as_str(), "button" | "buttonGroup" | "select") => {
                "vote".into()
            }
            None => "append".into(),
        };
        kinds.insert(action_id.clone(), kind);
    }
    if let Some(named) = reducers.as_object() {
        for (action_id, kind) in named {
            if let Some(kind) = kind.as_str() {
                kinds.insert(action_id.clone(), kind.to_owned());
            }
        }
    }
    for act in acts {
        kinds.entry(act.action_id.clone()).or_insert_with(|| reducer(ui, reducers, act));
    }
    // Upvotes first, so the lists they target can be sorted by them.
    let mut upvotes: HashMap<String, HashMap<String, HashSet<String>>> = HashMap::new();
    for act in acts {
        if let Some(target) = kinds[&act.action_id].strip_prefix("upvote:")
            && let Some(key) = item_key(&act.item)
        {
            let voters = upvotes.entry(target.to_owned()).or_default().entry(key).or_default();
            if !voters.remove(&act.did) {
                voters.insert(act.did.clone());
            }
        }
    }

    let mut out = Map::new();
    out.insert("viewer".into(), json!({ "did": viewer, "handle": viewer_handle }));
    for (action_id, kind) in &kinds {
        let mine = acts.iter().filter(|a| &a.action_id == action_id);
        let value = match kind.as_str() {
            "vote" => vote(mine, viewer, declared.get(action_id).map_or(&[][..], |d| &d.1[..])),
            "leaderboard" => leaderboard(mine, viewer),
            "count" => count(mine, viewer),
            "none" => continue,
            k if k.starts_with("upvote:") => continue,
            _ => append(mine, viewer, upvotes.get(action_id)),
        };
        out.insert(action_id.clone(), value);
    }
    Value::Object(out)
}

fn vote<'a>(acts: impl Iterator<Item = &'a Act>, viewer: &str, options: &[String]) -> Value {
    let mut latest: HashMap<&str, String> = HashMap::new();
    for act in acts {
        let v = text_of(&act.value);
        if v.is_empty() {
            latest.remove(act.did.as_str());
        } else {
            latest.insert(&act.did, v);
        }
    }
    let total = latest.len() as u64;
    let mut counts: BTreeMap<String, u64> = options.iter().map(|o| (o.clone(), 0)).collect();
    for v in latest.values() {
        *counts.entry(v.clone()).or_default() += 1;
    }
    let percent = |n: u64| if total == 0 { 0 } else { (n * 100 + total / 2) / total };
    let mine = latest.get(viewer).cloned();
    let mut options: Vec<(&String, &u64)> = counts.iter().collect();
    options.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    json!({
        "total": total,
        "mine": mine,
        "counts": counts,
        "percent": counts.iter().map(|(k, n)| (k.clone(), json!(percent(*n)))).collect::<Map<_, _>>(),
        "options": options.iter().map(|(value, n)| json!({
            "value": value, "count": n, "percent": percent(**n),
            "mine": mine.as_deref() == Some(value.as_str()),
        })).collect::<Vec<_>>(),
    })
}

fn append<'a>(
    acts: impl Iterator<Item = &'a Act>,
    viewer: &str,
    upvotes: Option<&HashMap<String, HashSet<String>>>,
) -> Value {
    let mut items: Vec<(i64, u64, Value)> = acts
        .map(|act| {
            let voters = upvotes.and_then(|u| u.get(&act.id));
            let votes = voters.map_or(0, |v| v.len() as u64);
            let mut item = json!({
                "id": act.id, "did": act.did, "handle": act.handle, "at": act.at,
                "value": text_of(&act.value), "votes": votes,
                "voted": voters.is_some_and(|v| v.contains(viewer)),
                "mine": act.did == viewer,
            });
            // A submitted form's fields are there by name too.
            if let Value::Object(fields) = &act.value {
                for (k, v) in fields {
                    item.as_object_mut().expect("an object").entry(k.clone()).or_insert(v.clone());
                }
            }
            (act.at, votes, item)
        })
        .collect();
    // Most upvoted first, then newest.
    items.sort_by(|a, b| b.1.cmp(&a.1).then(b.0.cmp(&a.0)));
    let total = items.len();
    items.truncate(MAX_ITEMS);
    json!({ "total": total, "items": items.into_iter().map(|i| i.2).collect::<Vec<_>>() })
}

fn leaderboard<'a>(acts: impl Iterator<Item = &'a Act>, viewer: &str) -> Value {
    let mut best: HashMap<&str, (f64, &Act)> = HashMap::new();
    for act in acts {
        let Some(n) = number_of(&act.value) else { continue };
        let entry = best.entry(&act.did).or_insert((n, act));
        if n > entry.0 {
            *entry = (n, act);
        }
    }
    let mine = best.get(viewer).map(|(n, _)| *n);
    let mut items: Vec<(f64, &Act)> = best.into_values().collect();
    items.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.at.cmp(&b.1.at)));
    let total = items.len();
    json!({
        "total": total,
        "mine": mine.map(fmt_number),
        "items": items.iter().take(MAX_ITEMS).enumerate().map(|(rank, (n, act))| json!({
            "id": act.did, "did": act.did, "handle": act.handle, "rank": rank + 1,
            "score": n, "value": fmt_number(*n), "mine": act.did == viewer,
        })).collect::<Vec<_>>(),
    })
}

fn count<'a>(acts: impl Iterator<Item = &'a Act>, viewer: &str) -> Value {
    let (mut total, mut mine) = (0.0, 0.0);
    for act in acts {
        let n = number_of(&act.value).unwrap_or(1.0);
        total += n;
        if act.did == viewer {
            mine += n;
        }
    }
    json!({ "total": fmt_number(total), "mine": fmt_number(mine) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn act(id: &str, did: &str, action: &str, value: Value, item: Value) -> Act {
        Act {
            id: id.into(),
            did: did.into(),
            handle: format!("{did}.test"),
            block_id: action.into(),
            action_id: action.into(),
            value,
            item,
            at: id.len() as i64,
        }
    }

    #[test]
    fn votes_keep_each_persons_latest_choice() {
        let ui = json!({ "blocks": [{ "$type": "app.eventside.block.defs#buttonGroup", "id": "poll" }] });
        let acts = [
            act("1", "a", "poll", json!("x"), Value::Null),
            act("2", "a", "poll", json!("y"), Value::Null),
            act("3", "b", "poll", json!("y"), Value::Null),
        ];
        let d = data(&ui, &json!({}), &acts, "a", "a.test");
        assert_eq!(d["poll"]["total"], 2);
        assert_eq!(d["poll"]["counts"]["y"], 2);
        assert_eq!(d["poll"]["percent"]["y"], 100);
        assert_eq!(d["poll"]["mine"], "y");
    }

    #[test]
    fn upvotes_toggle_and_sort_entries() {
        let reducers = json!({ "ask": "append", "up": "upvote:ask" });
        let on = |k: &str| json!([{ "key": k }]);
        let acts = [
            act("q1", "a", "ask", json!({ "question": "first" }), Value::Null),
            act("q22", "b", "ask", json!({ "question": "second" }), Value::Null),
            act("u1", "a", "up", json!(null), on("q22")),
            act("u2", "c", "up", json!(null), on("q22")),
            act("u3", "c", "up", json!(null), on("q1")),
            act("u4", "c", "up", json!(null), on("q1")),
        ];
        let d = data(&json!({}), &reducers, &acts, "a", "a.test");
        let items = d["ask"]["items"].as_array().unwrap();
        assert_eq!(items[0]["question"], "second");
        assert_eq!(items[0]["votes"], 2);
        assert_eq!(items[0]["voted"], true);
        assert_eq!(items[1]["votes"], 0);
    }
}
