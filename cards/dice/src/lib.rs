//! "Who buys the next round?": rolls two dice on the device. Posting a roll
//! sends it to the server, which keeps everyone's posted rolls and, when
//! someone takes the lowest roll, tells the whole group (`lowest`).
//!
//! - Local state: `{rolls, a?, b?, posted?, notice?}`
//! - Shared state: `{rolls: [{id, did, handle, a, b, total, at}], lowest: {did, handle, total} | null}`

use eventside_card::blocks::*;
use eventside_card::{
    Env, Rng, Value, action_type, actor_did, actor_handle, card_module, emit, env, json, meta, out,
    payload, state_or, unchanged,
};

card_module! { reduce: reducer, view: render }

const FACES: [&str; 6] = ["⚀", "⚁", "⚂", "⚃", "⚄", "⚅"];
const KEPT: usize = 50;

fn face(n: u64) -> &'static str {
    FACES[(n.clamp(1, 6) - 1) as usize]
}

pub fn reducer(input: &Value) -> Value {
    match env(input) {
        Env::Client => client(input),
        Env::Server => server(input),
    }
}

fn client(input: &Value) -> Value {
    let mut local = state_or(input, json!({ "rolls": 0 }));
    let from_server = meta(input)["origin"] == "server";
    match action_type(input) {
        "@init" => out(local, vec![]),
        "roll" if !from_server => {
            let mut rng = Rng::new(input);
            local["a"] = json!(rng.roll(6));
            local["b"] = json!(rng.roll(6));
            local["rolls"] = json!(local["rolls"].as_u64().unwrap_or(0) + 1);
            local["posted"] = json!(false);
            out(local, vec![])
        }
        "share" if !from_server => {
            let (Some(a), Some(b)) = (local["a"].as_u64(), local["b"].as_u64()) else {
                return unchanged();
            };
            if local["posted"] == json!(true) {
                return unchanged();
            }
            local["posted"] = json!(true);
            out(local, vec![emit("share", json!({ "a": a, "b": b }))])
        }
        "lowest" if from_server => {
            let p = payload(input);
            let total = p["total"].as_u64().unwrap_or(0);
            let notice = if p["did"].as_str() == input["viewer"]["did"].as_str() {
                format!("Your {total} is the lowest roll. Your round!")
            } else {
                let who = p["handle"].as_str().unwrap_or("Someone");
                format!("{who} rolled {total}: the lowest so far.")
            };
            local["notice"] = json!(notice);
            out(local, vec![])
        }
        _ => unchanged(),
    }
}

fn server(input: &Value) -> Value {
    let mut shared = state_or(input, json!({ "rolls": [], "lowest": null }));
    match action_type(input) {
        "@init" => out(shared, vec![]),
        "share" => {
            let p = payload(input);
            let (Some(a), Some(b)) = (p["a"].as_u64(), p["b"].as_u64()) else {
                return unchanged();
            };
            let (a, b) = (a.clamp(1, 6), b.clamp(1, 6));
            let total = a + b;
            let (did, handle) = (actor_did(input), actor_handle(input));
            let roll = json!({
                "id": meta(input)["id"], "did": did, "handle": handle,
                "a": a, "b": b, "total": total, "at": meta(input)["at"],
            });
            if !shared["rolls"].is_array() {
                shared["rolls"] = json!([]);
            }
            let rolls = shared["rolls"].as_array_mut().expect("an array");
            rolls.insert(0, roll);
            rolls.truncate(KEPT);
            let lower = shared["lowest"]["total"].as_u64().is_none_or(|low| total < low);
            if lower {
                let lowest = json!({ "did": did, "handle": handle, "total": total });
                shared["lowest"] = lowest.clone();
                return out(shared, vec![emit("lowest", lowest)]);
            }
            out(shared, vec![])
        }
        _ => unchanged(),
    }
}

pub fn render(input: &Value) -> Value {
    let local = &input["local"];
    let shared = &input["shared"];
    let mut blocks = vec![
        header("Who buys the next round?"),
        text("Roll two dice. Lowest roll at the table buys the bitterballen."),
    ];
    if let Some(notice) = local["notice"].as_str() {
        blocks.push(badge(notice, "warning"));
    }
    if let (Some(a), Some(b)) = (local["a"].as_u64(), local["b"].as_u64()) {
        let total = a + b;
        blocks.push(columns(vec![
            vec![stat("Die one", face(a))],
            vec![stat("Die two", face(b))],
            vec![stat("Total", &total.to_string())],
        ]));
        if a == b {
            blocks.push(badge("Doubles! Roll again", "success"));
        } else if total <= 4 {
            blocks.push(badge("Ouch. Your round, probably", "warning"));
        }
        blocks.push(button("roll", "Roll again", "roll", "", "default"));
        if local["posted"] == json!(true) {
            blocks.push(small("Posted to the group."));
        } else {
            let value = format!("{} {}  rolled {total}", face(a), face(b));
            blocks.push(button("share", "Post my roll", "share", &value, "primary"));
        }
    } else {
        blocks.push(button("roll", "Roll the dice", "roll", "", "primary"));
    }
    blocks.push(small(&format!(
        "Rolled {} times on this device. Rolls are local; only posts reach the group.",
        local["rolls"].as_u64().unwrap_or(0)
    )));
    blocks.push(divider());
    blocks.push(header("Posted rolls"));
    if let Some(low) = shared["lowest"]["total"].as_u64() {
        let who = shared["lowest"]["handle"].as_str().unwrap_or("someone");
        blocks.push(small(&format!("Lowest so far: {low}, by {who}.")));
    }
    let rolls = shared["rolls"].as_array().cloned().unwrap_or_default();
    if rolls.is_empty() {
        blocks.push(small("Nobody has posted a roll yet."));
    }
    for roll in rolls.iter().take(10) {
        let (a, b) = (roll["a"].as_u64().unwrap_or(1), roll["b"].as_u64().unwrap_or(1));
        let did = roll["did"].as_str().unwrap_or("");
        let line = format!("{} {}  rolled {}", face(a), face(b), roll["total"]);
        let who = if did.starts_with("did:") { person(did) } else { small(did) };
        blocks.push(columns(vec![vec![who], vec![text(&line)]]));
    }
    json!({ "ui": card(&input["base"], blocks) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn share(state: &Value, did: &str, a: u64, b: u64) -> Value {
        reducer(&json!({ "env": "server", "state": state, "action": {
            "type": "share", "payload": { "a": a, "b": b },
            "meta": { "id": did, "at": 1, "actor": { "did": did, "handle": did } } } }))
    }

    #[test]
    fn the_server_announces_a_new_lowest_roll() {
        let first = share(&Value::Null, "did:x:a", 3, 4);
        assert_eq!(first["actions"][0]["type"], "lowest");
        let higher = share(&first["state"], "did:x:b", 6, 6);
        assert_eq!(higher["actions"], json!([]));
        let lower = share(&higher["state"], "did:x:c", 1, 1);
        assert_eq!(lower["state"]["lowest"]["did"], "did:x:c");
        assert_eq!(lower["state"]["rolls"].as_array().unwrap().len(), 3);
    }
}
