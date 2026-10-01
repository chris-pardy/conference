use std::fs;
use std::path::{Path, PathBuf};

use conference_blocks::validate_card;
use serde_json::Value;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The JSON files in `dir` whose names end in `suffix`, sorted by name.
fn json_files(dir: &Path, suffix: &str) -> Vec<(String, Value)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<(String, Value)> = entries
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.to_string_lossy().ends_with(suffix))
        .map(|path| {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let json = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
            (name, json)
        })
        .collect();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

#[test]
fn tc_1_every_gallery_card_is_a_valid_card_record() {
    let cards = json_files(&repo().join("web/src/blocks/gallery"), ".card.json");
    assert!(
        !cards.is_empty(),
        "the gallery should ship sample cards in web/src/blocks/gallery/*.card.json"
    );
    for (name, json) in cards {
        assert_eq!(validate_card(&json["card"]), Ok(()), "{name}");
    }
}

#[test]
fn tc_2_invalid_cards_are_rejected_for_the_shared_reason() {
    let fixtures = json_files(&repo().join("tests/fixtures/cards/invalid"), ".json");
    assert_eq!(fixtures.len(), 3);
    for (name, json) in fixtures {
        let err = validate_card(&json["card"])
            .expect_err(&format!("{name}: {} should be rejected", json["description"]));
        assert_eq!(
            (err.path.as_str(), err.reason.as_str()),
            (json["error"]["path"].as_str().unwrap(), json["error"]["reason"].as_str().unwrap()),
            "{name}"
        );
        assert!(!err.message.is_empty(), "{name} should explain itself");
    }
}
