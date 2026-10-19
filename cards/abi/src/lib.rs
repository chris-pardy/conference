//! The card module ABI (v2), and helpers for building card-kit blocks.
//!
//! A card is redux: `(state, action) -> (state', actions[])`. The same
//! module runs in the browser (`env: "client"`) and on the server
//! (`env: "server"`); see DEMO.md for the whole model.
//!
//! A module is a wasm bundle with four exports:
//!
//! - `memory`
//! - `alloc(len: i32) -> i32`: room for the host to write the input
//! - `reduce(ptr: i32, len: i32) -> i64`
//! - `view(ptr: i32, len: i32) -> i64`
//!
//! Both take JSON and return JSON packed as `(ptr << 32) | len`. There are
//! no imports: the host passes `seed` (u32) and `now` (epoch ms).
//!
//! `reduce` gets `{env, state, shared, action, config, base, context, viewer, seed, now}`:
//! `state` is this device's local state on the client and the shared state
//! on the server; `action` is `{type, payload, meta: {id, actor, at, origin,
//! blockId?, item?}}`. It returns `{state?, actions?}`: a missing `state`
//! means unchanged. Actions the client emits go to the server; actions the
//! server emits go to clients (everyone, or `to` one did), except types
//! starting with `@`, which are platform effects.
//!
//! `view` gets `{shared, local, config, base, context, viewer, now}` and
//! returns `{ui?, data?}`: the card-kit card to show (default: `base`) and
//! what the card's `data` source binds to.
//!
//! Use [`card_module!`] to export `reduce` and `view` functions of type
//! `fn(&Value) -> Value`.

pub use serde_json::{Map, Value, json};

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

/// Exports `alloc`, `reduce` and `view`:
/// `card_module! { reduce: my_reduce, view: my_view }`.
#[macro_export]
macro_rules! card_module {
    (reduce: $reduce:path, view: $view:path $(,)?) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn alloc(len: i32) -> i32 {
            $crate::alloc(len)
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn reduce(ptr: i32, len: i32) -> i64 {
            $crate::call(ptr, len, $reduce)
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn view(ptr: i32, len: i32) -> i64 {
            $crate::call(ptr, len, $view)
        }
    };
}

/// Where the reducer runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Env {
    Client,
    Server,
}

/// The reducer's environment: `Server` when the host says so, else `Client`.
pub fn env(input: &Value) -> Env {
    if input["env"] == "server" { Env::Server } else { Env::Client }
}

/// The action's type, e.g. "roll" or "@init".
pub fn action_type(input: &Value) -> &str {
    input["action"]["type"].as_str().unwrap_or("")
}

/// The action's payload.
pub fn payload(input: &Value) -> &Value {
    &input["action"]["payload"]
}

/// The action's meta: `{id, actor, at, origin, blockId?, item?}`.
pub fn meta(input: &Value) -> &Value {
    &input["action"]["meta"]
}

/// Who dispatched the action (stamped by the server, so trusted there).
pub fn actor_did(input: &Value) -> &str {
    meta(input)["actor"]["did"].as_str().unwrap_or("")
}

/// The actor's handle, falling back to their did.
pub fn actor_handle(input: &Value) -> &str {
    meta(input)["actor"]["handle"].as_str().unwrap_or_else(|| actor_did(input))
}

/// The viewer's did (client side).
pub fn viewer_did(input: &Value) -> &str {
    input["viewer"]["did"].as_str().unwrap_or("")
}

/// The state, or `default` when it is null (before `@init`) or not an object.
pub fn state_or(input: &Value, default: Value) -> Value {
    match &input["state"] {
        s @ Value::Object(_) => s.clone(),
        _ => default,
    }
}

/// An action to emit.
pub fn emit(kind: &str, payload: Value) -> Value {
    json!({ "type": kind, "payload": payload })
}

/// An action the server sends to one person only.
pub fn emit_to(did: &str, kind: &str, payload: Value) -> Value {
    json!({ "type": kind, "payload": payload, "to": did })
}

/// A reduce output: the new state and the actions to emit.
pub fn out(state: Value, actions: Vec<Value>) -> Value {
    json!({ "state": state, "actions": actions })
}

/// A reduce output that changes nothing.
pub fn unchanged() -> Value {
    json!({})
}

/// A small PRNG seeded from the host's seed.
pub struct Rng(u32);

impl Rng {
    pub fn new(input: &Value) -> Self {
        Self((input["seed"].as_u64().unwrap_or(1) as u32) | 1)
    }
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    /// 1..=n
    pub fn roll(&mut self, n: u32) -> u32 {
        self.next_u32() % n + 1
    }
}

/// Epoch milliseconds as an RFC 3339 UTC datetime, e.g. "2027-04-30T07:30:00.000Z".
pub fn iso(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let millis = ms.rem_euclid(1000);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Civil from days (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{millis:03}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
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

    /// tone: neutral, info, warning, success or danger.
    pub fn badge(text: &str, tone: &str) -> Value {
        json!({ "$type": t("badge"), "text": text, "tone": tone })
    }

    pub fn progress(label: &str, value: u32, max: u32) -> Value {
        json!({ "$type": t("progress"), "label": label, "value": value, "max": max.max(1) })
    }

    /// variant: default, primary or danger.
    pub fn button(id: &str, label: &str, action: &str, value: &str, variant: &str) -> Value {
        json!({
            "$type": t("button"), "id": id, "label": label,
            "action": action, "value": value, "variant": variant,
        })
    }

    /// A button group: `options` are (label, value) pairs, unique by value.
    pub fn choices(id: &str, label: &str, action: &str, options: &[(&str, &str)]) -> Value {
        let buttons: Vec<Value> = options
            .iter()
            .map(|(label, value)| json!({ "label": label, "value": value }))
            .collect();
        json!({ "$type": t("buttonGroup"), "id": id, "label": label, "action": action, "buttons": buttons })
    }

    /// Up to four columns of blocks.
    pub fn columns(columns: Vec<Vec<Value>>) -> Value {
        let columns: Vec<Value> =
            columns.into_iter().map(|blocks| json!({ "blocks": blocks })).collect();
        json!({ "$type": t("columns"), "columns": columns })
    }

    pub fn section(blocks: Vec<Value>) -> Value {
        json!({ "$type": t("section"), "blocks": blocks })
    }

    /// A person, by did (their profile is looked up by the client).
    pub fn person(did: &str) -> Value {
        json!({ "$type": t("person"), "did": did })
    }

    /// A place: a name and an optional detail line.
    pub fn room(name: &str, detail: &str) -> Value {
        let mut b = json!({ "$type": t("room"), "name": name });
        if !detail.is_empty() {
            b["detail"] = json!(detail);
        }
        b
    }

    /// A time, or a countdown to it, with RFC 3339 datetimes (see [`crate::iso`]).
    pub fn time(label: &str, at: &str, end: Option<&str>, countdown: bool) -> Value {
        let mut b = json!({
            "$type": t("time"), "label": label, "at": at,
            "mode": if countdown { "countdown" } else { "time" },
        });
        if let Some(end) = end {
            b["end"] = json!(end);
        }
        b
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_formats_epoch_ms() {
        assert_eq!(iso(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(iso(1_809_070_200_000), "2027-04-30T07:30:00.000Z");
    }
}
