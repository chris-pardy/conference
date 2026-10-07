//! The appview as a confidential atproto OAuth client: its metadata and keys,
//! authorization-server discovery, pushed authorization requests, and the
//! token endpoint (code exchange, refresh) and revocation, all with
//! `private_key_jwt` client assertions and DPoP.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::db::{Db, now_ms};
use crate::keys::{EcKey, client_assertion, dpop_proof, random_token};
use crate::net::Http;

#[derive(Debug, Clone, Deserialize)]
pub struct AuthServer {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub pushed_authorization_request_endpoint: String,
    pub revocation_endpoint: Option<String>,
    /// The capabilities atproto requires of an authorization server, which
    /// this client relies on. Missing ones read as unsupported.
    #[serde(default)]
    pub authorization_response_iss_parameter_supported: bool,
    #[serde(default)]
    pub require_pushed_authorization_requests: bool,
    #[serde(default)]
    pub client_id_metadata_document_supported: bool,
    #[serde(default)]
    pub token_endpoint_auth_methods_supported: Vec<String>,
    #[serde(default)]
    pub token_endpoint_auth_signing_alg_values_supported: Vec<String>,
    #[serde(default)]
    pub dpop_signing_alg_values_supported: Vec<String>,
    #[serde(default)]
    pub scopes_supported: Vec<String>,
}

impl AuthServer {
    /// The first capability atproto requires (and this client uses) that the
    /// server doesn't advertise, if any.
    pub fn missing_capability(&self) -> Option<&'static str> {
        let has = |list: &[String], value: &str| list.iter().any(|v| v == value);
        if !self.authorization_response_iss_parameter_supported {
            Some("authorization_response_iss_parameter_supported")
        } else if !self.require_pushed_authorization_requests {
            Some("require_pushed_authorization_requests")
        } else if !self.client_id_metadata_document_supported {
            Some("client_id_metadata_document_supported")
        } else if !has(&self.token_endpoint_auth_methods_supported, "private_key_jwt") {
            Some("private_key_jwt in token_endpoint_auth_methods_supported")
        } else if !has(&self.token_endpoint_auth_signing_alg_values_supported, "ES256") {
            Some("ES256 in token_endpoint_auth_signing_alg_values_supported")
        } else if !has(&self.dpop_signing_alg_values_supported, "ES256") {
            Some("ES256 in dpop_signing_alg_values_supported")
        } else if !has(&self.scopes_supported, "atproto") {
            Some("atproto in scopes_supported")
        } else {
            None
        }
    }
}

/// Why an authorization server can't be used.
#[derive(Debug)]
pub enum ServerError {
    /// It couldn't be reached, or its metadata didn't check out.
    Unavailable(String),
    /// It answered, but lacks a capability atproto OAuth requires.
    Unsupported(String),
}

impl std::fmt::Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(why) | Self::Unsupported(why) => f.write_str(why),
        }
    }
}

impl From<String> for ServerError {
    fn from(why: String) -> Self {
        Self::Unavailable(why)
    }
}

impl ServerError {
    /// The error code the PWA's sign-in page shows a message for.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unavailable(_) => "server_unavailable",
            Self::Unsupported(_) => "server_unsupported",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TokenSet {
    pub access_token: String,
    pub token_type: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<i64>,
    pub scope: Option<String>,
    pub sub: Option<String>,
}

impl TokenSet {
    /// When the access token expires, in epoch milliseconds. `expires_in`
    /// comes from whatever server the account's DID document names, so it's
    /// kept between 30 seconds and a day (5 minutes when it's missing).
    pub fn expires_at(&self, now: i64) -> i64 {
        now.saturating_add(self.expires_in.unwrap_or(300).clamp(30, 24 * 60 * 60) * 1000)
    }
}

/// How a call to an authorization server failed.
#[derive(Debug)]
pub enum OAuthError {
    /// The server refused the grant: the session behind it is over.
    InvalidGrant(String),
    /// Any other refusal (a 4xx with some other error).
    Rejected(String),
    /// Network errors and 5xx: worth retrying, never a reason to end a session.
    Unavailable(String),
}

impl std::fmt::Display for OAuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidGrant(why) => write!(f, "invalid grant: {why}"),
            Self::Rejected(why) => write!(f, "rejected: {why}"),
            Self::Unavailable(why) => write!(f, "unavailable: {why}"),
        }
    }
}

