//! The event card: a sub-event, posted in its parent's feed. It shows the
//! event from the platform's context (live going count, your RSVP) and
//! RSVPs from the card.
//!
//! - Client: an RSVP tap is kept as `pending` on the device, so the card
//!   answers at once, and is sent to the server.
//! - Server: an RSVP becomes the platform effect `@rsvp {status}`, which
//!   RSVPs the actor to the linked event. The card has no shared state.

use eventside_card::blocks::*;
use eventside_card::{
    Env, Value, action_type, card_module, emit, env, iso, json, meta, out, payload, state_or,
    unchanged,
};

card_module! { reduce: reducer, view: render }

const STATUSES: [&str; 3] = ["going", "maybe", "no"];

fn status_of(v: &Value) -> Option<&'static str> {
    let s = v.as_str().or_else(|| v["status"].as_str())?;
    STATUSES.iter().copied().find(|x| *x == s)
}

pub fn reducer(input: &Value) -> Value {
    match (env(input), action_type(input)) {
        (_, "@init") => out(json!({}), vec![]),
        (Env::Client, "rsvp") if meta(input)["origin"] != "server" => {
            let Some(status) = status_of(payload(input)) else { return unchanged() };
            let mut local = state_or(input, json!({}));
            local["pending"] = json!(status);
            out(local, vec![emit("rsvp", json!(status))])
        }
        (Env::Server, "rsvp") => match status_of(payload(input)) {
            Some(status) => json!({ "actions": [emit("@rsvp", json!({ "status": status }))] }),
            None => unchanged(),
        },
        _ => unchanged(),
    }
}

pub fn render(input: &Value) -> Value {
    let ev = &input["context"]["event"];
    let mut base = input["base"].clone();
    let Some(title) = ev["title"].as_str() else {
        let ui = card(&base, vec![header("Event"), small("This event isn't available any more.")]);
        return json!({ "ui": ui });
    };
    if let Some(tz) = ev["timeZone"].as_str().filter(|t| !t.is_empty())
        && base.is_object()
    {
        base["timeZone"] = json!(tz);
    }

    let confirmed = ev["rsvp"].as_str();
    let pending = input["local"]["pending"].as_str().filter(|p| Some(*p) != confirmed);

    let mut blocks = vec![header(title)];
    match (pending, confirmed) {
        (Some(_), _) => blocks.push(badge("Saving your RSVP…", "info")),
        (None, Some("going")) => blocks.push(badge("You're going", "success")),
        (None, Some("maybe")) => blocks.push(badge("You might go", "info")),
        (None, Some("no")) => blocks.push(badge("You can't make it", "neutral")),
        _ => {}
    }
    if let Some(d) = ev["description"].as_str().filter(|d| !d.trim().is_empty()) {
        blocks.push(text(d));
    }
    if let Some(start) = ev["startsAt"].as_i64().filter(|s| *s > 0) {
        let end = ev["endsAt"].as_i64().filter(|e| *e > start).map(iso);
        let now = input["now"].as_i64().unwrap_or(0);
        let countdown = now > 0 && now < start;
        let label = if countdown { "Starts" } else { "When" };
        blocks.push(time(label, &iso(start), end.as_deref(), countdown));
    }
    if let Some(place) = ev["location"].as_str().filter(|l| !l.trim().is_empty()) {
        blocks.push(room(place, ""));
    }
    let n = |k: &str| ev[k].as_u64().unwrap_or(0).to_string();
    blocks.push(columns(vec![vec![stat("going", &n("going"))], vec![stat("maybe", &n("maybe"))]]));
    if let Some(host) = ev["host"].as_str().filter(|h| h.starts_with("did:")) {
        blocks.push(small("Hosted by"));
        blocks.push(person(host));
    }
    blocks.push(choices(
        "rsvp",
        "Are you going?",
        "rsvp",
        &[("I'm going", "going"), ("Maybe", "maybe"), ("Can't go", "no")],
    ));
    blocks.push(small("Going opens the plan's own feed: tap Open plan below."));
    json!({ "ui": card(&base, blocks) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_rsvp_is_pending_on_the_client_and_an_effect_on_the_server() {
        let tap = json!({ "env": "client", "state": {}, "action": {
            "type": "rsvp", "payload": "going", "meta": { "origin": "ui" } } });
        let o = reducer(&tap);
        assert_eq!(o["state"]["pending"], "going");
        assert_eq!(o["actions"][0], json!({ "type": "rsvp", "payload": "going" }));
        let up = json!({ "env": "server", "state": {}, "action": { "type": "rsvp", "payload": "going" } });
        assert_eq!(
            reducer(&up)["actions"][0],
            json!({ "type": "@rsvp", "payload": { "status": "going" } })
        );
    }

    #[test]
    fn the_view_shows_saving_until_the_server_agrees() {
        let input = |rsvp: Value| {
            json!({
                "local": { "pending": "going" },
                "context": { "event": { "title": "Canal tour", "rsvp": rsvp, "going": 3 } },
            })
        };
        let saving = render(&input(Value::Null)).to_string();
        assert!(saving.contains("Saving your RSVP"));
        let done = render(&input(json!("going"))).to_string();
        assert!(done.contains("You're going") && !done.contains("Saving"));
    }
}
