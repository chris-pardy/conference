//! Our server as the space host for every eventside space: each
//! organization's space authority (a DID whose document names us as its
//! `#atproto_space_host`), the credentials apps trade delegation tokens for,
//! write notifications from members' PDSes, the writer set, revocation, and
//! the index of the records every permission is built from.

pub mod api;
pub mod authority;
pub mod credential;
pub mod index;
pub mod notify;
pub mod sync;

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::{get, post};
use serde_json::Value;

use crate::AppState;
use crate::auth::xrpc_error;
use crate::db::now_ms;

/// The space types eventside hosts.
pub const ADMIN_TYPE: &str = "app.eventside.admin";
pub const CONFERENCE_TYPE: &str = "app.eventside.conference";
pub const INTAKE_TYPE: &str = "app.eventside.intake";

/// `at://{authority}/space/{type}/{skey}`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
        let rkey_ok = |s: &str| {
            !s.is_empty()
                && s.len() <= 512
                && s != "."
                && s != ".."
                && s.bytes().all(|b| b.is_ascii_alphanumeric() || b"._:~-".contains(&b))
        };
        let nsid_ok = |s: &str| {
            s.split('.').count() >= 3
                && s.split('.').all(|seg| {
                    !seg.is_empty() && seg.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                })
        };
        (crate::identity::is_valid_did(authority) && nsid_ok(kind) && rkey_ok(skey)).then(|| Self {
            authority: authority.to_owned(),
            kind: kind.to_owned(),
            skey: skey.to_owned(),
        })
    }

    pub fn new(authority: &str, kind: &str, skey: &str) -> Self {
        Self { authority: authority.to_owned(), kind: kind.to_owned(), skey: skey.to_owned() }
    }

    /// An organization's admin space.
    pub fn admin(authority: &str) -> Self {
        Self::new(authority, ADMIN_TYPE, "self")
    }
}

impl std::fmt::Display for SpaceUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "at://{}/space/{}/{}", self.authority, self.kind, self.skey)
    }
}

/// `{authority}#atproto_space_host`: the audience of delegation tokens, client
/// attestations and write notifications.
pub fn host_audience(authority: &str) -> String {
    format!("{authority}#atproto_space_host")
}

/// The host's in-memory state: DID documents, single-use token caches and
/// rate limits. All of it is disposable.
#[derive(Default)]
pub struct Host {
    docs: Mutex<HashMap<String, (Value, i64)>>,
    used: Mutex<HashMap<String, i64>>,
    limits: Mutex<HashMap<String, Vec<i64>>>,
    /// Organizations' derived permissions, by the index generation they
    /// were derived at.
    orgs: Mutex<HashMap<String, (i64, Arc<index::Org>)>>,
    /// Held while a space revision is assigned, so no two are the same.
    pub sequence: tokio::sync::Mutex<()>,
}

/// The address a request came from, when the server knows it: the peer, or
/// for a request through one of our `TRUSTED_PROXIES`, the address the
/// proxies saw it from.
pub struct ClientIp(pub Option<IpAddr>);

impl axum::extract::FromRequestParts<AppState> for ClientIp {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let peer = parts
            .extensions
            .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
            .map(|info| info.0.ip());
        Ok(Self(peer.map(|peer| client_ip(peer, &parts.headers, &state.config.trusted_proxies))))
    }
}

/// The client's address: the peer, unless it's a trusted proxy, in which
/// case the last `X-Forwarded-For` address that isn't one. Anything before
/// that is the client's own say, and not trusted.
pub fn client_ip(peer: IpAddr, headers: &axum::http::HeaderMap, trusted: &[IpAddr]) -> IpAddr {
    if !trusted.contains(&peer) {
        return peer;
    }
    let forwarded: Vec<IpAddr> = headers
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .filter_map(|a| a.trim().parse().ok())
        .collect();
    forwarded.into_iter().rev().find(|a| !trusted.contains(a)).unwrap_or(peer)
}

/// How long a DID document is reused.
const DOC_TTL_MS: i64 = 60_000;
/// The most entries any one cache holds before it's swept.
const CACHE_MAX: usize = 10_000;