#[derive(Clone)]
pub struct OAuthClient {
    pub client_id: String,
    pub redirect_uri: String,
    pub public_url: String,
    pub scope: String,
    pub key: EcKey,
    pub http: Http,
    /// The latest DPoP nonce each server handed out, by origin.
    nonces: Arc<Mutex<HashMap<String, String>>>,
    /// Authorization-server metadata by issuer, with when it was fetched.
    servers: Arc<Mutex<HashMap<String, (AuthServer, i64)>>>,
    /// Issuers that recently couldn't be reached, and until when to leave them be.
    down: Arc<Mutex<HashMap<String, i64>>>,
}

const MAX_NONCE_BYTES: usize = 512;
const MAX_NONCES: usize = 1024;

/// How long an unreachable authorization server is left alone.
const BACKOFF_MS: i64 = 30_000;

/// How long fetched authorization-server metadata is reused.
const METADATA_TTL_MS: i64 = 10 * 60 * 1000;

impl OAuthClient {
    /// A client asking for `scopes`, which must be a well-formed list: it is
    /// the client ID, so `metadata` has to serve it.
    pub fn new(
        public_url: &str,
        scopes: &[String],
        key: EcKey,
        http: Http,
    ) -> Result<Self, String> {
        let scope = scopes.join(" ");
        if let Some(problem) = scope_problem(&scope) {
            return Err(format!("the OAuth scope list {problem}"));
        }
        Ok(Self {
            client_id: client_id(public_url, &scope),
            redirect_uri: format!("{public_url}/oauth/callback"),
            public_url: public_url.to_owned(),
            scope,
            key,
            http,
            nonces: Arc::default(),
            servers: Arc::default(),
            down: Arc::default(),
        })
    }

    /// The metadata document served at a client ID, given the raw query
    /// string it was fetched with (none for the bare URL, which is `atproto`
    /// alone). Only the exact URL that is a client ID is served: `scope=`
    /// and the list encoded as `client_id` encodes it, nothing else, so the
    /// document's `client_id` is always the URL it was fetched from.
    ///
    /// Not only this instance's own scope list is served: a grant issued
    /// under another list (by an instance on another version, during a
    /// rolling deploy) is refreshed and revoked as its own client ID, and the
    /// authorization server may fetch that client's metadata again. So any
    /// well-formed list is served, at the URL that is its client ID. That
    /// grants nothing: the client is confidential, so every request as any
    /// of these client IDs needs an assertion signed with this client's key.
    pub fn metadata(&self, query: Option<&str>) -> Option<Value> {
        let scope = match query {
            None => "atproto".to_owned(),
            Some(query) => scope_of_query(query)?,
        };
        let scope = scope.as_str();
        Some(json!({
            "client_id": client_id(&self.public_url, scope),
            "client_name": "Eventside",
            "client_uri": self.public_url,
            "redirect_uris": [self.redirect_uri],
            "scope": scope,
            "grant_types": ["authorization_code", "refresh_token"],
            "response_types": ["code"],
            "token_endpoint_auth_method": "private_key_jwt",
            "token_endpoint_auth_signing_alg": "ES256",
            "dpop_bound_access_tokens": true,
            "application_type": "web",
            "jwks_uri": format!("{}/oauth/jwks.json", self.public_url),
        }))
    }

    pub fn jwks(&self) -> Value {
        let mut key = self.key.public_jwk();
        key["alg"] = json!("ES256");
        key["use"] = json!("sig");
        json!({ "keys": [key] })
    }

    /// The authorization server behind a PDS, with the spec's checks: the PDS
    /// names it, and its metadata's issuer is the URL it was fetched from.
    pub async fn discover(&self, pds: &str) -> Result<AuthServer, ServerError> {
        let resource_url = format!("{pds}/.well-known/oauth-protected-resource");
        let resource: Value = self.get_json(&resource_url).await?;
        let described =
            resource.get("resource").and_then(Value::as_str).map(|r| r.trim_end_matches('/'));
        if described != Some(pds.trim_end_matches('/')) {
            return Err(format!("{resource_url} describes {described:?}, not {pds}").into());
        }
        let issuer = resource
            .get("authorization_servers")
            .and_then(Value::as_array)
            .and_then(|servers| servers.first())
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{pds} names no authorization server"))?
            .trim_end_matches('/')
            .to_owned();
        self.auth_server(&issuer).await
    }

