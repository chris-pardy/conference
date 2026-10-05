//! Identity: handle to DID, DID to document, and the PDS and handle in it.

use serde::Deserialize;
use serde_json::Value;

use crate::net::Http;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityError {
    /// The handle doesn't resolve to a DID.
    HandleNotFound,
    /// The DID document couldn't be fetched, or names no PDS.
    Unresolvable(String),
}

#[derive(Debug, Clone)]
pub struct Identity {
    pub did: String,
    /// The handle, checked in both directions, or `handle.invalid`.
    pub handle: String,
    pub pds: String,
}

#[derive(Clone)]
pub struct Resolver {
    pub http: Http,
    pub plc_url: String,
    pub handle_resolver_url: String,
}

impl Resolver {
    pub async fn resolve_handle(&self, handle: &str) -> Result<String, IdentityError> {
        let handle = normalize_handle(handle).ok_or(IdentityError::HandleNotFound)?;
        let url = format!("{}/xrpc/com.atproto.identity.resolveHandle", self.handle_resolver_url);
        let res = self
            .http
            .trusted
            .get(&url)
            .query(&[("handle", &handle)])
            .send()
            .await
            .map_err(|e| IdentityError::Unresolvable(format!("resolveHandle failed: {e}")))?;
        // Only "no such handle" is final; rate limits and the like aren't.
        if matches!(res.status().as_u16(), 400 | 404) {
            return Err(IdentityError::HandleNotFound);
        }
        if !res.status().is_success() {
            return Err(IdentityError::Unresolvable(format!(
                "resolveHandle answered {}",
                res.status()
            )));
        }
        #[derive(Deserialize)]
        struct Resolved {
            did: String,
        }
        let resolved: Resolved =
            crate::net::read_json(res).await.map_err(IdentityError::Unresolvable)?;
        if !is_valid_did(&resolved.did) {
            return Err(IdentityError::HandleNotFound);
        }
        Ok(resolved.did)
    }

    /// The DID's document, its PDS, and its handle if the handle points back.
    pub async fn resolve_did(&self, did: &str) -> Result<Identity, IdentityError> {
        if !is_valid_did(did) {
            return Err(IdentityError::Unresolvable(format!("{did:?} is not a supported DID")));
        }
        let doc = self.did_document(did).await?;
        if doc.get("id").and_then(Value::as_str) != Some(did) {
            return Err(IdentityError::Unresolvable(format!(
                "the document for {did} is for someone else"
            )));
        }
        let pds = pds_endpoint(&doc, did)
            .ok_or_else(|| IdentityError::Unresolvable(format!("{did} names no PDS")))?;
        let claimed = claimed_handle(&doc);
        // `handle.invalid` only when the handle definitely doesn't point back;
        // a resolver that can't answer right now fails the whole resolution.
        let handle = match claimed {
            Some(h) => match self.resolve_handle(&h).await {
                Ok(back) if back == did => h,
                Ok(_) | Err(IdentityError::HandleNotFound) => "handle.invalid".to_owned(),
                Err(err) => return Err(err),
            },
            None => "handle.invalid".to_owned(),
        };
        Ok(Identity { did: did.to_owned(), handle, pds })
    }

    async fn did_document(&self, did: &str) -> Result<Value, IdentityError> {
        let unresolvable = |why: String| IdentityError::Unresolvable(format!("{did}: {why}"));
        let res = if did.starts_with("did:plc:") {
            self.http.trusted.get(format!("{}/{did}", self.plc_url)).send().await
        } else if let Some(host) = did.strip_prefix("did:web:") {
            if host.contains(':') || host.is_empty() {
                return Err(unresolvable("only host-level did:web is supported".into()));
            }
            let url = format!("https://{host}/.well-known/did.json");
            self.http.guarded(&url).map_err(unresolvable)?.get(&url).send().await
        } else {
            return Err(unresolvable("unsupported DID method".into()));
        };
        let res = res.map_err(|e| unresolvable(e.to_string()))?;
        if !res.status().is_success() {
            return Err(unresolvable(format!("document answered {}", res.status())));
        }
        crate::net::read_json(res).await.map_err(unresolvable)
    }
}