impl Host {
    /// A DID's document, cached for a minute.
    pub async fn did_document(&self, state: &AppState, did: &str) -> Result<Value, String> {
        let now = now_ms();
        if let Some((doc, at)) = self.docs.lock().expect("the DID cache isn't poisoned").get(did)
            && now - at < DOC_TTL_MS
        {
            return Ok(doc.clone());
        }
        let doc = state.resolver.did_document(did).await.map_err(|e| format!("{e:?}"))?;
        let mut docs = self.docs.lock().expect("the DID cache isn't poisoned");
        if docs.len() >= CACHE_MAX {
            docs.clear();
        }
        docs.insert(did.to_owned(), (doc.clone(), now));
        Ok(doc)
    }

    /// The key a DID signs with under `kid` (`#atproto` when absent), from
    /// its document's verification methods.
    pub async fn signing_key(
        &self,
        state: &AppState,
        did: &str,
        kid: Option<&str>,
    ) -> Result<crate::crypto::PublicKey, String> {
        let doc = self.did_document(state, did).await?;
        let wanted = match kid {
            Some(kid) if kid.starts_with('#') => kid.to_owned(),
            Some(kid) => format!("#{}", kid.rsplit('#').next().unwrap_or(kid)),
            None => "#atproto".to_owned(),
        };
        let methods =
            doc.get("verificationMethod").and_then(Value::as_array).cloned().unwrap_or_default();
        let find = |fragment: &str| {
            methods.iter().find_map(|m| {
                let id = m.get("id")?.as_str()?;
                (id == fragment || id.ends_with(fragment))
                    .then(|| m.get("publicKeyMultibase")?.as_str().map(str::to_owned))
                    .flatten()
            })
        };
        let key = find(&wanted)
            .or_else(|| find("#atproto"))
            .ok_or_else(|| format!("{did} publishes no {wanted} key"))?;
        crate::crypto::PublicKey::from_did_key(&key)
    }

    /// Where a DID's repo lives, from its document.
    pub async fn pds_of(&self, state: &AppState, did: &str) -> Result<String, String> {
        let doc = self.did_document(state, did).await?;
        service_endpoint(&doc, "atproto_pds").ok_or_else(|| format!("{did} names no PDS"))
    }

    /// An organization's derived permissions, if cached at `generation`.
    pub fn cached_org(&self, org: &str, generation: i64) -> Option<Arc<index::Org>> {
        let orgs = self.orgs.lock().expect("the organization cache isn't poisoned");
        orgs.get(org).filter(|(at, _)| *at == generation).map(|(_, org)| org.clone())
    }

    pub fn cache_org(&self, org: &str, generation: i64, derived: Arc<index::Org>) {
        let mut orgs = self.orgs.lock().expect("the organization cache isn't poisoned");
        if orgs.len() >= CACHE_MAX {
            orgs.clear();
        }
        // Never replace a newer view with an older one.
        if orgs.get(org).is_none_or(|(at, _)| *at <= generation) {
            orgs.insert(org.to_owned(), (generation, derived));
        }
    }

    /// Marks a single-use token's `jti` used; false if it already was.
    pub fn consume(&self, kind: &str, jti: &str, exp_secs: i64) -> bool {
        let now = now_ms();
        let mut used = self.used.lock().expect("the replay cache isn't poisoned");
        if used.len() >= CACHE_MAX {
            used.retain(|_, until| *until > now);
        }
        let key = format!("{kind}:{jti}");
        if used.get(&key).is_some_and(|until| *until > now) {
            return false;
        }
        used.insert(key, exp_secs.saturating_mul(1000).saturating_add(60_000));
        true
    }

    /// Counts a request from `ip` against a limit of `max` a minute. Loopback
    /// is exempt only in local development (an `http` `PUBLIC_URL`), where
    /// the tests and the dev server share it; behind a reverse proxy, set
    /// `TRUSTED_PROXIES` so each client is counted by its own address.
    pub fn allow_ip(&self, state: &AppState, what: &str, ip: IpAddr, max: usize) -> bool {
        (ip.is_loopback() && !state.secure_cookies())
            || self.allow(&format!("{what}:ip:{ip}"), max, 60_000)
    }

