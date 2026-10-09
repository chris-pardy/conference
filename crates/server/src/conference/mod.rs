//! Conferences (features/conference-space.md, design rounds 1 and 2): one
//! private space per conference, on the organization's own PDS, with
//! eventside as the space's managing app.
//!
//! - **The space** is created with the organization's session
//!   (`simplespace.createSpace`), with a `managingAppPolicy` naming
//!   eventside's `#eventside_access` service, so the PDS asks eventside
//!   (`checkUserAccess`, in [`api`]) who may read and write.
//! - **Who's in** is decided by [`decide`], from the decisions log in
//!   eventside's database. The signed member, ban and apps records in the
//!   organization's repo in the space are a copy of it, written by the
//!   [`outbox`] and signed with eventside's `#eventside_attest` keys
//!   ([`attest`]).
//! - **Joining** ([`join`]) and leaving go through the app's XRPC
//!   ([`api`]); everything an organizer does goes through the admin CLI
//!   ([`cli`]).
//!
//! **Test hook (TC-40).** When the environment has
//! `EVENTSIDE_HALT_AFTER_DECISION=1`, the process that commits a decision
//! (the server or the CLI) exits with code 86 right after the decision's
//! database transaction commits, before any outbox entry is applied. The
//! outbox worker in the server applies it when the server starts again.

pub mod api;
pub mod attest;
pub mod cli;
pub mod decide;
pub mod join;
pub mod outbox;
pub mod repo;
pub mod sync;
#[cfg(test)]
pub mod test_support;

use serde_json::Value;

use crate::AppState;
use crate::db::Db;

/// The exit code a process uses when `EVENTSIDE_HALT_AFTER_DECISION` stops it.
pub const HALT_AFTER_DECISION_EXIT: u8 = 86;

/// Every conference space's type.
pub const SPACE_TYPE: &str = "app.eventside.private";
/// The public event a conference publishes.
pub const EVENT: &str = "community.lexicon.calendar.event";
/// Eventside's settings next to the public event.
pub const SIDECAR: &str = "app.eventside.conference.sidecar";
/// A member and their role, signed (organization's repo, in the space).
pub const MEMBER: &str = "app.eventside.conference.member";
/// A ban, signed (organization's repo, in the space).
pub const BAN: &str = "app.eventside.conference.ban";
/// The apps a conference allows, signed (organization's repo, in the space).
pub const APPS: &str = "app.eventside.conference.apps";
/// A feed; the main feed is made when a conference is created.
pub const FEED: &str = "app.eventside.feed.feed";

/// The join methods a conference can turn on (the Nov 1 slice).
pub const METHODS: &[&str] = &["code", "list", "open"];

/// Eventside's own DID: the `did:web` of its public origin.
pub fn eventside_did(public_url: &str) -> String {
    let host = public_url.split_once("://").map_or(public_url, |(_, rest)| rest);
    format!("did:web:{}", host.trim_end_matches('/').replace(':', "%3A"))
}

/// The service a conference space's policy names as its managing app.
pub fn access_service(public_url: &str) -> String {
    format!("{}#eventside_access", eventside_did(public_url))
}

/// A space URI, `at://{authority}/space/{type}/{skey}`, taken apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceUri {
    pub authority: String,
    pub kind: String,
    pub skey: String,
}

impl SpaceUri {
    pub fn parse(uri: &str) -> Option<Self> {
        let rest = uri.strip_prefix("at://")?;
        let mut parts = rest.split('/');
        let (Some(authority), Some("space"), Some(kind), Some(skey), None) =
            (parts.next(), parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return None;
        };
        if !authority.starts_with("did:") || kind.is_empty() || skey.is_empty() {
            return None;
        }
        Some(Self { authority: authority.to_owned(), kind: kind.to_owned(), skey: skey.to_owned() })
    }

    pub fn conference(org: &str, skey: &str) -> String {
        format!("at://{org}/space/{SPACE_TYPE}/{skey}")
    }
}

/// A conference, as eventside keeps it.
#[derive(Debug, Clone)]
pub struct Conference {
    pub space: String,
    pub org: String,
    pub rkey: String,
    pub event: String,
    pub name: String,
    pub starts_at: String,
    pub ends_at: String,
    pub city: String,
    pub description: Option<String>,
    pub theme: Value,
    pub methods: Vec<String>,
}

type ConferenceRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    String,
    String,
);

const CONFERENCE_COLUMNS: &str =
    "space, org, rkey, event, name, starts_at, ends_at, city, description, theme, methods";

impl From<ConferenceRow> for Conference {
    fn from(row: ConferenceRow) -> Self {
        let (space, org, rkey, event, name, starts_at, ends_at, city, description, theme, methods) =
            row;
        Self {
            space,
            org,
            rkey,
            event,
            name,
            starts_at,
            ends_at,
            city,
            description,
            theme: serde_json::from_str(&theme).unwrap_or(Value::Null),
            methods: split_methods(&methods),
        }
    }
}

/// A comma-separated list of join methods, as kept.
pub fn split_methods(methods: &str) -> Vec<String> {
    methods.split(',').map(str::trim).filter(|m| !m.is_empty()).map(str::to_owned).collect()
}

impl Conference {
    pub fn has_method(&self, method: &str) -> bool {
        self.methods.iter().any(|m| m == method)
    }

    /// The conference's page in the app.
    pub fn page_url(&self, public_url: &str) -> String {
        format!("{public_url}/c/{}/{}", self.org, self.rkey)
    }
}

/// A conference by its space URI.
pub async fn load(db: &Db, space: &str) -> Result<Option<Conference>, String> {
    sqlx::query_as::<_, ConferenceRow>(&format!(
        "SELECT {CONFERENCE_COLUMNS} FROM conferences WHERE space = $1 AND status = 'ready'"
    ))
    .bind(space)
    .fetch_optional(db)
    .await
    .map(|row| row.map(Conference::from))
    .map_err(|e| e.to_string())
}

/// A conference by its public event: the organization's DID and the event's rkey.
pub async fn by_event(db: &Db, org: &str, rkey: &str) -> Result<Option<Conference>, String> {
    sqlx::query_as::<_, ConferenceRow>(&format!(
        "SELECT {CONFERENCE_COLUMNS} FROM conferences WHERE org = $1 AND rkey = $2 AND status = 'ready'"
    ))
    .bind(org)
    .bind(rkey)
    .fetch_optional(db)
    .await
    .map(|row| row.map(Conference::from))
    .map_err(|e| e.to_string())
}

/// A handle (or DID) to its DID.
pub async fn did_of(state: &AppState, who: &str) -> Result<String, String> {
    if who.starts_with("did:") {
        return if crate::identity::is_valid_did(who) {
            Ok(who.to_owned())
        } else {
            Err(format!("{who} isn't a DID eventside can resolve"))
        };
    }
    state.resolver.resolve_handle(who).await.map_err(|e| match e {
        crate::identity::IdentityError::HandleNotFound => {
            format!("{who} doesn't resolve to an account")
        }
        crate::identity::IdentityError::Unresolvable(why) => {
            format!("couldn't resolve {who}: {why}")
        }
    })
}

/// Milliseconds since the epoch as an RFC 3339 UTC timestamp, to the millisecond.
pub fn iso(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let millis = ms.rem_euclid(1000);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{millis:03}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eventside_is_the_did_web_of_its_origin() {
        assert_eq!(eventside_did("https://eventside.app"), "did:web:eventside.app");
        assert_eq!(eventside_did("http://127.0.0.1:5173"), "did:web:127.0.0.1%3A5173");
        assert_eq!(
            access_service("http://127.0.0.1:5173"),
            "did:web:127.0.0.1%3A5173#eventside_access"
        );
    }

    #[test]
    fn space_uris_parse() {
        let uri = SpaceUri::conference("did:plc:abc", "3kx");
        assert_eq!(uri, "at://did:plc:abc/space/app.eventside.private/3kx");
        let parsed = SpaceUri::parse(&uri).unwrap();
        assert_eq!((parsed.authority.as_str(), parsed.skey.as_str()), ("did:plc:abc", "3kx"));
        assert!(SpaceUri::parse("at://did:plc:abc/app.bsky.feed.post/3kx").is_none());
        assert!(SpaceUri::parse("at://did:plc:abc/space/app.eventside.private/3kx/x").is_none());
    }

    #[test]
    fn timestamps_are_rfc3339() {
        assert_eq!(iso(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(iso(1_809_327_600_123), "2027-05-03T07:00:00.123Z");
    }
}