/// A handle as typed, lowercased and without a leading `@`; `None` when it
/// can't be a handle.
pub fn normalize_handle(input: &str) -> Option<String> {
    let handle = input.trim().trim_start_matches('@').to_ascii_lowercase();
    let valid = handle.len() <= 253
        && handle.contains('.')
        && handle.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        });
    valid.then_some(handle)
}

/// Whether a DID is one the appview resolves: `did:plc:` with its 24 base32
/// characters, or a host-level `did:web:`. Checked before a DID goes into a URL.
pub fn is_valid_did(did: &str) -> bool {
    if let Some(id) = did.strip_prefix("did:plc:") {
        return id.len() == 24 && id.bytes().all(|b| matches!(b, b'a'..=b'z' | b'2'..=b'7'));
    }
    if let Some(host) = did.strip_prefix("did:web:") {
        return normalize_handle(host).as_deref() == Some(host);
    }
    false
}

fn pds_endpoint(doc: &Value, did: &str) -> Option<String> {
    doc.get("service")?.as_array()?.iter().find_map(|service| {
        let id = service.get("id")?.as_str()?;
        let is_pds = (id == "#atproto_pds" || id == format!("{did}#atproto_pds"))
            && service.get("type")?.as_str()? == "AtprotoPersonalDataServer";
        let endpoint = service.get("serviceEndpoint")?.as_str()?;
        is_pds.then(|| endpoint.trim_end_matches('/').to_owned())
    })
}

/// The handle the document claims, normalized as `resolve_handle` checks it,
/// so the handle stored is the one that was checked; `None` when its first
/// `at://` entry can't be a handle.
fn claimed_handle(doc: &Value) -> Option<String> {
    let aka = doc
        .get("alsoKnownAs")?
        .as_array()?
        .iter()
        .find_map(|aka| aka.as_str()?.strip_prefix("at://"))?;
    normalize_handle(aka)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn handles_are_normalized() {
        assert_eq!(normalize_handle(" @Ana.Example.com "), Some("ana.example.com".into()));
        assert_eq!(normalize_handle("ana"), None);
        assert_eq!(normalize_handle("ana..example"), None);
        assert_eq!(normalize_handle("ana/../x.com"), None);
    }

    #[test]
    fn only_well_formed_dids_are_resolved() {
        assert!(is_valid_did("did:plc:abcdefghijklmnopqrstuvwx"));
        assert!(is_valid_did("did:web:pds.example.com"));
        assert!(!is_valid_did("did:plc:x/../../export"));
        assert!(!is_valid_did("did:plc:abcdefghijklmnopqrstuvw1"));
        assert!(!is_valid_did("did:plc:abcdefghijklmnopqrstuvwx?a=b"));
        assert!(!is_valid_did("did:web:example.com:8080"));
        assert!(!is_valid_did("did:key:z6Mk"));
    }

    #[test]
    fn documents_name_their_pds_and_handle() {
        let doc = json!({
            "id": "did:plc:abc",
            "alsoKnownAs": ["at://ana.example.com"],
            "service": [{ "id": "#atproto_pds", "type": "AtprotoPersonalDataServer", "serviceEndpoint": "https://pds.example/" }]
        });
        assert_eq!(pds_endpoint(&doc, "did:plc:abc"), Some("https://pds.example".into()));
        let theirs = json!({
            "service": [{ "id": "did:plc:other#atproto_pds", "type": "AtprotoPersonalDataServer", "serviceEndpoint": "https://x.example" }]
        });
        assert_eq!(pds_endpoint(&theirs, "did:plc:abc"), None);
        let untyped = json!({ "service": [{ "id": "#atproto_pds", "type": "Other", "serviceEndpoint": "https://x.example" }] });
        assert_eq!(pds_endpoint(&untyped, "did:plc:abc"), None);
        assert_eq!(claimed_handle(&doc), Some("ana.example.com".into()));
    }

    #[test]
    fn a_claimed_handle_is_normalized_like_the_one_checked() {
        let claims = |aka: &str| claimed_handle(&json!({ "alsoKnownAs": [aka] }));
        assert_eq!(claims("at://@Ana.Test"), Some("ana.test".into()));
        assert_eq!(claims("at://ana.test/app.bsky"), None);
        assert_eq!(claims("at://ana"), None);
        assert_eq!(claims("https://ana.test"), None);
    }
}
