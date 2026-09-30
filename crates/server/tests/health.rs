use conference_server::health::{AtprotoStatus, health_report};
use serde_json::json;

#[test]
fn tc_5_the_backends_health_response_has_a_fixed_shape() {
    let reachable = serde_json::to_value(health_report(AtprotoStatus::Reachable)).unwrap();
    assert_eq!(reachable, json!({ "status": "up", "atproto": "reachable" }));

    let unreachable = serde_json::to_value(health_report(AtprotoStatus::Unreachable)).unwrap();
    assert_eq!(unreachable, json!({ "status": "up", "atproto": "unreachable" }));
}
