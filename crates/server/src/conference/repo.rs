//! The organization's repo, written as the organization (the session its
//! account connected with, `admin org connect`), and reading a conference's
//! space with eventside's own credential.
//!
//! - **Writing:** the space itself (`simplespace.createSpace`), the public
//!   event and sidecar, and the records eventside keeps in the
//!   organization's repo in the space ([`put`] and [`delete`], the
//!   authority write API other features use too).
//! - **Reading:** eventside asks the organization's PDS for a delegation
//!   token, trades it at the space's host (also the organization's PDS) for
//!   a space credential bound to eventside's key, and reads each writer's
//!   ops from their own PDS with it, like any other app would.

use std::collections::HashMap;
use std::sync::Mutex;

use reqwest::Method;
use serde_json::{Value, json};

use crate::AppState;
use crate::auth::pds::{PdsClient, PdsError};
use crate::db::now_ms;
use crate::keys::EcKey;

/// The organization's live session, by its DID.
pub async fn org_session(state: &AppState, org: &str) -> Result<String, String> {
    sqlx::query_scalar::<_, String>(
        "SELECT id_hash FROM sessions WHERE did = $1 AND kind = 'org' AND ended_at IS NULL \
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(org)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| {
        format!("the organization {org} isn't connected. Run `conference-server admin org connect <its handle>`.")
    })
}

/// A client for the organization's PDS, as the organization.
pub async fn org_client(state: &AppState, org: &str) -> Result<PdsClient, String> {
    let id_hash = org_session(state, org).await?;
    PdsClient::for_session(state, &id_hash).await.map_err(|e| match e {
        PdsError::SessionExpired => format!(
            "the organization {org}'s session has ended. Run `conference-server admin org connect <its handle>` again."
        ),
        PdsError::Unavailable(why) => format!("the organization {org}'s PDS couldn't be reached: {why}"),
    })
}

/// An XRPC call at the organization's PDS, as the organization.
async fn call(
    client: &PdsClient,
    method: Method,
    nsid: &str,
    query: &[(&str, &str)],
    body: Option<&Value>,
) -> Result<Value, String> {
    let mut path = format!("/xrpc/{nsid}");
    if !query.is_empty() {
        let encoded: String = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(query.iter().copied())
            .finish();
        path = format!("{path}?{encoded}");
    }
    let res = client.send(method, &path, body).await.map_err(|e| match e {
        PdsError::SessionExpired => "the organization's session has ended".to_owned(),
        PdsError::Unavailable(why) => format!("the organization's PDS couldn't be reached: {why}"),
    })?;
    let status = res.status();
    let text = res.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("{nsid} answered {status}: {text}"));
    }
    Ok(serde_json::from_str(&text).unwrap_or(Value::Null))
}

/// Creates a conference's space on the organization's PDS, with eventside
/// as its managing app for reads and writes. Every app may ask, so the PDS
/// always asks eventside (`appAccess: #open`).
pub async fn create_space(state: &AppState, org: &str, skey: &str) -> Result<String, String> {
    let client = org_client(state, org).await?;
    let policy = json!({
        "$type": "com.atproto.simplespace.defs#managingAppPolicy",
        "managingApp": super::access_service(&state.oauth.public_url),
    });
    let body = json!({
        "spaceType": super::SPACE_TYPE,
        "skey": skey,
        "readPolicy": policy,
        "writePolicy": policy,
        "appAccess": { "$type": "com.atproto.simplespace.defs#open" },
    });
    let out = call(&client, Method::POST, "com.atproto.simplespace.createSpace", &[], Some(&body))
        .await?;
    out.get("uri")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("createSpace answered without the space's URI: {out}"))
}

/// Writes a public record in the organization's repo, returning its AT-URI.
pub async fn put_public(
    state: &AppState,
    org: &str,
    collection: &str,
    rkey: &str,
    record: Value,
) -> Result<String, String> {
    let client = org_client(state, org).await?;
    let body = json!({ "repo": org, "collection": collection, "rkey": rkey, "record": record });
    let out = call(&client, Method::POST, "com.atproto.repo.putRecord", &[], Some(&body)).await?;
    Ok(out
        .get("uri")
        .and_then(Value::as_str)
        .map_or_else(|| format!("at://{org}/{collection}/{rkey}"), str::to_owned))
}

