//! A card read with `Card::from_record` and written back is the same record.

use conference_blocks::{Card, validate_card};
use serde_json::json;

#[test]
fn tc_1_a_card_round_trips_with_unknown_fields_and_blocks() {
    let record = json!({
        "$type": "app.eventside.block.card",
        "blocks": [
            { "$type": "app.eventside.block.defs#header", "text": "Welkom" },
            { "$type": "app.eventside.future#hologram", "size": 3 }
        ],
        "sources": [{
            "name": "me",
            "ref": { "$type": "app.eventside.block.defs#profileSource", "did": "did:plc:alice" },
            "futureSourceField": true
        }],
        "timeZone": "Europe/Amsterdam",
        "createdAt": "2027-04-30T07:00:00.000Z",
        "futureField": { "pinned": true }
    });
    let card = Card::from_record(&record).expect("a valid card");
    let written = serde_json::to_value(&card).unwrap();
    assert_eq!(written, record);
    assert_eq!(validate_card(&written), Ok(()));
}

#[test]
fn tc_1_a_card_without_optional_fields_round_trips() {
    let record = json!({
        "$type": "app.eventside.block.card",
        "blocks": [{ "$type": "app.eventside.block.defs#divider" }],
        "createdAt": "2027-04-30T07:00:00.000Z"
    });
    let written = serde_json::to_value(Card::from_record(&record).unwrap()).unwrap();
    assert_eq!(written, record);
}
