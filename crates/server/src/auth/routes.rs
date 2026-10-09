//! The sign-in routes: `/oauth/login`, `/oauth/signup`, `/oauth/callback`,
//! `/oauth/logout`, the client's metadata and keys, and `getSession`; and
//! `/oauth/connect`, where the admin CLI's `connect` and `org connect` send
//! a browser.

use std::collections::HashMap;

use axum::extract::{Query, RawQuery, State};
use axum::http::header::{CACHE_CONTROL, LOCATION, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde_json::{Value, json};

use super::session::{self, Lookup, NewSession, TOMBSTONE};
use super::{CheckedSession, cookies, return_to::sanitize, xrpc_error};
use crate::AppState;
use crate::db::now_ms;
use crate::identity::{IdentityError, is_valid_did, normalize_handle};
use crate::keys::{EcKey, random_token, sha256_b64};
use crate::oauth::{self, AuthServer, OAuthError, ParRequest};

/// How long sign-out waits for revocation before answering.
const REVOKE_WAIT: std::time::Duration = std::time::Duration::from_secs(3);

/// How long a pending sign-in can wait for its callback.
const PENDING_MS: i64 = 10 * 60 * 1000;

type Params = Query<HashMap<String, String>>;

/// The client metadata at a client ID: the bare URL, or `?scope=…` for
/// another scope list, exactly as the client ID spells it (see
/// `OAuthClient::metadata`).
pub async fn client_metadata(State(state): State<AppState>, RawQuery(query): RawQuery) -> Response {
    match state.oauth.metadata(query.as_deref()) {
        Some(metadata) => Json(metadata).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

pub async fn jwks(State(state): State<AppState>) -> Json<Value> {
    Json(state.oauth.jwks())
}

/// A redirect that also sets cookies.
fn redirect(location: &str, cookies: &[String]) -> Response {
    let mut res = StatusCode::FOUND.into_response();
    let headers = res.headers_mut();
    if let Ok(value) = HeaderValue::from_str(location) {
        headers.insert(LOCATION, value);
    }
    for cookie in cookies {
        if let Ok(value) = HeaderValue::from_str(cookie) {
            headers.append(SET_COOKIE, value);
        }
    }
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

/// Back to the PWA's sign-in page with an error code.
fn fail(code: &str, return_to: &str, cookies: &[String]) -> Response {
    let encoded = utf8_percent_encode(return_to, NON_ALPHANUMERIC);
    redirect(&format!("/signin?error={code}&return_to={encoded}"), cookies)
}

pub async fn login(State(state): State<AppState>, Query(params): Params) -> Response {
    let return_to = sanitize(params.get("return_to").map(String::as_str));
    let handle = params.get("handle").map(String::as_str).unwrap_or_default().trim();
    // A DID works too: signing someone back in whose handle didn't verify.
    let resolved = if handle.starts_with("did:") {
        if is_valid_did(handle) {
            Ok(handle.to_owned())
        } else {
            Err(IdentityError::HandleNotFound)
        }
    } else {
        state.resolver.resolve_handle(handle).await
    };
    let did = match resolved {
        Ok(did) => did,
        Err(IdentityError::HandleNotFound) => return fail("handle_not_found", &return_to, &[]),
        Err(IdentityError::Unresolvable(why)) => {
            eprintln!("sign-in: {why}");
            return fail("resolution_failed", &return_to, &[]);
        }
    };
    let identity = match state.resolver.resolve_did(&did).await {
        Ok(identity) => identity,
        Err(err) => {
            eprintln!("sign-in: {err:?}");
            return fail("resolution_failed", &return_to, &[]);
        }
    };
    let server = match state.oauth.discover(&identity.pds).await {
        Ok(server) => server,
        Err(why) => {
            eprintln!("sign-in: {why}");
            return fail(why.code(), &return_to, &[]);
        }
    };
    // The handle as typed, normalized; it resolved, so it's a valid handle.
    let hint = normalize_handle(handle).unwrap_or_else(|| did.clone());
    let request = Start {
        expected_did: Some(&did),
        login_hint: Some(&hint),
        ..Start::new(Kind::Login, &return_to)
    };
    start(&state, &server, request).await
}

pub async fn signup(State(state): State<AppState>, Query(params): Params) -> Response {
    let return_to = sanitize(params.get("return_to").map(String::as_str));
    let server = match state.oauth.discover(&state.config.signup_pds_url).await {
        Ok(server) => server,
        Err(why) => {
            eprintln!("sign-up: {why}");
            return fail(why.code(), &return_to, &[]);
        }
    };
    let request = Start { prompt: Some("create"), ..Start::new(Kind::Signup, &return_to) };
    start(&state, &server, request).await
}

/// `/oauth/connect?id=…`: where `admin connect` and `admin org connect` send
/// a browser. Signs the account in with the scopes the CLI asked for, into a
/// session of its own that the browser never holds.
pub async fn connect(State(state): State<AppState>, Query(params): Params) -> Response {
    let id = params.get("id").map(String::as_str).unwrap_or_default();
    let row = sqlx::query_as::<_, (String, String, String, String, i64, Option<i64>)>(
        "SELECT kind, did, handle, scopes, expires_at, completed_at FROM admin_connects WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await;
    let (kind, did, handle, scopes) = match row {
        Ok(Some((kind, did, handle, scopes, expires_at, None))) if expires_at > now_ms() => {
            (kind, did, handle, scopes)
        }
        Ok(_) => {
            return page(
                StatusCode::NOT_FOUND,
                "This link has expired. Run the connect command again.",
            );
        }
        Err(err) => {
            eprintln!("connect: {err}");
            return page(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong. Try again.");
        }
    };
    let failed = |why: String| {
        let state = state.clone();
        async move {
            eprintln!("connect: {why}");
            let _ = sqlx::query("UPDATE admin_connects SET error = $1 WHERE id = $2")
                .bind(&why)
                .bind(id)
                .execute(&state.db)
                .await;
            page(StatusCode::BAD_GATEWAY, &format!("Couldn't connect: {why}"))
        }
    };
    let kind = if kind == session::ORG { Kind::Org } else { Kind::Admin };
    let identity = match state.resolver.resolve_did(&did).await {
        Ok(identity) => identity,
        Err(err) => return failed(format!("{err:?}")).await,
    };
    let server = match state.oauth.discover(&identity.pds).await {
        Ok(server) => server,
        Err(why) => return failed(why.to_string()).await,
    };
    let request = Start {
        expected_did: Some(&did),
        login_hint: Some(&handle),
        scope: Some(&scopes),
        context: Some(id),
        ..Start::new(kind, "/")
    };
    start(&state, &server, request).await
}

/// A small HTML page, for the browser an admin connects from.
fn page(status: StatusCode, message: &str) -> Response {
    let escaped = message.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let body = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Eventside</title></head><body><p>{escaped}</p></body></html>"
    );
    let mut res = (status, body).into_response();
    let headers = res.headers_mut();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Login,
    Signup,
    /// An admin connecting from the CLI.
    Admin,
    /// An organization's account connecting from the CLI.
    Org,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Login => "login",
            Self::Signup => "signup",
            Self::Admin => session::ADMIN,
            Self::Org => session::ORG,
        }
    }

    /// The kind of session a finished request of this kind makes.
    fn session_kind(kind: &str) -> &'static str {
        match kind {
            session::ADMIN => session::ADMIN,
            session::ORG => session::ORG,
            _ => session::ATTENDEE,
        }
    }
}

/// An authorization request to start.
struct Start<'a> {
    kind: Kind,
    expected_did: Option<&'a str>,
    login_hint: Option<&'a str>,
    prompt: Option<&'a str>,
    return_to: &'a str,
    /// Another scope list than sign-in's.
    scope: Option<&'a str>,
    /// What the callback finishes with: the `admin_connects` row.
    context: Option<&'a str>,
}

impl<'a> Start<'a> {
    fn new(kind: Kind, return_to: &'a str) -> Self {
        Self {
            kind,
            expected_did: None,
            login_hint: None,
            prompt: None,
            return_to,
            scope: None,
            context: None,
        }
    }
}

/// Pushes the authorization request, stores it as pending, and sends the
/// browser off with a pre-auth cookie that only it can bring back.
async fn start(state: &AppState, server: &AuthServer, request: Start<'_>) -> Response {
    let Start { kind, expected_did, login_hint, prompt, return_to, scope, context } = request;
    let secure = state.secure_cookies();
    let flow = kind.as_str();
    let request_state = random_token(24);
    let verifier = random_token(32);
    let dpop_key = EcKey::generate();
    let preauth = random_token(32);
    let par = ParRequest {
        state: &request_state,
        code_challenge: &sha256_b64(&verifier),
        dpop_key: &dpop_key,
        login_hint,
        prompt,
    };
    let scope = scope.unwrap_or(&state.oauth.scope);
    let client_id = state.oauth.client_id_for(scope);
    let authorize_url = match state.oauth.par_for(server, &par, scope).await {
        Ok(url) => url,
        Err(err) => {
            eprintln!("{flow}: pushed authorization request failed: {err}");
            return fail("server_unavailable", return_to, &[]);
        }
    };
    // `kind` says how the callback finishes: login and signup alike, or an
    // admin's or organization's connection. The client ID is the one `par`
    // pushed the request as; the callback redeems the code as it, whichever
    // instance (and scope list) answers.
    let stored = sqlx::query(
        "INSERT INTO oauth_requests (state, kind, client_id, pkce_verifier, dpop_key, issuer, expected_did, \
         preauth_hash, return_to, expires_at, context) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
    )
    .bind(&request_state)
    .bind(flow)
    .bind(&client_id)
    .bind(&verifier)
    .bind(dpop_key.private_jwk())
    .bind(&server.issuer)
    .bind(expected_did)
    .bind(sha256_b64(&preauth))
    .bind(return_to)
    .bind(now_ms() + PENDING_MS)
    .bind(context)
    .execute(&state.db)
    .await;
    if let Err(err) = stored {
        eprintln!("{flow}: could not store the pending request: {err}");
        return fail("server_error", return_to, &[]);
    }
    redirect(&authorize_url, &[cookies::set_preauth(&request_state, &preauth, secure)])
}

#[derive(sqlx::FromRow)]
struct Pending {
    kind: String,
    context: Option<String>,
    client_id: String,
    pkce_verifier: String,
    dpop_key: String,
    issuer: String,
    expected_did: Option<String>,
    preauth_hash: String,
    return_to: String,
    expires_at: i64,
}

pub async fn callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Params,
) -> Response {
    let secure = state.secure_cookies();
    let Some(request_state) = params.get("state") else {
        return fail("invalid_request", "/", &[]);
    };
    // Each sign-in has its own pre-auth cookie, so one tab's sign-in (or a
    // stray callback link) never disturbs another's.
    let clear = [cookies::clear_preauth(request_state, secure)];

    // Consume the pending request: whoever deletes it is the only one to use it.
    let pending = sqlx::query_as::<_, Pending>(
        "SELECT kind, context, client_id, pkce_verifier, dpop_key, issuer, expected_did, preauth_hash, return_to, \
         expires_at FROM oauth_requests WHERE state = $1",
    )
    .bind(request_state)
    .fetch_optional(&state.db)
    .await;
    let pending = match pending {
        Ok(Some(pending)) => pending,
        Ok(None) => return fail("invalid_request", "/", &clear),
        Err(err) => {
            eprintln!("callback: could not load the pending request: {err}");
            return fail("server_error", "/", &clear);
        }
    };
    let return_to = sanitize(Some(&pending.return_to));
    let consumed = sqlx::query("DELETE FROM oauth_requests WHERE state = $1")
        .bind(request_state)
        .execute(&state.db)
        .await;
    match consumed {
        Ok(done) if done.rows_affected() == 1 => {}
        // Another request consumed it first.
        Ok(_) => return fail("invalid_request", "/", &clear),
        Err(err) => {
            eprintln!("callback: could not consume the pending request: {err}");
            return fail("server_error", &return_to, &clear);
        }
    }

    // Checked before the browser is: the pre-auth cookie lives exactly as
    // long as the request, so a late callback has lost it by now. The
    // request is consumed either way.
    if pending.expires_at < now_ms() {
        return fail("request_expired", &return_to, &clear);
    }
    // Only the browser that started the sign-in may finish it.
    let preauth = cookies::preauth(&headers, request_state, secure).map(|p| sha256_b64(&p));
    if preauth.as_deref() != Some(pending.preauth_hash.as_str()) {
        return fail("invalid_request", &return_to, &clear);
    }
    // RFC 9207: the issuer is checked on errors too.
    if params.get("iss").map(String::as_str) != Some(pending.issuer.as_str()) {
        return fail("issuer_mismatch", &return_to, &clear);
    }
    if let Some(error) = params.get("error") {
        let code = if error == "access_denied" { "access_denied" } else { "authorization_failed" };
        return fail(code, &return_to, &clear);
    }
    let Some(code) = params.get("code") else { return fail("invalid_request", &return_to, &clear) };

    let connecting = Kind::session_kind(&pending.kind) != session::ATTENDEE;
    let connection = pending.context.clone().unwrap_or_default();
    match complete(&state, &headers, &pending, code).await {
        Ok(_) if connecting => {
            let done = sqlx::query(
                "UPDATE admin_connects SET completed_at = $1 WHERE id = $2 AND completed_at IS NULL",
            )
            .bind(now_ms())
            .bind(&connection)
            .execute(&state.db)
            .await;
            if let Err(err) = done {
                eprintln!("callback: could not record the connection: {err}");
                return page(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong. Try again.");
            }
            let mut res = page(StatusCode::OK, "Connected. You can close this tab.");
            if let Ok(value) = HeaderValue::from_str(&clear[0]) {
                res.headers_mut().append(SET_COOKIE, value);
            }
            res
        }
        Err(code) if connecting => {
            let _ = sqlx::query("UPDATE admin_connects SET error = $1 WHERE id = $2")
                .bind(code)
                .bind(&connection)
                .execute(&state.db)
                .await;
            page(
                StatusCode::BAD_REQUEST,
                &format!("Couldn't connect ({code}). Run the connect command again."),
            )
        }
        Ok(cookie) => {
            let max_age = state.config.session_idle_timeout + TOMBSTONE;
            redirect(
                &return_to,
                &[clear[0].clone(), cookies::set_session(&cookie, max_age, secure)],
            )
        }
        Err(code) => fail(code, &return_to, &clear),
    }
}

/// Exchanges the code, checks who came back and from where, and creates the
/// session. Returns the new session cookie, or the error code for the PWA.
async fn complete(
    state: &AppState,
    headers: &HeaderMap,
    pending: &Pending,
    code: &str,
) -> Result<String, &'static str> {
    let dpop_key = EcKey::from_jwk(&pending.dpop_key).map_err(|_| "server_error")?;
    let server = state.oauth.auth_server(&pending.issuer).await.map_err(|why| {
        eprintln!("callback: {why}");
        why.code()
    })?;
    let client_id = pending.client_id.as_str();
    let tokens = state
        .oauth
        .exchange_code(&server, client_id, code, &pending.pkce_verifier, &dpop_key)
        .await
        .map_err(|err| {
            eprintln!("callback: token exchange failed: {err}");
            match err {
                OAuthError::Unavailable(_) => "server_unavailable",
                _ => "authorization_failed",
            }
        })?;
    let revoke = || async {
        if let Some(refresh) = &tokens.refresh_token
            && let Err(err) = state.oauth.revoke(&server, client_id, refresh, &dpop_key).await
        {
            eprintln!("callback: could not revoke an unused grant: {err}");
        }
    };
    let Some(did) = tokens.sub.clone().filter(|sub| is_valid_did(sub)) else {
        revoke().await;
        return Err("authorization_failed");
    };
    if pending.expected_did.as_deref().is_some_and(|expected| expected != did) {
        revoke().await;
        return Err("account_mismatch");
    }

    // The DID's own PDS must use the server that answered.
    let identity = match state.resolver.resolve_did(&did).await {
        Ok(identity) => identity,
        Err(err) => {
            eprintln!("callback: {err:?}");
            revoke().await;
            return Err("resolution_failed");
        }
    };
    match state.oauth.discover(&identity.pds).await {
        Ok(theirs) if theirs.issuer == server.issuer => {}
        other => {
            eprintln!(
                "callback: {did}'s PDS doesn't use {}: {:?}",
                server.issuer,
                other.map(|s| s.issuer)
            );
            revoke().await;
            return Err("issuer_mismatch");
        }
    }
    // The grant is checked against the scopes this request was pushed with,
    // not this instance's: during a rolling deploy another instance may have
    // pushed it. If this instance asks for more, `session::outdated` sends
    // the person back through sign-in on their next request here.
    let scopes = tokens.scope.clone().unwrap_or_default();
    let granted: Vec<&str> = scopes.split_whitespace().collect();
    // Every pending request was pushed with a client ID this server built,
    // so its scopes are always recoverable; if not, the grant can't be checked.
    let Some(asked) = oauth::client_scopes(client_id) else {
        eprintln!("callback: no scope list in the client ID {client_id:?}");
        revoke().await;
        return Err("server_error");
    };
    if asked.iter().any(|s| !granted.contains(&s.as_str())) {
        revoke().await;
        return Err("scope_missing");
    }

    let profile = profile(state, &identity.pds, &did).await;
    let created = session::create(
        &state.db,
        NewSession {
            did: &did,
            handle: &identity.handle,
            display_name: profile.display_name.as_deref(),
            avatar: profile.avatar.as_deref(),
            pds: &identity.pds,
            dpop_key: &pending.dpop_key,
            issuer: &server.issuer,
            access_token: &tokens.access_token,
            refresh_token: tokens.refresh_token.as_deref(),
            token_expires_at: tokens.expires_at(now_ms()),
            scopes: &scopes,
            client_id,
            kind: Kind::session_kind(&pending.kind),
        },
    )
    .await;
    let cookie = match created {
        Ok(cookie) => cookie,
        Err(err) => {
            // The browser keeps whatever session it had; nothing holds the new grant.
            eprintln!("callback: could not store the session: {err}");
            revoke().await;
            return Err("server_error");
        }
    };
    let kind = Kind::session_kind(&pending.kind);
    if kind != session::ATTENDEE {
        // One admin (or organization) session per account: connecting again
        // replaces the last. The browser's own session is left alone.
        let id_hash = sha256_b64(&cookie);
        let older = sqlx::query_as::<_, (String,)>(
            "SELECT id_hash FROM sessions WHERE did = $1 AND kind = $2 AND ended_at IS NULL AND id_hash <> $3",
        )
        .bind(&did)
        .bind(kind)
        .bind(&id_hash)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();
        for (old,) in older {
            retire(state, &old).await;
        }
        return Ok(cookie);
    }
    // Only now is whatever session this browser had replaced, never promoted.
    if let Some(old) = cookies::session(headers, state.secure_cookies()) {
        retire(state, &sha256_b64(&old)).await;
    }
    Ok(cookie)
}

