//! Space credentials: trading a user's delegation token for one
//! (`getSpaceCredential`), checking the ones presented to our host methods,
//! and minting our own, so the appview reads a space like any other app.
//!
//! The wire format is vivarium 0.0.2's: an app proves its P-256 key with an
//! RFC 9421 HTTP message signature (label `atproto-space`), and the credential
//! is a JWT signed by the authority's `#atproto_space` key, bound to that key
//! by `cnf.kid`.

use std::net::IpAddr;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use super::index;
use super::{SpaceUri, authority, host_audience, internal, invalid, refuse};
use crate::AppState;
use crate::crypto::{Jwt, PublicKey};
use crate::db::now_ms;
use crate::keys::{EcKey, now_secs, random_token};

const DELEGATION_TYP: &str = "atproto-space-delegation+jwt";
const CREDENTIAL_TYP: &str = "atproto-space-credential+jwt";
const ATTESTATION_TYP: &str = "atproto-client-attestation+jwt";
const LABEL: &str = "atproto-space";

/// How long the credentials we issue last.
pub const CREDENTIAL_SECS: i64 = 600;

/// A credential for `space`, bound to the P-256 key `key_id`, signed by the
/// space's authority. Returns it with its `jti` and expiry (epoch seconds).
pub async fn mint(
    state: &AppState,
    space: &SpaceUri,
    key_id: &str,
) -> Result<(String, String, i64), String> {
    let key = authority::space_key(state, &space.authority).await?;
    let now = now_secs();
    let jti = random_token(16);
    let exp = now + CREDENTIAL_SECS;
    let credential = key.sign(
        json!({ "alg": "ES256", "typ": CREDENTIAL_TYP, "kid": "#atproto_space" }),
        &json!({
            "iss": space.authority,
            "sub": space.to_string(),
            "cnf": { "kid": key_id },
            "iat": now,
            "exp": exp,
            "jti": jti,
        }),
    );
    Ok((credential, jti, exp))
}

/// A credential for our own reads of a space, bound to the appview's key.
pub async fn for_self(state: &AppState, space: &SpaceUri) -> Result<String, String> {
    Ok(mint(state, space, &state.oauth.key.did_key()).await?.0)
}

/// The headers that present a credential (or, without an audience, a
/// delegation token) with an `atproto-space` signature by `key`.
pub fn signed_headers(
    key: &EcKey,
    authorization: &str,
    audience: Option<&str>,
) -> Vec<(&'static str, String)> {
    let input = match audience {
        None => format!("(\"authorization\");keyid=\"{}\"", key.did_key()),
        Some(_) => "(\"authorization\" \"atproto-space-audience\")".to_owned(),
    };
    let base = signature_base(authorization, audience, &input);
    let signature = STANDARD.encode(key.sign_bytes(base.as_bytes()));
    let mut headers = vec![("authorization", authorization.to_owned())];
    if let Some(audience) = audience {
        headers.push(("atproto-space-audience", audience.to_owned()));
    }
    headers.push(("signature-input", format!("{LABEL}={input}")));
    headers.push(("signature", format!("{LABEL}=:{signature}:")));
    headers
}

fn signature_base(authorization: &str, audience: Option<&str>, input: &str) -> String {
    let mut lines = vec![format!("\"authorization\": {}", authorization.trim())];
    if let Some(audience) = audience {
        lines.push(format!("\"atproto-space-audience\": {}", audience.trim()));
    }
    lines.push(format!("\"@signature-params\": {input}"));
    lines.join("\n")
}