    /// An issuer's metadata, reused for a while: refreshes and revocations
    /// for sessions whose issuer was checked when they were created. A failed
    /// fetch marks the issuer down, which only the renewer heeds.
    pub async fn cached_auth_server(&self, issuer: &str) -> Result<AuthServer, ServerError> {
        let now = now_ms();
        let cached =
            self.servers.lock().expect("the metadata cache isn't poisoned").get(issuer).cloned();
        if let Some((server, at)) = cached
            && now - at < METADATA_TTL_MS
        {
            return Ok(server);
        }
        let server = self.auth_server(issuer).await.inspect_err(|_| self.mark_down(issuer))?;
        self.servers
            .lock()
            .expect("the metadata cache isn't poisoned")
            .insert(issuer.to_owned(), (server.clone(), now));
        Ok(server)
    }

    /// An authorization server's metadata, checked against its issuer.
    /// One lacking a capability atproto requires is refused as unsupported.
    pub async fn auth_server(&self, issuer: &str) -> Result<AuthServer, ServerError> {
        if !bare_origin(issuer) {
            return Err(format!("the issuer {issuer:?} isn't a bare origin").into());
        }
        let metadata_url = format!("{issuer}/.well-known/oauth-authorization-server");
        let server: AuthServer = serde_json::from_value(self.get_json(&metadata_url).await?)
            .map_err(|e| format!("{metadata_url}: {e}"))?;
        // An atproto issuer is a bare origin; the same string is stored and
        // compared everywhere, so a trailing slash is refused, not trimmed.
        if server.issuer != issuer {
            return Err(format!("{metadata_url} claims to be {}", server.issuer).into());
        }
        for endpoint in [
            &server.authorization_endpoint,
            &server.token_endpoint,
            &server.pushed_authorization_request_endpoint,
        ] {
            self.http.guarded(endpoint)?;
        }
        if let Some(missing) = server.missing_capability() {
            return Err(ServerError::Unsupported(format!(
                "{issuer} doesn't support atproto OAuth: its metadata lacks {missing}"
            )));
        }
        Ok(server)
    }

    /// Whether an issuer recently couldn't be reached.
    pub fn backing_off(&self, issuer: &str) -> bool {
        let down = self.down.lock().expect("the backoff table isn't poisoned");
        down.get(issuer).is_some_and(|until| *until > now_ms())
    }

    /// Leaves an unreachable issuer alone for a while.
    pub fn mark_down(&self, issuer: &str) {
        let mut down = self.down.lock().expect("the backoff table isn't poisoned");
        down.insert(issuer.to_owned(), now_ms() + BACKOFF_MS);
    }

    /// The latest DPoP nonce a server (by any URL on it) handed out.
    pub fn nonce(&self, url: &str) -> Option<String> {
        self.nonces.lock().expect("the nonce cache isn't poisoned").get(&origin(url)).cloned()
    }

    /// Remembers a server's nonce. Bounded, since anyone can make the appview
    /// talk to servers of their choosing: oversized nonces are ignored, and a
    /// full cache starts over (nonces are only an optimization).
    pub fn remember_nonce(&self, url: &str, nonce: &str) {
        if nonce.len() > MAX_NONCE_BYTES {
            return;
        }
        let mut nonces = self.nonces.lock().expect("the nonce cache isn't poisoned");
        if nonces.len() >= MAX_NONCES {
            nonces.clear();
        }
        nonces.insert(origin(url), nonce.to_owned());
    }

    async fn get_json(&self, url: &str) -> Result<Value, String> {
        let res =
            self.http.guarded(url)?.get(url).send().await.map_err(|e| format!("{url}: {e}"))?;
        if !res.status().is_success() {
            return Err(format!("{url} answered {}", res.status()));
        }
        crate::net::read_json(res).await
    }