#[derive(Default)]
struct Profile {
    display_name: Option<String>,
    avatar: Option<String>,
}

/// The person's public `app.bsky.actor.profile`, if they have one.
async fn profile(state: &AppState, pds: &str, did: &str) -> Profile {
    let url = format!("{pds}/xrpc/com.atproto.repo.getRecord");
    let Ok(client) = state.http.guarded(&url) else { return Profile::default() };
    let res = client
        .get(&url)
        .query(&[("repo", did), ("collection", "app.bsky.actor.profile"), ("rkey", "self")])
        .send()
        .await;
    let Ok(res) = res else { return Profile::default() };
    if !res.status().is_success() {
        return Profile::default();
    }
    let Ok(record) = crate::net::read_json::<Value>(res).await else { return Profile::default() };
    let value = &record["value"];
    let display_name = value["displayName"]
        .as_str()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(|n| truncate(n, MAX_DISPLAY_NAME_CHARS, MAX_DISPLAY_NAME_BYTES));
    let avatar = value["avatar"]["ref"]["$link"].as_str().map(|cid| {
        let mut url = url::Url::parse(&format!("{pds}/xrpc/com.atproto.sync.getBlob"))
            .expect("the PDS URL was fetched");
        url.query_pairs_mut().append_pair("did", did).append_pair("cid", cid);
        url.to_string()
    });
    Profile { display_name, avatar }
}

