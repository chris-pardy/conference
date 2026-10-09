//! The conference XRPC the PWA uses, what the organization's PDS asks
//! eventside (`checkUserAccess`), and eventside's own DID document.

use axum::Json;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};

use super::decide::{self, Action, Actor, Outcome, Refusal, Role};
use super::join::{self, Joined};
use super::{Conference, EVENT, SpaceUri, attest, eventside_did};
use crate::AppState;
use crate::auth::session::{self, Lookup};
use crate::auth::{CurrentUser, xrpc_error};
use crate::crypto::{Jwt, PublicKey};
use crate::keys::now_secs;

fn server_error(why: &str) -> Response {
    eprintln!("conference: {why}");
    xrpc_error(StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError", "Something went wrong.")
}

fn not_found() -> Response {
    xrpc_error(StatusCode::NOT_FOUND, "NotFound", "There's no conference here.")
}

/// A conference by what the app was given: its space URI, or its public
/// event's AT-URI with the organization named by DID or handle.
async fn find(state: &AppState, uri: &str) -> Result<Option<Conference>, String> {
    if SpaceUri::parse(uri).is_some() {
        return super::load(&state.db, uri).await;
    }
    let Some(rest) = uri.strip_prefix("at://") else { return Ok(None) };
    let mut parts = rest.split('/');
    let (Some(actor), Some(EVENT), Some(rkey), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Ok(None);
    };
    let org = if actor.starts_with("did:") {
        actor.to_owned()
    } else {
        match state.resolver.resolve_handle(actor).await {
            Ok(did) => did,
            Err(crate::identity::IdentityError::HandleNotFound) => return Ok(None),
            Err(crate::identity::IdentityError::Unresolvable(why)) => return Err(why),
        }
    };
    super::by_event(&state.db, &org, rkey).await
}

/// What anyone may see of a conference: its public page.
pub fn public_view(conference: &Conference, public_url: &str) -> Value {
    let mut view = json!({
        "space": conference.space,
        "event": conference.event,
        "url": conference.page_url(public_url),
        "name": conference.name,
        "startsAt": conference.starts_at,
        "endsAt": conference.ends_at,
        "locations": [{ "locality": conference.city }],
        "join": { "methods": conference.methods },
    });
    if let Some(description) = &conference.description {
        view["description"] = json!(description);
    }
    if conference.theme.is_object() {
        view["theme"] = conference.theme.clone();
    }
    view
}

#[derive(Deserialize)]
pub struct ConferenceParam {
    conference: Option<String>,
}

/// `app.eventside.conference.get`: a conference's public view, and, when
/// signed in, whether the viewer is a member and in which role.
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ConferenceParam>,
) -> Response {
    let Some(uri) = params.conference else {
        return xrpc_error(StatusCode::BAD_REQUEST, "InvalidRequest", "Name a conference.");
    };
    let conference = match find(&state, &uri).await {
        Ok(Some(conference)) => conference,
        Ok(None) => return not_found(),
        Err(why) => return server_error(&why),
    };
    let mut view = public_view(&conference, &state.oauth.public_url);
    // Signed in or not, the public page answers; an ended session is signed out.
    if let Ok(Lookup::Live(row)) = session::lookup(&state, &headers).await {
        match decide::role(&state.db, &conference.space, &row.did).await {
            Ok(role) => view["viewer"] = viewer(role),
            Err(why) => return server_error(&why),
        }
    }
    Json(view).into_response()
}

fn viewer(role: Option<Role>) -> Value {
    match role {
        Some(role) => json!({ "member": true, "role": role.as_str() }),
        None => json!({ "member": false }),
    }
}

#[derive(Deserialize)]
pub struct JoinInput {
    conference: Option<String>,
    code: Option<String>,
}

/// `app.eventside.conference.join`: answers `joined` or `refused` (and, for
/// the methods that come later, `pending` or `emailNeeded`).
pub async fn join(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(input): Json<JoinInput>,
) -> Response {
    let Some(uri) = input.conference else {
        return xrpc_error(StatusCode::BAD_REQUEST, "InvalidRequest", "Name a conference.");
    };
    let conference = match find(&state, &uri).await {
        Ok(Some(conference)) => conference,
        Ok(None) => return not_found(),
        Err(why) => return server_error(&why),
    };
    match join::join(&state, &conference, &user.did, input.code.as_deref()).await {
        Ok(Joined::Joined) => {
            super::outbox::kick();
            Json(json!({ "status": "joined", "conference": conference.space })).into_response()
        }
        Ok(Joined::Refused) => Json(json!({ "status": "refused" })).into_response(),
        Ok(Joined::SlowDown) => xrpc_error(
            StatusCode::TOO_MANY_REQUESTS,
            "RateLimitExceeded",
            "Too many tries. Wait a few minutes and try again.",
        ),
        Err(why) => server_error(&why),
    }
}