    /// The client ID that asks for `scope`, a well-formed scope list.
    pub fn client_id_for(&self, scope: &str) -> String {
        client_id(&self.public_url, scope)
    }

    /// Pushes an authorization request, returning the URL to send the browser to.
    pub async fn par(
        &self,
        server: &AuthServer,
        request: &ParRequest<'_>,
    ) -> Result<String, OAuthError> {
        self.par_for(server, request, &self.scope).await
    }

    /// Pushes an authorization request for another scope list (an admin's,
    /// or the email step's), as the client ID that asks for it.
    pub async fn par_for(
        &self,
        server: &AuthServer,
        request: &ParRequest<'_>,
        scope: &str,
    ) -> Result<String, OAuthError> {
        let client_id = client_id(&self.public_url, scope);
        let assertion = client_assertion(&self.key, &client_id, &server.issuer);
        let mut form = vec![
            ("response_type", "code"),
            ("client_id", client_id.as_str()),
            ("redirect_uri", self.redirect_uri.as_str()),
            ("scope", scope),
            ("state", request.state),
            ("code_challenge", request.code_challenge),
            ("code_challenge_method", "S256"),
            ("client_assertion_type", ASSERTION_TYPE),
            ("client_assertion", assertion.as_str()),
        ];
        if let Some(hint) = request.login_hint {
            form.push(("login_hint", hint));
        }
        if let Some(prompt) = request.prompt {
            form.push(("prompt", prompt));
        }
        let body = self
            .post_form(&server.pushed_authorization_request_endpoint, &form, request.dpop_key)
            .await?;
        let request_uri = body
            .get("request_uri")
            .and_then(Value::as_str)
            .ok_or_else(|| OAuthError::Rejected("the PAR response has no request_uri".into()))?;
        let mut url = url::Url::parse(&server.authorization_endpoint)
            .map_err(|e| OAuthError::Rejected(format!("bad authorization endpoint: {e}")))?;
        url.query_pairs_mut()
            .append_pair("client_id", &client_id)
            .append_pair("request_uri", request_uri);
        Ok(url.into())
    }

