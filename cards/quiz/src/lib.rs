//! A five-question pub quiz that runs on the device: answers are checked
//! locally and instantly. Only the final score is sent to the group.

use eventside_card::blocks::*;
use eventside_card::{Value, action_id, card_module, event_type, json};

card_module!(handle);

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

pub fn handle(input: &Value) -> Value {
    let mut local = input["local"].clone();
    if !local.is_object() {
        local = json!({ "i": 0, "score": 0 });
    }
    let i = local["i"].as_u64().unwrap_or(0) as usize;
    let score = local["score"].as_u64().unwrap_or(0);
    let forward = match (event_type(input), action_id(input)) {
        ("action", "answer") if i < QUESTIONS.len() && local["picked"].is_null() => {
            let picked = input["event"]["value"].as_str().unwrap_or("");
            let right = QUESTIONS[i].options[QUESTIONS[i].answer] == picked;
            local["picked"] = json!(picked);
            local["right"] = json!(right);
            if right {
                local["score"] = json!(score + 1);
            }
            false
        }
        ("action", "next") => {
            local["i"] = json!(i + 1);
            local["picked"] = Value::Null;
            false
        }
        ("action", "restart") => {
            local = json!({ "i": 0, "score": 0 });
            false
        }
        ("action", "score") => {
            local["posted"] = json!(true);
            true
        }
        ("init", _) => false,
        _ => return json!({}),
    };
    json!({ "ui": render(input, &local), "local": local, "forward": forward })
}

fn render(input: &Value, local: &Value) -> Value {
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
        } else {
            blocks.push(button("score", "Post my score", "score", &score.to_string(), "primary"));
        }
        blocks.push(button("restart", "Play again", "restart", "", "default"));
    }
    blocks.push(divider());
    blocks.push(header("Leaderboard"));
    blocks.push(list(
        "data",
        "/score/items",
        "/id",
        "No scores yet. Be the first.",
        vec![columns(vec![
            vec![bound(json!({ "$type": "app.eventside.block.defs#person" }), "did", "$item", "/did")],
            vec![bound(stat("points", ""), "value", "$item", "/value")],
        ])],
    ));
    card(&input["base"], blocks)
}