/// The lexicon's limits on `displayName`. Characters stand in for graphemes:
/// never more graphemes than characters.
const MAX_DISPLAY_NAME_CHARS: usize = 64;
const MAX_DISPLAY_NAME_BYTES: usize = 640;

/// At most `chars` characters and `bytes` bytes of `s`, cut on a character boundary.
fn truncate(s: &str, chars: usize, bytes: usize) -> String {
    let mut out = String::new();
    for c in s.chars().take(chars) {
        if out.len() + c.len_utf8() > bytes {
            break;
        }
        out.push(c);
    }
    out
}

/// Deletes a session, revoking the grant it held in the background.
async fn retire(state: &AppState, id_hash: &str) {
    let row = match session::delete(&state.db, id_hash).await {
        Ok(Some(row)) => row,
        Ok(None) => return,
        Err(err) => {
            eprintln!("could not delete a session: {err}");
            return;
        }
    };
    let state = state.clone();
    tokio::spawn(async move { session::revoke(&state, &row).await });
}

/// Signs out: revokes the grant, deletes the session and clears the cookie
/// the request carried. The CSRF layer has already checked the token.
pub async fn logout(
    State(state): State<AppState>,
    checked: Option<Extension<CheckedSession>>,
    headers: HeaderMap,
) -> Response {
    let secure = state.secure_cookies();
    let row = match checked {
        // A live session, whose CSRF token the layer has checked.
        Some(Extension(CheckedSession(row))) => Some(row),
        // The layer lets an ended or unknown session through without a token.
        None => match session::lookup(&state, &headers).await {
            Ok(Lookup::Expired(row)) => Some(row),
            // Gone (another sign-out, or never there): only the cookie is
            // cleared. Never a live session unchecked.
            Ok(Lookup::Live(_) | Lookup::None) => None,
            Err(err) => {
                eprintln!("sign-out: {err}");
                return xrpc_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "InternalServerError",
                    "could not sign out",
                );
            }
        },
    };
    if let Some(row) = row {
        // Signed out first, whatever the authorization server does. The
        // tokens revoked are the ones deleted: a renewal may have replaced
        // them since `row` was read.
        let row = match session::delete(&state.db, &row.id_hash).await {
            Ok(deleted) => deleted,
            Err(err) => {
                eprintln!("sign-out: could not delete the session: {err}");
                return xrpc_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "InternalServerError",
                    "could not sign out",
                );
            }
        };
        // Then revoke, waiting briefly so the grant is usually gone by the
        // time sign-out returns; a slow server finishes in the background.
        // Already gone means another request deleted it, and revokes it.
        if let Some(row) = row {
            let background = state.clone();
            let revoking = tokio::spawn(async move { session::revoke(&background, &row).await });
            let _ = tokio::time::timeout(REVOKE_WAIT, revoking).await;
        }
    }
    let mut res = Json(json!({})).into_response();
    // Only a cookie the request carried is cleared. A cross-site form POST
    // carries none (it's `SameSite=Lax`), so it can't clear the cookie of a
    // session it can't end.
    if cookies::session(&headers, secure).is_some()
        && let Ok(value) = HeaderValue::from_str(&cookies::clear_session(secure))
    {
        res.headers_mut().insert(SET_COOKIE, value);
    }
    res
}