    /// Exchanges an authorization code for tokens, as the client ID the
    /// request was pushed as.
    pub async fn exchange_code(
        &self,
        server: &AuthServer,
        client_id: &str,
        code: &str,
        verifier: &str,
        dpop_key: &EcKey,
    ) -> Result<TokenSet, OAuthError> {
        let assertion = client_assertion(&self.key, client_id, &server.issuer);
        let form = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("code_verifier", verifier),
            ("client_id", client_id),
            ("redirect_uri", self.redirect_uri.as_str()),
            ("client_assertion_type", ASSERTION_TYPE),
            ("client_assertion", assertion.as_str()),
        ];
        self.token_request(server, &form, dpop_key).await
    }

    /// Refreshes a grant, as the client ID it was issued to.
    pub async fn refresh(
        &self,
        server: &AuthServer,
        client_id: &str,
        refresh_token: &str,
        dpop_key: &EcKey,
    ) -> Result<TokenSet, OAuthError> {
        let assertion = client_assertion(&self.key, client_id, &server.issuer);
        let form = [
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", client_id),
            ("client_assertion_type", ASSERTION_TYPE),
            ("client_assertion", assertion.as_str()),
        ];
        self.token_request(server, &form, dpop_key).await
    }

    /// Revokes a token, as the client ID it was issued to. Servers without a
    /// revocation endpoint are skipped.
    pub async fn revoke(
        &self,
        server: &AuthServer,
        client_id: &str,
        token: &str,
        dpop_key: &EcKey,
    ) -> Result<(), OAuthError> {
        let Some(endpoint) = &server.revocation_endpoint else { return Ok(()) };
        let assertion = client_assertion(&self.key, client_id, &server.issuer);
        let form = [
            ("token", token),
            ("token_type_hint", "refresh_token"),
            ("client_id", client_id),
            ("client_assertion_type", ASSERTION_TYPE),
            ("client_assertion", assertion.as_str()),
        ];
        self.post_form(endpoint, &form, dpop_key).await.map(|_| ())
    }

    async fn token_request(
        &self,
        server: &AuthServer,
        form: &[(&str, &str)],
        dpop_key: &EcKey,
    ) -> Result<TokenSet, OAuthError> {
        let body = self.post_form(&server.token_endpoint, form, dpop_key).await?;
        let tokens: TokenSet = serde_json::from_value(body)
            .map_err(|e| OAuthError::Rejected(format!("bad token response: {e}")))?;
        // atproto tokens are DPoP-bound, and always say whose they are.
        if !tokens.token_type.eq_ignore_ascii_case("DPoP") {
            return Err(OAuthError::Rejected(format!(
                "token_type {:?}, not DPoP",
                tokens.token_type
            )));
        }
        if tokens.sub.is_none() {
            return Err(OAuthError::Rejected("the token response has no sub".into()));
        }
        Ok(tokens)
    }

    /// POSTs a form with a DPoP proof, retrying once with the server's nonce.
    async fn post_form(
        &self,
        url: &str,
        form: &[(&str, &str)],
        dpop_key: &EcKey,
    ) -> Result<Value, OAuthError> {
        let client = self.http.guarded(url).map_err(OAuthError::Rejected)?;
        for attempt in 0..2 {
            let nonce = self.nonce(url);
            let proof = dpop_proof(dpop_key, "POST", url, nonce.as_deref(), None);
            let res = client
                .post(url)
                .header("DPoP", proof)
                .form(form)
                .send()
                .await
                .map_err(|e| OAuthError::Unavailable(format!("{url}: {e}")))?;
            let status = res.status();
            if let Some(new) = res.headers().get("dpop-nonce").and_then(|v| v.to_str().ok()) {
                self.remember_nonce(url, new);
            }
            let body = crate::net::read_capped(res).await.map_err(OAuthError::Unavailable)?;
            let text = String::from_utf8_lossy(&body);
            let body: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
            if status.is_success() {
                return Ok(body);
            }
            let error = body.get("error").and_then(Value::as_str).unwrap_or_default();
            if error == "use_dpop_nonce" && attempt == 0 {
                continue;
            }
            let why = format!("{url} answered {status}: {text}");
            return Err(if status.is_server_error() {
                OAuthError::Unavailable(why)
            } else if error == "invalid_grant" {
                OAuthError::InvalidGrant(why)
            } else {
                OAuthError::Rejected(why)
            });
        }
        Err(OAuthError::Rejected(format!("{url} kept asking for a new DPoP nonce")))
    }
}

pub struct ParRequest<'a> {
    pub state: &'a str,
    pub code_challenge: &'a str,
    pub dpop_key: &'a EcKey,
    pub login_hint: Option<&'a str>,
    pub prompt: Option<&'a str>,
}

/// The client ID: the metadata document's URL. Authorization servers cache
/// client metadata, so a server that saw the old scope list would refuse the
/// new one. Beyond the base `atproto` scope, the scope list is part of the
/// ID, which changes exactly when the list does.
fn client_id(public_url: &str, scope: &str) -> String {
    let base = format!("{public_url}/oauth-client-metadata.json");
    if scope == "atproto" { base } else { format!("{base}?{}", scope_query(scope)) }
}

/// The query string a client ID carries for a scope list other than `atproto`.
fn scope_query(scope: &str) -> String {
    format!(
        "scope={}",
        percent_encoding::utf8_percent_encode(scope, percent_encoding::NON_ALPHANUMERIC)
    )
}

/// The scope list a client ID's query string names, if it is exactly the
/// query `client_id` builds for a well-formed list other than `atproto`.
fn scope_of_query(query: &str) -> Option<String> {
    let encoded = query.strip_prefix("scope=")?;
    let scope = percent_encoding::percent_decode_str(encoded).decode_utf8().ok()?.into_owned();
    (scope != "atproto" && well_formed_scope(&scope) && scope_query(&scope) == query)
        .then_some(scope)
}

