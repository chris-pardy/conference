//! The server's configuration, read from the environment.

use std::time::Duration;

/// The scopes asked for at sign-in. A feature that needs more adds them here;
/// sessions granted fewer are asked to sign in again.
///
/// conference-space: joining and leaving a conference are the person's own
/// records in its intake space.
pub const LOGIN_SCOPES: &[&str] = &["atproto", INTAKE_SCOPE];

/// Writing your own join and leave records into any conference's intake space.
pub const INTAKE_SCOPE: &str = "space:app.eventside.intake?authority=*&action=create&action=delete&collection=app.eventside.intake.join&collection=app.eventside.intake.leave";

/// What the email step adds to the sign-in scopes, for one sign-in.
pub const EMAIL_SCOPE: &str = "transition:email";

/// The scopes an admin connects with (`admin connect`): the conference's
/// public records, its role and rules records, the admin space, and blobs for
/// branding. Admins whose grant lacks one are told to reconnect.
pub const ADMIN_SCOPES: &[&str] = &[
    "atproto",
    "repo:community.lexicon.calendar.event",
    "repo:app.eventside.conference",
    "space:app.eventside.conference?authority=*&action=create&action=update&action=delete&collection=app.eventside.conference&collection=app.eventside.conference.role&collection=app.eventside.conference.rules&collection=community.lexicon.calendar.event",
    "space:app.eventside.admin?authority=*&action=read&action=create&action=update&action=delete&collection=app.eventside.admin.admin&collection=app.eventside.admin.space&collection=app.eventside.admin.member&collection=app.eventside.admin.ban&collection=app.eventside.admin.deny&collection=app.eventside.admin.code&collection=app.eventside.admin.codeRevoke&collection=app.eventside.admin.listEntry",
    "blob:*/*",
];

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
    /// The scopes admins connect with: `ADMIN_OAUTH_SCOPES`, or `ADMIN_SCOPES`.
    pub admin_scopes: Vec<String>,
    /// The secret the authorities' keys are encrypted under, and emails and
    /// codes are HMAC'd with. Without it, one is generated and kept in the
    /// database (dev and tests).
    pub authority_key_secret: Option<String>,
    pub signup_pds_url: String,
    pub plc_url: String,
    pub handle_resolver_url: String,
    pub allow_private_network: bool,
    /// Reverse proxies in front of us (`TRUSTED_PROXIES`): a request from one
    /// of them is from the address it names last in `X-Forwarded-For`.
    pub trusted_proxies: Vec<std::net::IpAddr>,
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
        let admin_scopes =
            scopes("ADMIN_OAUTH_SCOPES", var("ADMIN_OAUTH_SCOPES").as_deref(), ADMIN_SCOPES)?;
        let scopes = scopes("OAUTH_SCOPES", var("OAUTH_SCOPES").as_deref(), LOGIN_SCOPES)?;
        let public_url = var("PUBLIC_URL").map(|u| public_url(&u)).transpose()?;
        let database_url =
            var("DATABASE_URL").unwrap_or_else(|| "sqlite://data/eventside.db?mode=rwc".into());
        crate::db::check_url(&database_url)?;
        let signing_key = var("OAUTH_SIGNING_KEY");
        if let Some(jwk) = &signing_key {
            crate::keys::EcKey::from_jwk(jwk).map_err(|e| format!("OAUTH_SIGNING_KEY: {e}"))?;
        }
        Ok(Self {
            port,
            public_url,
            database_url,
            signing_key,
            scopes,
            admin_scopes,
            authority_key_secret: var("AUTHORITY_KEY_SECRET"),
            signup_pds_url: trim_url(&var("SIGNUP_PDS_URL").unwrap_or_else(|| atproto_url.clone())),
            plc_url: trim_url(&var("PLC_URL").unwrap_or_else(|| atproto_url.clone())),
            handle_resolver_url: trim_url(
                &var("HANDLE_RESOLVER_URL").unwrap_or_else(|| atproto_url.clone()),
            ),
            allow_private_network: var("ALLOW_PRIVATE_NETWORK")
                .is_some_and(|v| v == "true" || v == "1"),
            trusted_proxies: trusted_proxies(var("TRUSTED_PROXIES").as_deref())?,
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

/// The scopes to ask for: the variable `name`, or `default` when it's unset.
/// The list is a client ID, so it must be one the metadata route serves:
/// valid, distinct scope names, `atproto` among them.
fn scopes(name: &str, value: Option<&str>, default: &[&str]) -> Result<Vec<String>, String> {
    let scopes: Vec<String> = match value {
        Some(s) => s.split_whitespace().map(str::to_owned).collect(),
        None => default.iter().map(|s| (*s).to_owned()).collect(),
    };
    match crate::oauth::scope_problem(&scopes.join(" ")) {
        None => Ok(scopes),
        Some(problem) => Err(format!("{name} {problem}")),
    }
}

/// `PUBLIC_URL`, which must be a bare `http` or `https` origin: the client
/// ID, redirect URI, cookies and every same-origin check are built on it. It's
/// returned as the browser serializes an origin (lowercase, punycode host, no
/// default port), so it matches the `Origin` header.
fn public_url(value: &str) -> Result<String, String> {
    url::Url::parse(value.trim())
        .ok()
        .filter(|u| {
            matches!(u.scheme(), "http" | "https")
                && u.username().is_empty()
                && u.password().is_none()
                && u.path() == "/"
                && u.query().is_none()
                && u.fragment().is_none()
        })
        .map(|u| u.origin().ascii_serialization())
        .ok_or_else(|| {
            format!(
                "PUBLIC_URL must be a bare origin like https://app.example (no path, query or credentials), not {value:?}"
            )
        })
}

/// `TRUSTED_PROXIES`: IP addresses, separated by commas or spaces.
fn trusted_proxies(value: Option<&str>) -> Result<Vec<std::net::IpAddr>, String> {
    value
        .unwrap_or_default()
        .split([',', ' '])
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            s.trim()
                .parse()
                .map_err(|_| format!("TRUSTED_PROXIES must list IP addresses, not {s:?}"))
        })
        .collect()
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
        let scopes = |value| scopes("OAUTH_SCOPES", value, LOGIN_SCOPES);
        assert_eq!(scopes(None), Ok(LOGIN_SCOPES.iter().map(|s| (*s).to_owned()).collect()));
        assert_eq!(
            scopes(Some(" atproto  transition:generic ")),
            Ok(vec!["atproto".to_owned(), "transition:generic".to_owned()])
        );
        assert!(scopes(Some("transition:generic transition:generic")).is_err());
        assert!(scopes(Some("transition:generic")).is_err());
        assert!(scopes(Some("atproto atproto")).is_err());
        assert!(scopes(Some("atproto \"quoted\"")).is_err());
        let long = format!("atproto {}", "x".repeat(2048));
        assert!(scopes(Some(&long)).unwrap_err().contains("2048"));
    }

    #[test]
    fn the_admin_and_email_scope_lists_are_well_formed() {
        assert!(crate::oauth::well_formed_scope(&ADMIN_SCOPES.join(" ")));
        let email = [LOGIN_SCOPES, &[EMAIL_SCOPE]].concat().join(" ");
        assert!(crate::oauth::well_formed_scope(&email));
    }

    #[test]
    fn the_public_url_is_a_bare_origin() {
        assert_eq!(public_url("https://app.example/"), Ok("https://app.example".to_owned()));
        assert_eq!(public_url("http://127.0.0.1:5173"), Ok("http://127.0.0.1:5173".to_owned()));
        // Normalized as the browser sends it in `Origin`.
        assert_eq!(public_url("HTTPS://App.Example"), Ok("https://app.example".to_owned()));
        assert_eq!(
            public_url("https://bücher.example"),
            Ok("https://xn--bcher-kva.example".to_owned())
        );
        assert_eq!(public_url("https://app.example:443/"), Ok("https://app.example".to_owned()));
        for refused in [
            "https://app.example/eventside",
            "app.example",
            "127.0.0.1:5173",
            "ftp://app.example",
            "https://app.example?x=1",
            "https://app.example#top",
            "https://me@app.example",
        ] {
            assert!(public_url(refused).is_err(), "{refused}");
        }
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
