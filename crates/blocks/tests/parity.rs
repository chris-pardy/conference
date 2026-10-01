//! The backend validator against results recorded from `@atproto/lexicon`
//! (web/src/blocks/validate-parity.test.ts checks the same file), so the
//! appview and the PWA accept exactly the same cards.

use std::fs;
use std::path::Path;

use conference_blocks::validate_card;
use serde_json::{Value, json};

#[test]
fn tc_2_the_backend_validator_matches_the_recorded_lexicon_results() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/cards/parity.json");
    let cases: Vec<Value> = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    assert!(cases.len() > 100);
    let mut mismatches = Vec::new();
    for case in &cases {
        let got = match validate_card(&case["card"]) {
            Ok(()) => json!({ "ok": true }),
            Err(err) => json!({ "path": err.path, "reason": err.reason }),
        };
        if got != case["expect"] {
            mismatches.push(format!("{}: expected {}, got {}", case["name"], case["expect"], got));
        }
    }
    assert!(mismatches.is_empty(), "{} mismatches:\n{}", mismatches.len(), mismatches.join("\n"));
}
