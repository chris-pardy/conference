//! The server's configuration, read from the environment.

use std::time::Duration;

/// The scopes asked for at sign-in. A feature that needs more adds them here;
/// sessions granted fewer are asked to sign in again.
pub const LOGIN_SCOPES: &[&str] = &["atproto"];

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    /// The vivarium (or network) the health check probes, and the default
    /// for every identity service.
    pub atproto_url: String,
    /// The origin the browser uses for the app and the appview.
    pub public_url: Option<String>,
    pub database_url: String,
    /// An ES256 private JWK. Without it, a key is generated and kept in the database.
    pub signing_key: Option<String>,
    pub scopes: Vec<String>,
    pub signup_pds_url: String,
    pub plc_url: String,
    pub handle_resolver_url: String,
    pub allow_private_network: bool,
    pub session_idle_timeout: Duration,
    pub token_renew_interval: Duration,
    pub token_refresh_skew: Duration,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.trim().is_empty());
        let port = match var("PORT") {
            Some(p) => p.parse().map_err(|_| format!("PORT must be a port number, not {p:?}"))?,
            None => 3100,
        };
        // Unset or empty means a vivarium on its default port.
        let atproto_url =
            trim_url(&var("ATPROTO_URL").unwrap_or_else(|| "http://localhost:2580".into()));
        let duration = |name: &str, default: Duration| match var(name) {
            Some(v) => parse_duration(&v).map_err(|e| format!("{name}: {e}")),
            None => Ok(default),
        };
        let scopes = scopes(var("OAUTH_SCOPES").as_deref())?;
        Ok(Self {
            port,
            public_url: var("PUBLIC_URL").map(|u| trim_url(&u)),
            database_url: var("DATABASE_URL")
                .unwrap_or_else(|| "sqlite://data/eventside.db?mode=rwc".into()),
            signing_key: var("OAUTH_SIGNING_KEY"),
            scopes,
            signup_pds_url: trim_url(&var("SIGNUP_PDS_URL").unwrap_or_else(|| atproto_url.clone())),
            plc_url: trim_url(&var("PLC_URL").unwrap_or_else(|| atproto_url.clone())),
            handle_resolver_url: trim_url(
                &var("HANDLE_RESOLVER_URL").unwrap_or_else(|| atproto_url.clone()),
            ),
            allow_private_network: var("ALLOW_PRIVATE_NETWORK")
                .is_some_and(|v| v == "true" || v == "1"),
            session_idle_timeout: nonzero(
                "SESSION_IDLE_TIMEOUT",
                duration("SESSION_IDLE_TIMEOUT", Duration::from_secs(30 * 24 * 60 * 60))?,
            )?,
            token_renew_interval: nonzero(
                "TOKEN_RENEW_INTERVAL",
                duration("TOKEN_RENEW_INTERVAL", Duration::from_secs(5 * 60))?,
            )?,
            token_refresh_skew: duration("TOKEN_REFRESH_SKEW", Duration::from_secs(60))?,
            atproto_url,
        })
    }
}

/// The scopes to ask for: `OAUTH_SCOPES`, or `LOGIN_SCOPES` when it's unset.
/// The list is this instance's client ID, so it must be one the metadata
/// route serves: valid, distinct scope names, `atproto` among them.
fn scopes(value: Option<&str>) -> Result<Vec<String>, String> {
    let scopes: Vec<String> = match value {
        Some(s) => s.split_whitespace().map(str::to_owned).collect(),
        None => LOGIN_SCOPES.iter().map(|s| (*s).to_owned()).collect(),
    };
    if crate::oauth::well_formed_scope(&scopes.join(" ")) {
        Ok(scopes)
    } else {
        Err("OAUTH_SCOPES must be distinct scope names, `atproto` among them".to_owned())
    }
}

fn nonzero(name: &str, value: Duration) -> Result<Duration, String> {
    if value.is_zero() { Err(format!("{name} must be longer than zero")) } else { Ok(value) }
}

fn trim_url(url: &str) -> String {
    url.trim().trim_end_matches('/').to_owned()
}

/// The longest duration a setting takes. It keeps timestamps, and cookie
/// lifetimes built from them, far from overflowing.
const MAX_DURATION: Duration = Duration::from_secs(3650 * 24 * 60 * 60);

/// Parses `90`, `90s`, `5m`, `2h` or `30d`, up to ten years. A bare number is seconds.
pub fn parse_duration(value: &str) -> Result<Duration, String> {
    let value = value.trim();
    let split = value.find(|c: char| !c.is_ascii_digit()).unwrap_or(value.len());
    let (number, unit) = value.split_at(split);
    let number: u64 = number
        .parse()
        .map_err(|_| format!("{value:?} is not a duration like 30s, 5m, 2h or 30d"))?;
    let seconds = match unit {
        "" | "s" => 1,
        "m" => 60,
        "h" => 60 * 60,
        "d" => 24 * 60 * 60,
        _ => return Err(format!("{value:?} has an unknown unit; use s, m, h or d")),
    };
    match number.checked_mul(seconds).map(Duration::from_secs) {
        Some(duration) if duration <= MAX_DURATION => Ok(duration),
        _ => Err(format!("{value:?} is too long; the most is 3650d")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_take_a_unit() {
        assert_eq!(parse_duration("2s"), Ok(Duration::from_secs(2)));
        assert_eq!(parse_duration("90"), Ok(Duration::from_secs(90)));
        assert_eq!(parse_duration("5m"), Ok(Duration::from_secs(300)));
        assert_eq!(parse_duration("2h"), Ok(Duration::from_secs(7200)));
        assert_eq!(parse_duration("30d"), Ok(Duration::from_secs(2_592_000)));
        assert!(parse_duration("soon").is_err());
        assert!(parse_duration("5w").is_err());
    }

    #[test]
    fn the_scope_list_must_include_atproto_once() {
        assert_eq!(scopes(None), Ok(vec!["atproto".to_owned()]));
        assert_eq!(
            scopes(Some(" atproto  transition:generic ")),
            Ok(vec!["atproto".to_owned(), "transition:generic".to_owned()])
        );
        assert!(scopes(Some("transition:generic transition:generic")).is_err());
        assert!(scopes(Some("transition:generic")).is_err());
        assert!(scopes(Some("atproto atproto")).is_err());
        assert!(scopes(Some("atproto \"quoted\"")).is_err());
    }

    #[test]
    fn durations_are_bounded() {
        assert_eq!(parse_duration("3650d"), Ok(MAX_DURATION));
        assert!(parse_duration("3651d").is_err());
        assert!(parse_duration("18446744073709551615d").is_err());
        assert!(nonzero("TOKEN_RENEW_INTERVAL", parse_duration("0").unwrap()).is_err());
        assert!(nonzero("TOKEN_RENEW_INTERVAL", parse_duration("1s").unwrap()).is_ok());
    }
}