/// Writes a record into the organization's repo in a conference's space.
pub async fn put(
    state: &AppState,
    space: &str,
    collection: &str,
    rkey: &str,
    record: Value,
) -> Result<(), String> {
    let org = authority(space)?;
    let client = org_client(state, &org).await?;
    let body = json!({
        "space": space, "repo": org, "collection": collection, "rkey": rkey, "record": record,
    });
    call(&client, Method::POST, "com.atproto.space.putRecord", &[], Some(&body)).await.map(drop)
}

/// Deletes a record from the organization's repo in a conference's space;
/// one that isn't there is already deleted.
pub async fn delete(
    state: &AppState,
    space: &str,
    collection: &str,
    rkey: &str,
) -> Result<(), String> {
    let org = authority(space)?;
    let client = org_client(state, &org).await?;
    let body = json!({ "space": space, "repo": org, "collection": collection, "rkey": rkey });
    call(&client, Method::POST, "com.atproto.space.deleteRecord", &[], Some(&body)).await.map(drop)
}

fn authority(space: &str) -> Result<String, String> {
    super::SpaceUri::parse(space)
        .map(|s| s.authority)
        .ok_or_else(|| format!("{space} isn't a space"))
}

/// The `atproto-space` HTTP message signature headers (RFC 9421): without
/// `audience` (trading a delegation token) the signature covers
/// `authorization` and names the key; with it (using a credential) it covers
/// `authorization` and `atproto-space-audience`.
pub fn space_signature(
    key: &EcKey,
    authorization: &str,
    audience: Option<&str>,
) -> Vec<(&'static str, String)> {
    let input = match audience {
        None => format!("(\"authorization\");keyid=\"{}\"", key.did_key()),
        Some(_) => "(\"authorization\" \"atproto-space-audience\")".to_owned(),
    };
    let mut lines = vec![format!("\"authorization\": {authorization}")];
    if let Some(audience) = audience {
        lines.push(format!("\"atproto-space-audience\": {audience}"));
    }
    lines.push(format!("\"@signature-params\": {input}"));
    let signature = crate::crypto::encode_b64_padded(&key.sign_bytes(lines.join("\n").as_bytes()));
    let mut headers = vec![("authorization", authorization.to_owned())];
    if let Some(audience) = audience {
        headers.push(("atproto-space-audience", audience.to_owned()));
    }
    headers.push(("signature-input", format!("atproto-space={input}")));
    headers.push(("signature", format!("atproto-space=:{signature}:")));
    headers
}

/// Eventside's own space credentials, by space, with when they run out.
static CREDENTIALS: Mutex<Option<HashMap<String, (String, i64)>>> = Mutex::new(None);

/// Credentials being fetched: one at a time.
static FETCHING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A credential for the space that's still good, if there is one.
fn cached_credential(space: &str) -> Option<String> {
    CREDENTIALS
        .lock()
        .expect("the credential cache isn't poisoned")
        .as_ref()
        .and_then(|c| c.get(space).cloned())
        .filter(|(_, until)| *until > now_ms())
        .map(|(credential, _)| credential)
}

/// How long eventside uses a credential: well within the 10 minutes a PDS
/// issues them for.
const CREDENTIAL_MS: i64 = 5 * 60 * 1000;