#[derive(Deserialize)]
pub struct LeaveInput {
    conference: Option<String>,
}

/// `app.eventside.conference.leave`. What the person wrote stays in their
/// repo; eventside stops serving it while they're not a member.
pub async fn leave(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(input): Json<LeaveInput>,
) -> Response {
    let Some(uri) = input.conference else {
        return xrpc_error(StatusCode::BAD_REQUEST, "InvalidRequest", "Name a conference.");
    };
    let conference = match find(&state, &uri).await {
        Ok(Some(conference)) => conference,
        Ok(None) => return not_found(),
        Err(why) => return server_error(&why),
    };
    match decide::decide(&state, &conference.space, &user.did, Action::Leave, &Actor::Subject).await
    {
        Ok(Outcome::Decided(_)) => {
            super::outbox::kick();
            Json(json!({})).into_response()
        }
        Ok(Outcome::Unchanged) => Json(json!({})).into_response(),
        Ok(Outcome::Refused(Refusal::LastOwner)) => xrpc_error(
            StatusCode::BAD_REQUEST,
            "LastOwner",
            "You're the conference's only owner: make someone else an owner first.",
        ),
        Ok(Outcome::Refused(refusal)) => {
            xrpc_error(StatusCode::BAD_REQUEST, "Refused", &refusal.to_string())
        }
        Err(why) => server_error(&why),
    }
}

/// `app.eventside.conference.getMembership`: whether the signed-in person is
/// a member now, and in which role. Checked on every request.
pub async fn get_membership(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(params): Query<ConferenceParam>,
) -> Response {
    let Some(uri) = params.conference else {
        return xrpc_error(StatusCode::BAD_REQUEST, "InvalidRequest", "Name a conference.");
    };
    let conference = match find(&state, &uri).await {
        Ok(Some(conference)) => conference,
        // Nothing here, as for any conference you're not in.
        Ok(None) => return Json(json!({ "member": false })).into_response(),
        Err(why) => return server_error(&why),
    };
    match decide::role(&state.db, &conference.space, &user.did).await {
        Ok(role) => Json(viewer(role)).into_response(),
        Err(why) => server_error(&why),
    }
}

#[derive(Deserialize)]
pub struct AccessParams {
    space: Option<String>,
    user: Option<String>,
    access: Option<String>,
    #[serde(rename = "clientId")]
    client_id: Option<String>,
}

/// `com.atproto.simplespace.checkUserAccess`: what the organization's PDS
/// asks eventside, the managing app, before letting someone read or write
/// a conference space. The PDS signs the question as the space's authority.
///
/// - **write:** the user is a member now.
/// - **read, as eventside's own client:** the user is a member (the PDS
///   then gives them their own records; eventside serves the rest).
/// - **read, as another client:** the client is allowed to read, and the
///   user is an owner or staff member.
/// - Anyone else is denied.
pub async fn check_user_access(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<AccessParams>,
) -> Response {
    let (Some(space), Some(user), Some(access)) = (params.space, params.user, params.access) else {
        return xrpc_error(
            StatusCode::BAD_REQUEST,
            "InvalidRequest",
            "space, user and access are required",
        );
    };
    let Some(parsed) = SpaceUri::parse(&space) else {
        return xrpc_error(StatusCode::BAD_REQUEST, "InvalidRequest", "space must be a space URI");
    };
    if let Err(why) = check_service_auth(
        &state,
        &headers,
        &parsed.authority,
        "com.atproto.simplespace.checkUserAccess",
    )
    .await
    {
        return xrpc_error(StatusCode::UNAUTHORIZED, "AuthenticationRequired", &why);
    }
    let authorized =
        match authorized(&state, &space, &user, &access, params.client_id.as_deref()).await {
            Ok(authorized) => authorized,
            Err(why) => return server_error(&why),
        };
    Json(json!({ "authorized": authorized })).into_response()
}

/// The access rules, behind one function, so an event host can later call
/// them in-process.
pub async fn authorized(
    state: &AppState,
    space: &str,
    user: &str,
    access: &str,
    client_id: Option<&str>,
) -> Result<bool, String> {
    let Some(conference) = super::load(&state.db, space).await? else { return Ok(false) };
    let role = decide::role(&state.db, &conference.space, user).await?;
    Ok(match access {
        "write" => role.is_some(),
        "read" => match client_id {
            Some(id) if state.oauth.is_own_client_id(id) => role.is_some(),
            Some(id) => {
                role.is_some_and(Role::is_admin) && app_allowed(state, space, id, "read").await?
            }
            None => false,
        },
        _ => false,
    })
}