    /// Counts an attempt against a limit of `max` per `window_ms`; false once
    /// it's over.
    pub fn allow(&self, key: &str, max: usize, window_ms: i64) -> bool {
        let now = now_ms();
        let mut limits = self.limits.lock().expect("the rate limits aren't poisoned");
        if limits.len() >= CACHE_MAX {
            limits.retain(|_, hits| hits.last().is_some_and(|t| now - t < window_ms));
        }
        let hits = limits.entry(key.to_owned()).or_default();
        hits.retain(|t| now - t < window_ms);
        if hits.len() >= max {
            return false;
        }
        hits.push(now);
        true
    }
}

/// The endpoint of a service in a DID document, by its fragment.
pub fn service_endpoint(doc: &Value, fragment: &str) -> Option<String> {
    let did = doc.get("id").and_then(Value::as_str).unwrap_or_default();
    doc.get("service")?.as_array()?.iter().find_map(|s| {
        let id = s.get("id")?.as_str()?;
        (id == format!("#{fragment}") || id == format!("{did}#{fragment}"))
            .then(|| s.get("serviceEndpoint")?.as_str().map(|e| e.trim_end_matches('/').to_owned()))
            .flatten()
    })
}

/// An XRPC refusal in the protocol's shape.
pub fn refuse(status: StatusCode, error: &str, message: &str) -> Response {
    xrpc_error(status, error, message)
}

pub fn invalid(error: &str, message: &str) -> Response {
    refuse(StatusCode::BAD_REQUEST, error, message)
}

pub fn internal(why: impl std::fmt::Display) -> Response {
    eprintln!("space host: {why}");
    refuse(StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError", "something went wrong")
}

/// The space host's protocol methods, and our host API.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/xrpc/com.atproto.space.getSpaceCredential", post(credential::get_space_credential))
        .route("/xrpc/com.atproto.space.listRepos", get(notify::list_repos))
        .route("/xrpc/com.atproto.space.registerNotify", post(notify::register_notify))
        .route("/xrpc/com.atproto.space.unregisterNotify", post(notify::unregister_notify))
        .route("/xrpc/com.atproto.space.notifyWrite", post(notify::notify_write))
        .route("/xrpc/app.eventside.space.getSpace", get(api::get_space))
        .route("/xrpc/app.eventside.space.listMembers", get(api::list_members))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_client_is_counted_by_its_own_address_behind_a_trusted_proxy() {
        let proxy: IpAddr = "10.0.0.2".parse().unwrap();
        let client: IpAddr = "203.0.113.7".parse().unwrap();
        let mut headers = axum::http::HeaderMap::new();
        // The client claims to be someone else; the proxy appends who it saw.
        headers.insert("x-forwarded-for", "198.51.100.1, 203.0.113.7".parse().unwrap());
        assert_eq!(client_ip(proxy, &headers, &[proxy]), client);
        // Not through a trusted proxy: the header is the client's own say.
        assert_eq!(client_ip(client, &headers, &[proxy]), client);
        assert_eq!(client_ip(proxy, &headers, &[]), proxy);
        // Through a proxy that names no client: the proxy.
        assert_eq!(client_ip(proxy, &axum::http::HeaderMap::new(), &[proxy]), proxy);
    }

    #[test]
    fn space_uris_parse_strictly() {
        let uri = "at://did:plc:abcdefghijklmnopqrstuvwx/space/app.eventside.conference/3abc";
        let parsed = SpaceUri::parse(uri).unwrap();
        assert_eq!(parsed.kind, CONFERENCE_TYPE);
        assert_eq!(parsed.to_string(), uri);
        for bad in [
            "at://did:plc:abcdefghijklmnopqrstuvwx/space/app.eventside.conference",
            "at://did:plc:abcdefghijklmnopqrstuvwx/space/app.eventside.conference/x/y",
            "at://ana.test/space/app.eventside.conference/x",
            "at://did:plc:abcdefghijklmnopqrstuvwx/app.eventside.conference/x",
            "at://did:plc:abcdefghijklmnopqrstuvwx/space/app.eventside.conference/..",
        ] {
            assert!(SpaceUri::parse(bad).is_none(), "{bad}");
        }
    }
}