/// Eventside's credential for reading a conference's space, delegated by
/// the organization (the space's authority, always authorized).
pub async fn credential(state: &AppState, space: &str) -> Result<String, String> {
    if let Some(credential) = cached_credential(space) {
        return Ok(credential);
    }
    // One fetch at a time: syncs that start together share the first one's.
    let _one = FETCHING.lock().await;
    if let Some(credential) = cached_credential(space) {
        return Ok(credential);
    }
    let now = now_ms();
    let org = authority(space)?;
    let client = org_client(state, &org).await?;
    let delegation = call(
        &client,
        Method::GET,
        "com.atproto.space.getDelegationToken",
        &[("space", space)],
        None,
    )
    .await?;
    let token = delegation
        .get("token")
        .and_then(Value::as_str)
        .ok_or("getDelegationToken answered without a token")?;
    let host = pds_of(state, &org).await?;
    let url = format!("{host}/xrpc/com.atproto.space.getSpaceCredential");
    let mut req = state.http.guarded(&url)?.post(&url).json(&json!({ "space": space }));
    for (name, value) in space_signature(&state.oauth.key, &format!("Bearer {token}"), None) {
        req = req.header(name, value);
    }
    let res = req.send().await.map_err(|e| format!("{url}: {e}"))?;
    let status = res.status();
    let body: Value = res.json().await.unwrap_or(Value::Null);
    let credential = body
        .get("credential")
        .and_then(Value::as_str)
        .filter(|_| status.is_success())
        .ok_or_else(|| format!("getSpaceCredential answered {status}: {body}"))?
        .to_owned();
    CREDENTIALS
        .lock()
        .expect("the credential cache isn't poisoned")
        .get_or_insert_with(HashMap::new)
        .insert(space.to_owned(), (credential.clone(), now + CREDENTIAL_MS));
    Ok(credential)
}

/// A DID's PDS.
pub async fn pds_of(state: &AppState, did: &str) -> Result<String, String> {
    state
        .resolver
        .resolve_did(did)
        .await
        .map(|identity| identity.pds)
        .map_err(|e| format!("couldn't resolve {did}: {e:?}"))
}

/// One record version from a repo's ops in a space.
#[derive(Debug, Clone)]
pub struct Op {
    pub collection: String,
    pub rkey: String,
    /// The revision the writer's PDS gave the commit that wrote it.
    pub rev: String,
    /// The CID it wrote; `None` when it deleted the record.
    pub cid: Option<String>,
    /// The record as it is now, when this op wrote the current version.
    pub value: Option<Value>,
}

/// The most pages of ops read from one repo at once.
const MAX_PAGES: usize = 100;

/// A writer's ops in a space after revision `since` (all of them without
/// it), oldest first, read from their PDS with eventside's credential. No
/// repo in the space is no ops.
pub async fn repo_ops(
    state: &AppState,
    space: &str,
    repo: &str,
    since: Option<&str>,
) -> Result<Vec<Op>, String> {
    let credential = credential(state, space).await?;
    let pds = pds_of(state, repo).await?;
    let url = format!("{pds}/xrpc/com.atproto.space.listRepoOps");
    let client = state.http.guarded(&url)?;
    let authorization = format!("Atproto-Space {credential}");
    let mut cursor: Option<String> = None;
    let mut ops = Vec::new();
    for _ in 0..MAX_PAGES {
        let mut query = vec![
            ("space", space.to_owned()),
            ("repo", repo.to_owned()),
            ("limit", "500".to_owned()),
        ];
        if let Some(since) = since {
            query.push(("since", since.to_owned()));
        }
        if let Some(cursor) = &cursor {
            query.push(("cursor", cursor.clone()));
        }
        let mut req = client.get(&url).query(&query);
        for (name, value) in space_signature(&state.oauth.key, &authorization, Some(repo)) {
            req = req.header(name, value);
        }
        let res = req.send().await.map_err(|e| format!("{url}: {e}"))?;
        let status = res.status();
        let body: Value = res.json().await.unwrap_or(Value::Null);
        if !status.is_success() {
            if body.get("error").and_then(Value::as_str) == Some("RepoNotFound") {
                return Ok(ops);
            }
            if status.as_u16() == 401 || status.as_u16() == 403 {
                // A credential the space no longer honors: ask for another next time.
                if let Some(cache) =
                    CREDENTIALS.lock().expect("the credential cache isn't poisoned").as_mut()
                {
                    cache.remove(space);
                }
            }
            return Err(format!("listRepoOps of {repo} answered {status}: {body}"));
        }
        let page = body.get("ops").and_then(Value::as_array).cloned().unwrap_or_default();
        let empty = page.is_empty();
        for op in page {
            let (Some(collection), Some(rkey), Some(rev)) = (
                op.get("collection").and_then(Value::as_str),
                op.get("rkey").and_then(Value::as_str),
                op.get("rev").and_then(Value::as_str),
            ) else {
                continue;
            };
            ops.push(Op {
                collection: collection.to_owned(),
                rkey: rkey.to_owned(),
                rev: rev.to_owned(),
                cid: op.get("cid").and_then(Value::as_str).map(str::to_owned),
                value: op.get("value").cloned(),
            });
        }
        cursor = body.get("cursor").and_then(Value::as_str).map(str::to_owned);
        if cursor.is_none() || empty {
            break;
        }
    }
    Ok(ops)
}