/// Checks a request's `atproto-space` signature. Without `key_id` (trading a
/// delegation token) it must cover `authorization` and name its key in
/// `keyid`; with one (using a credential) it must cover `authorization` and
/// `atproto-space-audience`, by that key. Returns the key.
pub fn verify_signature(headers: &HeaderMap, key_id: Option<&str>) -> Result<String, String> {
    let single = |name: &str| -> Result<String, String> {
        let mut values = headers.get_all(name).iter();
        match (values.next().and_then(|v| v.to_str().ok()), values.next()) {
            (Some(v), None) if !v.contains(',') => Ok(v.to_owned()),
            _ => Err(format!("the request needs exactly one {name} field")),
        }
    };
    let input_header = single("signature-input")?;
    let signature_header = single("signature")?;
    let input = input_header
        .strip_prefix(&format!("{LABEL}="))
        .ok_or("the signature isn't labelled atproto-space")?
        .trim();
    let signature = signature_header
        .strip_prefix(&format!("{LABEL}="))
        .and_then(|s| s.strip_prefix(':'))
        .and_then(|s| s.strip_suffix(':'))
        .ok_or("the signature isn't labelled atproto-space")?;
    let signature = STANDARD.decode(signature).map_err(|_| "the signature isn't base64")?;
    let close = input.find(')').ok_or("malformed signature-input")?;
    let components: Vec<&str> = input.strip_prefix('(').ok_or("malformed signature-input")?
        [..close - 1]
        .split_whitespace()
        .collect();
    let expected: &[&str] = match key_id {
        None => &["\"authorization\""],
        Some(_) => &["\"authorization\"", "\"atproto-space-audience\""],
    };
    if components != expected {
        return Err(format!("the signature must cover exactly {}", expected.join(" ")));
    }
    let mut keyid = None;
    for param in input[close + 1..].split(';').filter(|p| !p.is_empty()) {
        let (name, value) = param.split_once('=').ok_or("malformed signature parameters")?;
        let value = value.trim_matches('"');
        match name {
            "keyid" => keyid = Some(value.to_owned()),
            "alg" if value != "ecdsa-p256-sha256" => {
                return Err("the signature must be ecdsa-p256-sha256".into());
            }
            _ => {}
        }
    }
    if let (Some(expected), Some(named)) = (key_id, &keyid)
        && expected != named
    {
        return Err("the signature's keyid isn't the credential's key".into());
    }
    let signing_key = key_id.map(str::to_owned).or(keyid).ok_or("the signature names no key")?;
    let key = PublicKey::from_did_key(&signing_key)?;
    if !matches!(key, PublicKey::P256(_)) {
        return Err("the signature key must be a P-256 did:key".into());
    }
    let authorization = single("authorization")?;
    let audience = key_id.map(|_| single("atproto-space-audience")).transpose()?;
    let base = signature_base(&authorization, audience.as_deref(), input);
    if signature.len() != 64 || !key.verify(base.as_bytes(), &signature) {
        return Err("invalid HTTP message signature".into());
    }
    Ok(signing_key)
}

fn unauthorized(error: &str, message: impl std::fmt::Display) -> Response {
    refuse(StatusCode::UNAUTHORIZED, error, &message.to_string())
}

