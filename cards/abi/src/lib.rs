//! The card module ABI, and helpers for building card-kit blocks.
//!
//! A card module is a wasm bundle with three exports:
//!
//! - `memory`
//! - `alloc(len: i32) -> i32`: room for the host to write the input
//! - `update(ptr: i32, len: i32) -> i64`: reads the input JSON and returns
//!   the output JSON, packed as `(ptr << 32) | len`
//!
//! The input is `{event, ui, base, data, local, viewer, seed}`:
//!
//! - `event`: `{type: "init"}`, `{type: "data"}` (the server's data changed)
//!   or `{type: "action", blockId, actionId, value, item?}`
//! - `ui`: the card-kit card on screen now; `base`: the card as authored
//! - `data`: the server's data for this card, as this viewer sees it
//! - `local`: whatever the module returned as `local` last time (null at first)
//! - `seed`: a fresh random u32, since a module has no clock or RNG
//!
//! The output is `{ui?, local?, forward?}`. `ui` replaces what's on screen,
//! `local` is kept for the next call, and `forward` says whether an action
//! also goes to the server: `true` (the default for actions), `false`, or
//! `{actionId?, value?}` to send something else.
//!
//! Use [`card_module!`] to export a function `fn(&Value) -> Value`.

pub use serde_json::{Value, json};

#[doc(hidden)]
pub fn alloc(len: i32) -> i32 {
    let mut buf = Vec::<u8>::with_capacity(len.max(0) as usize);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr as i32
}

#[doc(hidden)]
pub fn call(ptr: i32, len: i32, f: fn(&Value) -> Value) -> i64 {
    // SAFETY: the host wrote `len` bytes at a pointer `alloc` handed out.
    let input = unsafe { Vec::from_raw_parts(ptr as *mut u8, len as usize, len as usize) };
    let input: Value = serde_json::from_slice(&input).unwrap_or(Value::Null);
    let out = serde_json::to_vec(&f(&input)).unwrap_or_else(|_| b"{}".to_vec());
    let out = out.into_boxed_slice();
    let (ptr, len) = (out.as_ptr() as i64, out.len() as i64);
    std::mem::forget(out);
    (ptr << 32) | len
}

/// Exports `alloc` and `update` for a module whose logic is `f`.
#[macro_export]
macro_rules! card_module {
    ($f:path) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn alloc(len: i32) -> i32 {
            $crate::alloc(len)
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn update(ptr: i32, len: i32) -> i64 {
            $crate::call(ptr, len, $f)
        }
    };
}

/// The event's type: "init", "data" or "action".
pub fn event_type(input: &Value) -> &str {
    input["event"]["type"].as_str().unwrap_or("")
}

/// The action id, for an action event.
pub fn action_id(input: &Value) -> &str {
    input["event"]["actionId"].as_str().unwrap_or("")
}

/// A small PRNG seeded from the host's seed.
pub struct Rng(u32);

impl Rng {
    pub fn new(input: &Value) -> Self {
        Self((input["seed"].as_u64().unwrap_or(1) as u32) | 1)
    }
    pub fn next(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    /// 1..=n
    pub fn roll(&mut self, n: u32) -> u32 {
        self.next() % n + 1
    }
}

/// Builders for card-kit blocks.
pub mod blocks {
    use super::{Value, json};

    const DEFS: &str = "app.eventside.block.defs";

    fn t(name: &str) -> String {
        format!("{DEFS}#{name}")
    }

    /// A card from blocks, keeping the authored card's sources and time zone.
    pub fn card(base: &Value, blocks: Vec<Value>) -> Value {
        let mut card = base.clone();
        if !card.is_object() {
            card = json!({ "$type": "app.eventside.block.card", "createdAt": "2027-04-29T08:00:00.000Z" });
        }
        card["blocks"] = Value::Array(blocks);
        card
    }

    pub fn header(text: &str) -> Value {
        json!({ "$type": t("header"), "text": text })
    }

    pub fn text(text: &str) -> Value {
        json!({ "$type": t("richText"), "text": text })
    }

    pub fn small(text: &str) -> Value {
        json!({ "$type": t("context"), "text": text })
    }

    pub fn divider() -> Value {
        json!({ "$type": t("divider") })
    }

    pub fn stat(label: &str, value: &str) -> Value {
        json!({ "$type": t("stat"), "label": label, "value": value })
    }

    pub fn badge(text: &str, tone: &str) -> Value {
        json!({ "$type": t("badge"), "text": text, "tone": tone })
    }

    pub fn progress(label: &str, value: u32, max: u32) -> Value {
        json!({ "$type": t("progress"), "label": label, "value": value, "max": max.max(1) })
    }

    pub fn button(id: &str, label: &str, action: &str, value: &str, variant: &str) -> Value {
        json!({
            "$type": t("button"), "id": id, "label": label,
            "action": action, "value": value, "variant": variant,
        })
    }

    pub fn choices(id: &str, label: &str, action: &str, options: &[(&str, &str)]) -> Value {
        let buttons: Vec<Value> =
            options.iter().map(|(label, value)| json!({ "label": label, "value": value })).collect();
        json!({ "$type": t("buttonGroup"), "id": id, "label": label, "action": action, "buttons": buttons })
    }

    pub fn columns(columns: Vec<Vec<Value>>) -> Value {
        let columns: Vec<Value> = columns.into_iter().map(|blocks| json!({ "blocks": blocks })).collect();
        json!({ "$type": t("columns"), "columns": columns })
    }

    pub fn section(blocks: Vec<Value>) -> Value {
        json!({ "$type": t("section"), "blocks": blocks })
    }

    /// A list over `source` at `path`, repeating `template`.
    pub fn list(source: &str, path: &str, key: &str, empty: &str, template: Vec<Value>) -> Value {
        json!({
            "$type": t("list"), "items": { "source": source, "path": path },
            "key": key, "empty": empty, "template": template,
        })
    }

    /// A block with one property bound to a source.
    pub fn bound(mut block: Value, prop: &str, source: &str, path: &str) -> Value {
        block["bind"][prop] = json!({ "source": source, "path": path });
        block
    }
}