/// `app.eventside.auth.getSession`: who's signed in.
pub async fn get_session(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let row = match session::lookup(&state, &headers).await {
        Ok(Lookup::Live(row)) => row,
        Ok(Lookup::Expired(row)) => return super::expired(&row),
        Ok(Lookup::None) => return super::auth_required(),
        Err(err) => {
            eprintln!("getSession: {err}");
            return xrpc_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "InternalServerError",
                "could not load the session",
            );
        }
    };
    if let Err(err) = session::touch(&state, &row).await {
        eprintln!("getSession: could not record use: {err}");
    }
    let mut body = json!({
        "did": row.did,
        "handle": row.handle,
        "scopes": row.scope_list(),
        "csrfToken": row.csrf_token,
    });
    if let Some(name) = &row.display_name {
        body["displayName"] = json!(name);
    }
    if let Some(avatar) = &row.avatar {
        body["avatar"] = json!(avatar);
    }
    let mut res = Json(body).into_response();
    res.headers_mut().insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    // Using the session keeps its cookie alive too: renewed on every answer,
    // not only when `last_seen_at` moves, since other routes move that
    // without renewing the cookie.
    if let Some(cookie) = cookies::session(&headers, state.secure_cookies())
        && let Ok(value) = HeaderValue::from_str(&cookies::set_session(
            &cookie,
            state.config.session_idle_timeout + TOMBSTONE,
            state.secure_cookies(),
        ))
    {
        res.headers_mut().insert(SET_COOKIE, value);
    }
    res
}