/// The scopes a client ID of this app asks for: `atproto` for the bare
/// metadata URL, or the list its query names. A pending sign-in's grant is
/// checked against these, the scopes its request was pushed with, whichever
/// instance answers the callback. `None` for anything else.
pub fn client_scopes(client_id: &str) -> Option<Vec<String>> {
    let (url, query) = match client_id.split_once('?') {
        Some((url, query)) => (url, Some(query)),
        None => (client_id, None),
    };
    if !url.ends_with("/oauth-client-metadata.json") {
        return None;
    }
    let scope = match query {
        None => "atproto".to_owned(),
        Some(query) => scope_of_query(query)?,
    };
    Some(scope.split(' ').map(str::to_owned).collect())
}

/// Whether a scope list is well formed: RFC 6749 scope tokens separated by
/// single spaces, `atproto` among them, none twice, and not too long.
pub fn well_formed_scope(scope: &str) -> bool {
    scope_problem(scope).is_none()
}

/// The rule a scope list breaks, if any, worded to follow "the scope list".
pub fn scope_problem(scope: &str) -> Option<&'static str> {
    let tokens: Vec<&str> = scope.split(' ').collect();
    let token_ok = |t: &&str| {
        !t.is_empty() && t.bytes().all(|b| matches!(b, 0x21 | 0x23..=0x5B | 0x5D..=0x7E))
    };
    if scope.len() > 2048 {
        Some("is longer than 2048 bytes")
    } else if !tokens.iter().all(token_ok) {
        Some("must be scope names separated by single spaces")
    } else if !tokens.contains(&"atproto") {
        Some("must include `atproto`")
    } else if !tokens.iter().enumerate().all(|(i, t)| !tokens[..i].contains(t)) {
        Some("names a scope twice")
    } else {
        None
    }
}

/// Whether an issuer is a bare origin, as atproto requires: no path, query,
/// fragment, trailing slash or default port.
fn bare_origin(issuer: &str) -> bool {
    url::Url::parse(issuer).is_ok_and(|u| u.origin().ascii_serialization() == issuer)
}

const ASSERTION_TYPE: &str = "urn:ietf:params:oauth:client-assertion-type:jwt-bearer";

fn origin(url: &str) -> String {
    url::Url::parse(url).map_or_else(|_| url.to_owned(), |u| u.origin().ascii_serialization())
}

