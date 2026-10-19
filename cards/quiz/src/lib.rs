//! A five-question pub quiz. Answers are checked on the device, for
//! instant feedback. Submitting sends the answers, not the score: the
//! server re-scores them with the same code, so nobody can post a score
//! they didn't earn. It keeps the leaderboard and tells the player their
//! verified score (`scored`, to them only).
//!
//! - Local state: `{i, score, answers, picked?, right?, posted?, verified?}`
//! - Shared state: `{best: {did: {score, handle, at}}}`

use eventside_card::blocks::*;
use eventside_card::{
    Env, Value, action_type, actor_did, actor_handle, card_module, emit, emit_to, env, json, meta,
    out, payload, state_or, unchanged,
};

card_module! { reduce: reducer, view: render }

struct Question {
    ask: &'static str,
    options: [&'static str; 3],
    answer: usize,
    why: &'static str,
}

const QUESTIONS: [Question; 5] = [
    Question {
        ask: "What does the AT in AT Protocol stand for?",
        options: ["Authenticated Transfer", "Atmosphere Transport", "Async Timeline"],
        answer: 0,
        why: "Authenticated Transfer: every record is signed by its author's key.",
    },
    Question {
        ask: "What is a DID?",
        options: ["A data index document", "A decentralized identifier", "A distributed inbox"],
        answer: 1,
        why: "A decentralized identifier: the permanent name behind your handle.",
    },
    Question {
        ask: "Where does your data live on the atmosphere?",
        options: ["In the app's database", "On the relay", "In your PDS repo"],
        answer: 2,
        why: "Your personal data server holds your repo; apps read from it.",
    },
    Question {
        ask: "Which of these is Amsterdam's airport?",
        options: ["Schiphol", "Zaventem", "Tegel"],
        answer: 0,
        why: "Schiphol. Zaventem is Brussels, and Tegel has closed.",
    },
    Question {
        ask: "A bitterbal is traditionally dipped in…",
        options: ["Mayonnaise", "Mustard", "Ketchup"],
        answer: 1,
        why: "Mustard, and wait a minute: they're hotter inside than you think.",
    },
];

/// The score for a list of answers, the same on every device and the server.
pub fn score(answers: &[Value]) -> u64 {
    QUESTIONS.iter().zip(answers).filter(|(q, a)| a.as_str() == Some(q.options[q.answer])).count()
        as u64
}

pub fn reducer(input: &Value) -> Value {
    match env(input) {
        Env::Client => client(input),
        Env::Server => server(input),
    }
}

fn fresh() -> Value {
    json!({ "i": 0, "score": 0, "answers": [] })
}

fn client(input: &Value) -> Value {
    let mut local = state_or(input, fresh());
    let from_server = meta(input)["origin"] == "server";
    let i = local["i"].as_u64().unwrap_or(0) as usize;
    match action_type(input) {
        "@init" => out(local, vec![]),
        "answer" if !from_server && i < QUESTIONS.len() && local["picked"].is_null() => {
            let picked = payload(input).as_str().unwrap_or("").to_owned();
            if !local["answers"].is_array() {
                local["answers"] = json!([]);
            }
            local["answers"].as_array_mut().expect("an array").push(json!(picked));
            let answers = local["answers"].as_array().cloned().unwrap_or_default();
            let right = QUESTIONS[i].options[QUESTIONS[i].answer] == picked;
            local["picked"] = json!(picked);
            local["right"] = json!(right);
            local["score"] = json!(score(&answers));
            out(local, vec![])
        }
        "next" if !from_server => {
            local["i"] = json!(i + 1);
            local["picked"] = Value::Null;
            out(local, vec![])
        }
        "restart" if !from_server => out(fresh(), vec![]),
        "submit" if !from_server && i >= QUESTIONS.len() && local["posted"] != json!(true) => {
            local["posted"] = json!(true);
            let answers = local["answers"].clone();
            out(local, vec![emit("submit", json!({ "answers": answers }))])
        }
        "scored" if from_server => {
            local["verified"] = payload(input)["score"].clone();
            out(local, vec![])
        }
        _ => unchanged(),
    }
}

fn server(input: &Value) -> Value {
    let mut shared = state_or(input, json!({ "best": {} }));
    match action_type(input) {
        "@init" => out(shared, vec![]),
        "submit" => {
            let answers = payload(input)["answers"].as_array().cloned().unwrap_or_default();
            let points = score(&answers);
            let did = actor_did(input);
            if did.is_empty() {
                return unchanged();
            }
            if !shared["best"].is_object() {
                shared["best"] = json!({});
            }
            let better = shared["best"][did]["score"].as_u64().is_none_or(|old| points > old);
            if better {
                shared["best"][did] = json!({ "score": points, "handle": actor_handle(input), "at": meta(input)["at"] });
            }
            let total = QUESTIONS.len();
            out(shared, vec![emit_to(did, "scored", json!({ "score": points, "total": total }))])
        }
        _ => unchanged(),
    }
}

pub fn render(input: &Value) -> Value {
    let local = state_or(&json!({ "state": input["local"] }), fresh());
    let i = local["i"].as_u64().unwrap_or(0) as usize;
    let score = local["score"].as_u64().unwrap_or(0) as u32;
    let total = QUESTIONS.len() as u32;
    let mut blocks = vec![header("Pub quiz: the atmosphere edition")];
    if i < QUESTIONS.len() {
        let q = &QUESTIONS[i];
        blocks.push(progress(&format!("Question {} of {total}", i + 1), i as u32, total));
        blocks.push(text(q.ask));
        match local["picked"].as_str() {
            None => {
                let options: Vec<(&str, &str)> = q.options.iter().map(|o| (*o, *o)).collect();
                blocks.push(choices("answer", "Your answer", "answer", &options));
            }
            Some(picked) => {
                if local["right"] == json!(true) {
                    blocks.push(badge(&format!("{picked}: correct!"), "success"));
                } else {
                    blocks.push(badge(&format!("{picked}: not quite"), "danger"));
                }
                blocks.push(small(q.why));
                let label = if i + 1 == QUESTIONS.len() { "See my score" } else { "Next question" };
                blocks.push(button("next", label, "next", "", "primary"));
            }
        }
        blocks.push(small(&format!("Score so far: {score}. Answers are checked on your device.")));
    } else {
        blocks.push(stat("Your score", &format!("{score} / {total}")));
        if local["posted"] == json!(true) {
            blocks.push(badge("Score posted", "success"));
            match local["verified"].as_u64() {
                Some(v) => blocks.push(small(&format!("Verified by the server: {v} of {total} ✓"))),
                None => blocks.push(small("Checking your answers on the server…")),
            }
        } else {
            blocks.push(button("submit", "Post my score", "submit", &score.to_string(), "primary"));
        }
        blocks.push(button("restart", "Play again", "restart", "", "default"));
    }
    blocks.push(divider());
    blocks.push(header("Leaderboard"));
    let mut best: Vec<(String, u64, i64)> = input["shared"]["best"]
        .as_object()
        .map(|b| {
            b.iter()
                .map(|(did, e)| {
                    (did.clone(), e["score"].as_u64().unwrap_or(0), e["at"].as_i64().unwrap_or(0))
                })
                .collect()
        })
        .unwrap_or_default();
    best.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.cmp(&b.2)));
    if best.is_empty() {
        blocks.push(small("No scores yet. Be the first."));
    }
    for (did, points, _) in best.iter().take(10) {
        let who = if did.starts_with("did:") { person(did) } else { small(did) };
        blocks.push(columns(vec![vec![who], vec![stat("points", &points.to_string())]]));
    }
    json!({ "ui": card(&input["base"], blocks) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_server_rescores_the_answers_and_tells_only_the_player() {
        let answers = [
            "Authenticated Transfer",
            "A decentralized identifier",
            "In your PDS repo",
            "Zaventem",
            "Mustard",
        ];
        let o = reducer(&json!({ "env": "server", "state": null, "action": {
            "type": "submit", "payload": { "answers": answers },
            "meta": { "at": 5, "actor": { "did": "did:x:a", "handle": "a" } } } }));
        assert_eq!(o["state"]["best"]["did:x:a"]["score"], 4);
        assert_eq!(
            o["actions"][0],
            json!({ "type": "scored", "to": "did:x:a", "payload": { "score": 4, "total": 5 } })
        );
    }
}