/// `com.atproto.space.getSpaceCredential`.
pub async fn get_space_credential(
    State(state): State<AppState>,
    super::ClientIp(ip): super::ClientIp,
    headers: HeaderMap,
    body: Option<Json<Value>>,
) -> Response {
    let body = body.map(|Json(b)| b).unwrap_or_default();
    let Some(space) = body.get("space").and_then(Value::as_str).and_then(SpaceUri::parse) else {
        return invalid("InvalidRequest", "Input must have a valid \"space\"");
    };
    if let Some(ip) = ip
        && !ip_allowed(&state, "credential", ip)
    {
        return refuse(StatusCode::TOO_MANY_REQUESTS, "RateLimitExceeded", "Too many requests");
    }
    let Some(token) = bearer(&headers) else {
        return unauthorized("MissingJwt", "Delegation token required");
    };
    let delegation = match Jwt::decode(&token) {
        Ok(jwt) => jwt,
        Err(why) => return unauthorized("BadJwt", why),
    };
    if delegation.header_str("typ") != Some(DELEGATION_TYP) {
        return unauthorized("BadJwtType", "not a delegation token");
    }
    let Some(user) =
        delegation.claim_str("iss").filter(|d| crate::identity::is_valid_did(d)).map(str::to_owned)
    else {
        return unauthorized("BadJwtIss", "the delegation token names no user");
    };
    if delegation.claim_str("aud") != Some(host_audience(&space.authority).as_str()) {
        return unauthorized("BadJwtAudience", "the delegation token isn't for this space host");
    }
    if delegation.expired(now_secs()) {
        return unauthorized("JwtExpired", "the delegation token has expired");
    }
    if delegation.claim_str("sub") != Some(space.to_string().as_str()) {
        return invalid(
            "InvalidDelegationToken",
            "Delegation token subject does not match requested space",
        );
    }
    let org = match index::load_for_space(&state, &space).await {
        Ok(Some(org)) if org.knows(&space) => org,
        Ok(_) => return refuse(StatusCode::NOT_FOUND, "SpaceNotFound", "Space not found"),
        Err(why) => return internal(why),
    };

    // The app perimeter first, before resolving or verifying anything.
    let attestation = body.get("clientAttestation").and_then(Value::as_str);
    let claimed_client = attestation
        .and_then(|a| Jwt::decode(a).ok())
        .and_then(|a| a.claim_str("iss").map(str::to_owned));
    let access = org.app_access(&space);
    if !access.allows(claimed_client.as_deref(), &state.oauth.public_url) {
        return invalid("AppNotAuthorized", "Application not authorized for this space");
    }

    // Then the user's delegation, before anything is said or counted about
    // them: until it's verified, `iss` is anyone's claim.
    match state.host.signing_key(&state, &user, delegation.header_str("kid")).await {
        Ok(key) => {
            if let Err(why) = delegation.verify(&key) {
                return unauthorized("BadJwtSignature", why);
            }
        }
        Err(why) => {
            return unauthorized(
                "BadJwtSignature",
                format!("could not resolve the user's key: {why}"),
            );
        }
    }
    if !state.host.allow(&format!("credential:{user}"), 120, 60_000) {
        return refuse(StatusCode::TOO_MANY_REQUESTS, "RateLimitExceeded", "Too many requests");
    }
    if !org.can_read(&space, &user) {
        return invalid("UserNotAuthorized", "User not authorized for this space");
    }

    // Now the app's key, and its client ID.
    let key_id = match verify_signature(&headers, None) {
        Ok(key) => key,
        Err(why) => return unauthorized("BadSpaceSignature", why),
    };
    let jti = delegation.claim_str("jti").unwrap_or_default().to_owned();
    if jti.is_empty() {
        return unauthorized("BadJwt", "a delegation token requires a jti");
    }
    if !state.host.consume("delegation", &jti, delegation.claim_i64("exp").unwrap_or_default()) {
        return unauthorized("JwtReplayed", "delegation token has already been used");
    }
    let client_id = match attestation {
        Some(attestation) => match verify_attestation(&state, attestation, &space).await {
            Ok(client_id) => Some(client_id),
            Err(why) => return invalid("InvalidClientAttestation", &why),
        },
        None => None,
    };
    if !access.allows(client_id.as_deref(), &state.oauth.public_url) {
        return invalid("AppNotAuthorized", "Application not authorized for this space");
    }

    let (credential, jti, exp) = match mint(&state, &space, &key_id).await {
        Ok(minted) => minted,
        Err(why) => return internal(why),
    };
    let recorded = sqlx::query(
        "INSERT INTO space_credentials (jti, space, delegator, client_id, expires_at) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&jti)
    .bind(space.to_string())
    .bind(&user)
    .bind(client_id.as_deref())
    .bind(exp * 1000)
    .execute(&state.db)
    .await;
    if let Err(err) = recorded {
        return internal(format!("could not record a credential: {err}"));
    }
    Json(json!({ "credential": credential })).into_response()
}

/// Checks a client attestation, returning the client ID it proves.
async fn verify_attestation(
    state: &AppState,
    attestation: &str,
    space: &SpaceUri,
) -> Result<String, String> {
    let jwt = Jwt::decode(attestation)?;
    if jwt.header_str("typ") != Some(ATTESTATION_TYP) {
        return Err("not a client attestation".into());
    }
    let client_id = jwt.claim_str("iss").ok_or("the attestation names no client")?.to_owned();
    if jwt.claim_str("sub") != Some(client_id.as_str()) {
        return Err("a client attestation's iss and sub must both be the client ID".into());
    }
    if jwt.claim_str("aud") != Some(host_audience(&space.authority).as_str()) {
        return Err("the attestation isn't for this space host".into());
    }
    let now = now_secs();
    let exp = jwt.claim_i64("exp").ok_or("the attestation has no exp")?;
    if jwt.expired(now) || exp - now > 65 {
        return Err("the attestation has expired, or lasts too long".into());
    }
    let jwks = client_jwks(state, &client_id).await?;
    let kid = jwt.header_str("kid");
    let jwk = jwks
        .iter()
        .find(|k| kid.is_none() || k.get("kid").and_then(Value::as_str) == kid)
        .ok_or("the attestation's key isn't in the client's jwks")?;
    let bare = json!({ "kty": jwk.get("kty"), "crv": jwk.get("crv"), "x": jwk.get("x"), "y": jwk.get("y") });
    let key = p256::PublicKey::from_jwk_str(&bare.to_string())
        .map_err(|e| format!("the client's key isn't P-256: {e}"))?;
    jwt.verify(&PublicKey::P256(p256::ecdsa::VerifyingKey::from(&key)))?;
    let jti = jwt.claim_str("jti").ok_or("the attestation has no jti")?;
    if !state.host.consume("attestation", jti, exp) {
        return Err("Client attestation already used".into());
    }
    Ok(client_id)
}

/// A client's published keys, from its metadata (`jwks`, or `jwks_uri`).
async fn client_jwks(state: &AppState, client_id: &str) -> Result<Vec<Value>, String> {
    let get = |url: String| async move {
        let client = state.http.guarded(&url)?;
        let res = client.get(&url).send().await.map_err(|e| format!("{url}: {e}"))?;
        if !res.status().is_success() {
            return Err(format!("{url} answered {}", res.status()));
        }
        crate::net::read_json::<Value>(res).await
    };
    let metadata = get(client_id.to_owned()).await?;
    if metadata.get("client_id").and_then(Value::as_str) != Some(client_id) {
        return Err("the client's metadata is for another client ID".into());
    }
    let jwks = match (metadata.get("jwks"), metadata.get("jwks_uri").and_then(Value::as_str)) {
        (Some(jwks), _) => jwks.clone(),
        (None, Some(uri)) => get(uri.to_owned()).await?,
        (None, None) => return Err("the client publishes no keys".into()),
    };
    Ok(jwks.get("keys").and_then(Value::as_array).cloned().unwrap_or_default())
}

/// Who presented a credential to one of our host methods.
pub struct Caller {
    pub key_id: String,
    pub delegator: Option<String>,
    pub client_id: Option<String>,
}

/// Checks the credential a host method was called with: ours, for this
/// space, not revoked, presented by its key, with the authority as audience.
pub async fn check(
    state: &AppState,
    headers: &HeaderMap,
    space: &SpaceUri,
) -> Result<Caller, Response> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split_once(' '))
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("atproto-space"))
        .map(|(_, token)| token.trim().to_owned())
        .ok_or_else(|| {
            refuse(
                StatusCode::UNAUTHORIZED,
                "AuthRequired",
                "This method requires a space credential",
            )
        })?;
    let jwt = Jwt::decode(&token).map_err(|why| unauthorized("BadJwt", why))?;
    if jwt.header_str("typ") != Some(CREDENTIAL_TYP) {
        return Err(unauthorized("BadJwtType", "not a space credential"));
    }
    if jwt.claim_str("iss") != Some(space.authority.as_str()) {
        return Err(unauthorized(
            "BadJwtIss",
            "space credential issuer is not the space authority",
        ));
    }
    if jwt.expired(now_secs()) {
        return Err(unauthorized("JwtExpired", "the credential has expired"));
    }
    let key = authority::space_key(state, &space.authority).await.map_err(internal)?;
    jwt.verify(&key.verifying_key()).map_err(|why| unauthorized("BadJwtSignature", why))?;
    let audience =
        headers.get("atproto-space-audience").and_then(|v| v.to_str().ok()).unwrap_or_default();
    if !audience.starts_with("did:") {
        return Err(unauthorized("BadSpaceSignature", "missing or invalid space audience DID"));
    }
    let key_id = jwt
        .payload
        .get("cnf")
        .and_then(|c| c.get("kid"))
        .and_then(Value::as_str)
        .ok_or_else(|| unauthorized("BadJwtCnf", "the credential is bound to no key"))?
        .to_owned();
    verify_signature(headers, Some(&key_id))
        .map_err(|why| unauthorized("BadSpaceSignature", why))?;
    let jti = jwt.claim_str("jti").unwrap_or_default();
    let issued = sqlx::query_as::<_, (String, Option<String>, Option<i64>)>(
        "SELECT delegator, client_id, revoked_at FROM space_credentials WHERE jti = $1",
    )
    .bind(jti)
    .fetch_optional(&state.db)
    .await
    .map_err(internal)?;
    let revoked = || unauthorized("CredentialRevoked", "space credential has been revoked");
    if issued.as_ref().is_some_and(|(_, _, revoked)| revoked.is_some()) {
        return Err(revoked());
    }
    // Revocation is pushed when access changes; this catches a credential
    // used before that lands: its user and its app must still have access.
    if let Some((delegator, client_id, _)) = &issued {
        let org = index::load_for_space(state, space).await.map_err(internal)?;
        let still = org.is_some_and(|org| {
            org.can_read(space, delegator)
                && org.app_access(space).allows(client_id.as_deref(), &state.oauth.public_url)
        });
        if !still {
            return Err(revoked());
        }
    }
    if audience != space.authority {
        return Err(unauthorized("BadSpaceAudience", "space audience does not match the request"));
    }
    if jwt.claim_str("sub") != Some(space.to_string().as_str()) {
        return Err(refuse(
            StatusCode::FORBIDDEN,
            "InvalidCredential",
            "Credential is not scoped to this space",
        ));
    }
    let (delegator, client_id) = issued.map_or((None, None), |(d, c, _)| (Some(d), c));
    Ok(Caller { key_id, delegator, client_id })
}