/// The client's signing key: the configured one, or one generated once and
/// kept in the database's single `client_keys` row. Instances racing to
/// generate it conflict on that row, and all use the one that was stored.
pub async fn signing_key(db: &Db, configured: Option<&str>) -> Result<EcKey, String> {
    if let Some(jwk) = configured {
        let key = EcKey::from_jwk(jwk).map_err(|e| format!("OAUTH_SIGNING_KEY: {e}"))?;
        let kid = key.kid.clone().unwrap_or_else(|| "eventside-1".into());
        return Ok(key.with_kid(kid));
    }
    let key = EcKey::generate();
    sqlx::query(
        "INSERT INTO client_keys (slot, kid, private_jwk, created_at) VALUES (1, $1, $2, $3) ON CONFLICT (slot) DO NOTHING",
    )
    .bind(random_token(12))
    .bind(key.private_jwk())
    .bind(now_ms())
    .execute(db)
    .await
    .map_err(|e| format!("could not store the signing key: {e}"))?;
    let (kid, jwk) = sqlx::query_as::<_, (String, String)>(
        "SELECT kid, private_jwk FROM client_keys WHERE slot = 1",
    )
    .fetch_one(db)
    .await
    .map_err(|e| format!("could not read the signing key: {e}"))?;
    Ok(EcKey::from_jwk(&jwk)?.with_kid(kid))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_client_id_changes_when_the_scopes_do() {
        assert_eq!(
            client_id("https://app.example", "atproto"),
            "https://app.example/oauth-client-metadata.json"
        );
        assert_eq!(
            client_id("https://app.example", "atproto transition:generic"),
            "https://app.example/oauth-client-metadata.json?scope=atproto%20transition%3Ageneric"
        );
    }

    #[test]
    fn metadata_is_served_for_any_well_formed_scope_list() {
        let client = OAuthClient::new(
            "https://app.example",
            &["atproto".into()],
            EcKey::generate(),
            Http::new(false),
        )
        .unwrap();
        let bare = client.metadata(None).unwrap();
        assert_eq!(bare["client_id"], client.client_id);
        assert_eq!(bare["scope"], "atproto");
        // Another instance's list, during a rolling deploy, at exactly its client ID.
        let grown_id = client_id("https://app.example", "atproto transition:generic");
        let query = grown_id.split_once('?').unwrap().1;
        assert_eq!(query, "scope=atproto%20transition%3Ageneric");
        let grown = client.metadata(Some(query)).unwrap();
        assert_eq!(grown["client_id"], grown_id);
        assert_eq!(grown["scope"], "atproto transition:generic");
        for refused in [
            "scope=atproto",
            "",
            "scope=",
            "scope=transition%3Ageneric",
            "scope=atproto%20%20transition%3Ageneric",
            "scope=%20atproto",
            "scope=atproto%20atproto",
            "scope=atproto%20%22quoted%22",
            // Other spellings of a served list name other client IDs.
            "scope=atproto+transition:generic",
            "scope=atproto%20transition:generic",
            "scope=atproto%20transition%3ageneric",
            "scope=atproto%20transition%3Ageneric&x=1",
            "x=1&scope=atproto%20transition%3Ageneric",
            "scope=atproto%20transition%3Ageneric&scope=atproto",
        ] {
            assert!(client.metadata(Some(refused)).is_none(), "{refused:?}");
        }
        let long = scope_query(&format!("atproto {}", "x".repeat(2048)));
        assert!(client.metadata(Some(&long)).is_none());
    }

    #[test]
    fn a_client_id_names_the_scopes_it_asks_for() {
        let bare = client_id("https://app.example", "atproto");
        assert_eq!(client_scopes(&bare), Some(vec!["atproto".to_owned()]));
        let grown = client_id("https://app.example", "atproto transition:generic");
        assert_eq!(
            client_scopes(&grown),
            Some(vec!["atproto".to_owned(), "transition:generic".to_owned()])
        );
        assert_eq!(client_scopes("https://app.example/other.json"), None);
        assert_eq!(client_scopes(&format!("{bare}?scope=atproto")), None);
    }

    #[test]
    fn a_client_is_only_built_for_a_scope_list_it_can_serve() {
        let build = |scopes: &[&str]| {
            let scopes: Vec<String> = scopes.iter().map(|s| (*s).to_owned()).collect();
            OAuthClient::new("https://app.example", &scopes, EcKey::generate(), Http::new(false))
        };
        assert!(build(&["atproto", "transition:generic"]).is_ok());
        let err = |scopes: &[&str]| build(scopes).err().unwrap();
        assert!(err(&["transition:generic"]).contains("atproto"));
        assert!(err(&["atproto", "atproto"]).contains("twice"));
        assert!(err(&["atproto", "\"quoted\""]).contains("scope names"));
        let long = "x".repeat(2048);
        assert!(err(&["atproto", &long]).contains("2048"));
    }

    #[test]
    fn an_issuer_must_be_a_bare_origin() {
        assert!(bare_origin("https://as.example"));
        assert!(bare_origin("http://127.0.0.1:2583"));
        assert!(!bare_origin("https://as.example/tenant"));
        assert!(!bare_origin("https://as.example/"));
        assert!(!bare_origin("https://as.example?x=1"));
        assert!(!bare_origin("https://as.example:443"));
        assert!(!bare_origin("not a url"));
    }

    #[test]
    fn token_lifetimes_are_bounded() {
        let tokens = |expires_in| TokenSet {
            access_token: "access".into(),
            token_type: "DPoP".into(),
            refresh_token: None,
            expires_in,
            scope: None,
            sub: None,
        };
        assert_eq!(tokens(Some(3600)).expires_at(1_000), 3_601_000);
        assert_eq!(tokens(None).expires_at(1_000), 301_000);
        assert_eq!(tokens(Some(0)).expires_at(1_000), 31_000);
        assert_eq!(tokens(Some(-5)).expires_at(1_000), 31_000);
        assert_eq!(tokens(Some(i64::MAX)).expires_at(1_000), 86_401_000);
        assert_eq!(tokens(Some(60)).expires_at(i64::MAX), i64::MAX);
    }
}
