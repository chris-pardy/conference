//! The session and pre-auth cookies. On HTTPS they're `__Host-` prefixed and
//! `Secure`; on loopback HTTP (dev and tests) they're plain `HttpOnly`.

use std::time::Duration;

use axum::http::HeaderMap;
use axum::http::header::COOKIE;

const SESSION: &str = "session";
const PREAUTH: &str = "oauth_preauth";

fn name(base: &str, secure: bool) -> String {
    if secure { format!("__Host-{base}") } else { base.to_owned() }
}

fn read(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v.to_owned())
        .filter(|v| !v.is_empty())
}

fn set(name: &str, value: &str, max_age: Duration, secure: bool) -> String {
    let secure = if secure { "; Secure" } else { "" };
    format!("{name}={value}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}{secure}", max_age.as_secs())
}

pub fn session(headers: &HeaderMap, secure: bool) -> Option<String> {
    read(headers, &name(SESSION, secure))
}

/// The pre-auth cookie is named per sign-in, after the start of its `state`.
fn preauth_name(state: &str, secure: bool) -> String {
    let id: String = state
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .take(16)
        .collect();
    name(&format!("{PREAUTH}_{id}"), secure)
}

pub fn preauth(headers: &HeaderMap, state: &str, secure: bool) -> Option<String> {
    read(headers, &preauth_name(state, secure))
}

pub fn set_session(value: &str, max_age: Duration, secure: bool) -> String {
    set(&name(SESSION, secure), value, max_age, secure)
}

pub fn clear_session(secure: bool) -> String {
    set(&name(SESSION, secure), "", Duration::ZERO, secure)
}

pub fn set_preauth(state: &str, value: &str, secure: bool) -> String {
    set(&preauth_name(state, secure), value, Duration::from_secs(10 * 60), secure)
}

pub fn clear_preauth(state: &str, secure: bool) -> String {
    set(&preauth_name(state, secure), "", Duration::ZERO, secure)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn cookies_are_read_by_name() {
        let mut headers = HeaderMap::new();
        headers.insert(COOKIE, HeaderValue::from_static("a=1; session=abc; oauth_preauth_st8=xyz"));
        assert_eq!(session(&headers, false).as_deref(), Some("abc"));
        assert_eq!(preauth(&headers, "st8", false).as_deref(), Some("xyz"));
        assert_eq!(preauth(&headers, "other", false), None);
        assert_eq!(session(&headers, true), None);
    }

    #[test]
    fn secure_cookies_are_host_prefixed() {
        let cookie = set_session("v", Duration::from_secs(60), true);
        assert!(cookie.starts_with("__Host-session=v; Path=/; HttpOnly; SameSite=Lax; Max-Age=60"));
        assert!(cookie.ends_with("; Secure"));
        assert!(!set_session("v", Duration::from_secs(60), false).contains("Secure"));
    }
}