/// Whether a conference allows an app (by client ID) a use.
pub async fn app_allowed(
    state: &AppState,
    space: &str,
    client_id: &str,
    used_for: &str,
) -> Result<bool, String> {
    let uses = sqlx::query_scalar::<_, String>(
        "SELECT uses FROM conference_apps WHERE conference = $1 AND client_id = $2",
    )
    .bind(space)
    .bind(client_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    Ok(uses.is_some_and(|uses| super::split_methods(&uses).iter().any(|u| u == used_for)))
}

/// Checks the service JWT a PDS sent eventside: issued by `issuer` (the
/// space's authority), signed with its `#atproto` key, for eventside (its
/// DID, or its access service) and for exactly the method `lxm`, and not
/// expired.
pub async fn check_service_auth(
    state: &AppState,
    headers: &HeaderMap,
    issuer: &str,
    lxm: &str,
) -> Result<(), String> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or("a service JWT from the space's authority is required")?;
    let jwt = Jwt::decode(token)?;
    if jwt.claim_str("iss") != Some(issuer) {
        return Err("the token isn't from the space's authority".into());
    }
    let ours = eventside_did(&state.oauth.public_url);
    let audience = jwt.claim_str("aud").unwrap_or_default();
    if audience != ours && audience != format!("{ours}#eventside_access") {
        return Err("the token isn't for eventside".into());
    }
    if jwt.claim_str("lxm") != Some(lxm) {
        return Err(format!("the token isn't for {lxm}"));
    }
    if jwt.expired(now_secs()) {
        return Err("the token has expired".into());
    }
    // A cached key that doesn't verify may have been rotated: fetch it again, once.
    let key = signing_key(state, issuer, false).await?;
    if jwt.verify(&key).is_ok() {
        return Ok(());
    }
    let key = signing_key(state, issuer, true).await?;
    jwt.verify(&key)
}

/// How long a DID's signing key is reused.
const KEY_TTL_MS: i64 = 5 * 60 * 1000;

/// DIDs' `#atproto` keys, with when they were fetched.
static SIGNING_KEYS: std::sync::Mutex<Option<std::collections::HashMap<String, (PublicKey, i64)>>> =
    std::sync::Mutex::new(None);

/// A DID's `#atproto` signing key, from its document: cached for a few
/// minutes, unless `fresh`.
async fn signing_key(state: &AppState, did: &str, fresh: bool) -> Result<PublicKey, String> {
    let now = crate::db::now_ms();
    if !fresh
        && let Some((key, at)) = SIGNING_KEYS
            .lock()
            .expect("the key cache isn't poisoned")
            .as_ref()
            .and_then(|keys| keys.get(did).cloned())
        && now - at < KEY_TTL_MS
    {
        return Ok(key);
    }
    let doc = state.resolver.did_document(did).await.map_err(|e| format!("{e:?}"))?;
    let multibase = doc
        .get("verificationMethod")
        .and_then(Value::as_array)
        .and_then(|methods| {
            methods.iter().find(|m| {
                m.get("id")
                    .and_then(Value::as_str)
                    .is_some_and(|id| id == "#atproto" || id == format!("{did}#atproto"))
            })
        })
        .and_then(|m| m.get("publicKeyMultibase"))
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{did} lists no #atproto key"))?;
    let key = PublicKey::from_did_key(multibase)?;
    let mut cache = SIGNING_KEYS.lock().expect("the key cache isn't poisoned");
    let cache = cache.get_or_insert_with(std::collections::HashMap::new);
    if cache.len() > 10_000 {
        cache.clear();
    }
    cache.insert(did.to_owned(), (key.clone(), now));
    Ok(key)
}

/// `/.well-known/did.json`: eventside's `did:web` document, with the
/// `#eventside_access` service a conference space's policy names, and every
/// `#eventside_attest*` key that has signed records.
pub async fn did_document(State(state): State<AppState>) -> Response {
    let did = eventside_did(&state.oauth.public_url);
    let keys = match attest::keys(&state.db).await {
        Ok(keys) => keys,
        Err(why) => return server_error(&why),
    };
    let methods: Vec<Value> = keys
        .iter()
        .map(|(fragment, key)| {
            json!({
                "id": format!("{did}#{fragment}"),
                "type": "Multikey",
                "controller": did,
                "publicKeyMultibase": key.multibase(),
            })
        })
        .collect();
    Json(json!({
        "@context": ["https://www.w3.org/ns/did/v1", "https://w3id.org/security/multikey/v1"],
        "id": did,
        "verificationMethod": methods,
        "service": [{
            "id": "#eventside_access",
            "type": "EventsideAccess",
            "serviceEndpoint": state.oauth.public_url,
        }],
    }))
    .into_response()
}