/// A call to the space's host (the organization's PDS) with eventside's
/// credential, whose audience is the space's authority.
async fn host_call(
    state: &AppState,
    space: &str,
    nsid: &str,
    query: &[(&str, String)],
    body: Option<&Value>,
) -> Result<Value, String> {
    let org = authority(space)?;
    let credential = credential(state, space).await?;
    let host = pds_of(state, &org).await?;
    let url = format!("{host}/xrpc/{nsid}");
    let client = state.http.guarded(&url)?;
    let mut req = match body {
        Some(body) => client.post(&url).json(body),
        None => client.get(&url).query(query),
    };
    for (name, value) in
        space_signature(&state.oauth.key, &format!("Atproto-Space {credential}"), Some(&org))
    {
        req = req.header(name, value);
    }
    let res = req.send().await.map_err(|e| format!("{url}: {e}"))?;
    let status = res.status();
    let text = res.text().await.unwrap_or_default();
    if !status.is_success() {
        if (status.as_u16() == 401 || status.as_u16() == 403)
            && let Some(cache) =
                CREDENTIALS.lock().expect("the credential cache isn't poisoned").as_mut()
        {
            cache.remove(space);
        }
        return Err(format!("{nsid} answered {status}: {text}"));
    }
    Ok(serde_json::from_str(&text).unwrap_or(Value::Null))
}

/// Every writer the space's host lists (`listRepos`).
pub async fn writers(state: &AppState, space: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..MAX_PAGES {
        let mut query = vec![("space", space.to_owned()), ("limit", "1000".to_owned())];
        if let Some(cursor) = &cursor {
            query.push(("cursor", cursor.clone()));
        }
        let body = host_call(state, space, "com.atproto.space.listRepos", &query, None).await?;
        let repos = body.get("repos").and_then(Value::as_array).cloned().unwrap_or_default();
        out.extend(
            repos.iter().filter_map(|r| r.get("did").and_then(Value::as_str).map(str::to_owned)),
        );
        cursor = body.get("cursor").and_then(|c| {
            c.as_str().map(str::to_owned).or_else(|| c.as_i64().map(|n| n.to_string()))
        });
        if cursor.is_none() || repos.is_empty() {
            break;
        }
    }
    Ok(out)
}

/// Registers eventside for the space's write notifications, at its
/// `#eventside_access` service.
pub async fn register_notify(state: &AppState, space: &str) -> Result<(), String> {
    let body = json!({ "space": space, "service": super::access_service(&state.oauth.public_url) });
    host_call(state, space, "com.atproto.space.registerNotify", &[], Some(&body)).await.map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_space_signature_covers_the_authorization_and_audience() {
        let key = EcKey::generate();
        let headers =
            space_signature(&key, "Atproto-Space x", Some("did:plc:aaaaaaaaaaaaaaaaaaaaaaaa"));
        let input = headers.iter().find(|(n, _)| *n == "signature-input").unwrap();
        assert_eq!(input.1, "atproto-space=(\"authorization\" \"atproto-space-audience\")");
        let headers = space_signature(&key, "Bearer y", None);
        let input = headers.iter().find(|(n, _)| *n == "signature-input").unwrap();
        assert!(input.1.contains("keyid=\"did:key:zDn"), "{}", input.1);
    }
}
