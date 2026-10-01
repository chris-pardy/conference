//! A card nested far past the limit is rejected, not a stack overflow.
//! (Built in code: serde_json's parser stops at 128 levels on its own.)

use conference_blocks::validate_card;
use serde_json::{Value, json};

#[test]
fn tc_2_a_card_nested_thousands_deep_is_rejected_without_overflowing() {
    // Built by moving each level into the next: json! would copy `inner`
    // through to_value, which itself recurses 2000 deep.
    let wrap = |inner: Value| {
        let mut stack = json!({ "$type": "app.gather.block.defs#stack" });
        stack["blocks"] = Value::Array(vec![inner]);
        stack
    };
    let mut inner = json!({ "$type": "app.gather.block.defs#header", "text": "deep" });
    for _ in 0..2000 {
        inner = wrap(inner);
    }
    let mut card =
        json!({ "$type": "app.gather.block.card", "createdAt": "2027-04-30T07:00:00.000Z" });
    card["blocks"] = Value::Array(vec![inner]);
    let err = validate_card(&card).expect_err("too deep");
    assert_eq!(err.reason, "max-depth");
    assert_eq!(err.path, format!("/blocks/0{}", "/blocks/0".repeat(10)));
    // Take it apart level by level: dropping it whole would recurse 2000 deep.
    let mut value = card;
    while let Some(next) =
        value.get_mut("blocks").and_then(|b| b.as_array_mut()).and_then(|b| b.pop())
    {
        value = next;
    }
    drop::<Value>(value);
}