fn bearer(headers: &HeaderMap) -> Option<String> {
    let value = headers.get("authorization")?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    scheme.eq_ignore_ascii_case("bearer").then(|| token.trim().to_owned())
}

/// The per-IP limit on the host's public endpoints.
pub fn ip_allowed(state: &AppState, what: &str, ip: IpAddr) -> bool {
    state.host.allow_ip(state, what, ip, 600)
}

/// Marks every unexpired credential `delegator` was issued for `space` as
/// revoked, returning their `jti`s.
pub async fn revoke_delegated(
    state: &AppState,
    space: &str,
    delegator: &str,
) -> Result<Vec<String>, String> {
    revoke_where(state, space, |d, _| d == delegator).await
}

/// Marks every unexpired, unrevoked credential for `space` that `lost` picks
/// (by delegator and client ID) as revoked, returning their `jti`s.
pub async fn revoke_where(
    state: &AppState,
    space: &str,
    lost: impl Fn(&str, Option<&str>) -> bool,
) -> Result<Vec<String>, String> {
    let now = now_ms();
    let issued = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT jti, delegator, client_id FROM space_credentials WHERE space = $1 AND revoked_at IS NULL AND expires_at > $2",
    )
    .bind(space)
    .bind(now)
    .fetch_all(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    let mut jtis = Vec::new();
    for (jti, delegator, client_id) in issued {
        if !lost(&delegator, client_id.as_deref()) {
            continue;
        }
        sqlx::query("UPDATE space_credentials SET revoked_at = $1 WHERE jti = $2")
            .bind(now)
            .bind(&jti)
            .execute(&state.db)
            .await
            .map_err(|e| e.to_string())?;
        jtis.push(jti);
    }
    Ok(jtis)
}
