//! Card records (`app.gather.block.card`): types and lexicon validation.

use serde_json::Value;

/// Why a card failed validation: the JSON pointer of the offending value, and
/// a reason code shared with the frontend validator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardError {
    pub path: String,
    pub reason: String,
    pub message: String,
}

/// Validates an `app.gather.block.card` record against the lexicons.
pub fn validate_card(_record: &Value) -> Result<(), CardError> {
    todo!("not implemented")
}
