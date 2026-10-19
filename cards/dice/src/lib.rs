//! "Who buys the next round?": rolls two dice locally, then posts the roll
//! to the group if you want. Rolling never leaves the device.

use eventside_card::blocks::*;
use eventside_card::{Rng, Value, action_id, card_module, event_type, json};

card_module!(handle);

const FACES: [&str; 6] = ["⚀", "⚁", "⚂", "⚃", "⚄", "⚅"];

fn face(n: u64) -> &'static str {
    FACES[(n.clamp(1, 6) - 1) as usize]
}

pub fn handle(input: &Value) -> Value {
    let mut local = input["local"].clone();
    if !local.is_object() {
        local = json!({ "rolls": 0 });
    }
    match event_type(input) {
        "action" if action_id(input) == "roll" => {
            let mut rng = Rng::new(input);
            local["a"] = json!(rng.roll(6));
            local["b"] = json!(rng.roll(6));
            local["rolls"] = json!(local["rolls"].as_u64().unwrap_or(0) + 1);
            local["posted"] = json!(false);
            json!({ "ui": render(input, &local), "local": local, "forward": false })
        }
        "action" if action_id(input) == "share" => {
            local["posted"] = json!(true);
            json!({ "ui": render(input, &local), "local": local, "forward": true })
        }
        "init" => json!({ "ui": render(input, &local), "local": local }),
        _ => json!({}),
    }
}

fn render(input: &Value, local: &Value) -> Value {
    let mut blocks = vec![
        header("Who buys the next round?"),
        text("Roll two dice. Lowest roll at the table buys the bitterballen."),
    ];
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
    blocks.push(list(
        "data",
        "/share/items",
        "/id",
        "Nobody has posted a roll yet.",
        vec![columns(vec![
            vec![bound(json!({ "$type": "app.eventside.block.defs#person" }), "did", "$item", "/did")],
            vec![bound(json!({ "$type": "app.eventside.block.defs#richText" }), "text", "$item", "/value")],
        ])],
    ));
    card(&input["base"], blocks)
}
