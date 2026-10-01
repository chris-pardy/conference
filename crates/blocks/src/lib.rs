//! Card records (`app.gather.block.card`): types and lexicon validation.
//!
//! The appview uses this to check cards it's handed before storing or
//! serving them. It validates exactly as the PWA does, against the same
//! lexicons in `lexicons/`.

mod datetime;
mod lexicon;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CARD_NSID: &str = "app.gather.block.card";

/// Why a card failed validation: the JSON pointer of the offending value, and
/// a reason code shared with the frontend validator (`required`,
/// `min-length`, `max-length`, `type`, `format`, …).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardError {
    pub path: String,
    pub reason: String,
    pub message: String,
}

impl std::fmt::Display for CardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CardError {}

/// Validates an `app.gather.block.card` record against the lexicons.
pub fn validate_card(record: &Value) -> Result<(), CardError> {
    let Some(obj) = record.as_object() else {
        return Err(CardError {
            path: String::new(),
            reason: "type".into(),
            message: "a card must be an object".into(),
        });
    };
    if obj.get("$type").and_then(Value::as_str) != Some(CARD_NSID) {
        return Err(CardError {
            path: "/$type".into(),
            reason: "const".into(),
            message: format!("/$type must be {CARD_NSID}"),
        });
    }
    lexicon::check_depth(record)?;
    lexicon::validate_record(&format!("{CARD_NSID}#main"), record)?;
    lexicon::check_card(record)
}

/// A card record. Blocks and sources stay as JSON: the renderer, not the
/// appview, interprets them. A card read and written back is the same
/// record: unknown block types, and fields a newer lexicon adds, survive
/// untouched.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    #[serde(rename = "$type", default = "card_nsid")]
    pub record_type: String,
    pub blocks: Vec<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<Source>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub middleware: Option<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_zone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_text: Option<String>,
    pub created_at: String,
    /// Fields this version doesn't know.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

fn card_nsid() -> String {
    CARD_NSID.to_owned()
}

/// A named source that bindings refer to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Source {
    pub name: String,
    #[serde(rename = "ref")]
    pub source: Value,
    /// Fields this version doesn't know.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

impl Card {
    /// Validates a record and reads it as a card.
    pub fn from_record(record: &Value) -> Result<Card, CardError> {
        validate_card(record)?;
        serde_json::from_value(record.clone()).map_err(|err| CardError {
            path: String::new(),
            reason: "invalid".into(),
            message: err.to_string(),
        })
    }
}
