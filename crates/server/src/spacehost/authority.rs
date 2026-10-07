//! Space authorities: minting an organization's `did:plc`, and the custody of
//! its keys. The genesis operation names us as the `#atproto_space_host` and
//! publishes the key we sign credentials with as `#atproto_space` (and as
//! `#atproto`, so service auth from the DID is accepted anywhere). The
//! operator's recovery key comes first among its rotation keys, ahead of
//! ours, so a breach of our server can be undone within PLC's recovery window.

use serde_json::{Value, json};

use crate::AppState;
use crate::crypto::{self, Cbor};
use crate::db::{Db, now_ms};
use crate::keys::{EcKey, random_token};

/// The server's secrets: the key the authorities' keys are encrypted under,
/// and the HMAC key for emails and invite codes.
#[derive(Clone)]
pub struct Secrets {
    seal: [u8; 32],
    hmac: [u8; 32],
}

impl Secrets {
    /// From `AUTHORITY_KEY_SECRET`, or a secret generated once and kept in the
    /// database (dev and tests), as the OAuth signing key is.
    pub async fn load(db: &Db, configured: Option<&str>) -> Result<Self, String> {
        let secret = match configured {
            Some(secret) => secret.to_owned(),
            None => {
                sqlx::query(
                    "INSERT INTO server_secrets (name, value, created_at) VALUES ('authority_key_secret', $1, $2) \
                     ON CONFLICT (name) DO NOTHING",
                )
                .bind(random_token(32))
                .bind(now_ms())
                .execute(db)
                .await
                .map_err(|e| format!("could not store the authority key secret: {e}"))?;
                sqlx::query_scalar::<_, String>(
                    "SELECT value FROM server_secrets WHERE name = 'authority_key_secret'",
                )
                .fetch_one(db)
                .await
                .map_err(|e| format!("could not read the authority key secret: {e}"))?
            }
        };
        Ok(Self {
            seal: crypto::derive_key(&secret, "eventside authority keys"),
            hmac: crypto::derive_key(&secret, "eventside hmac"),
        })
    }

    pub fn seal(&self, plaintext: &str) -> String {
        crypto::seal(&self.seal, plaintext)
    }

    pub fn open(&self, sealed: &str) -> Result<String, String> {
        crypto::open(&self.seal, sealed)
    }

    /// An email, lowercased, HMAC'd: how attendee lists keep them.
    pub fn email_hmac(&self, email: &str) -> String {
        crypto::hmac_b64(&self.hmac, &format!("email:{}", email.trim().to_lowercase()))
    }

    /// An invite code, HMAC'd: how code records keep them.
    pub fn code_hmac(&self, code: &str) -> String {
        crypto::hmac_b64(&self.hmac, &format!("code:{}", code.trim()))
    }
}

/// An organization we hold the keys for.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct Authority {
    pub did: String,
    pub super_admin: String,
    pub name: Option<String>,
    pub created_at: i64,
}

pub async fn get(db: &Db, did: &str) -> Result<Option<Authority>, String> {
    sqlx::query_as::<_, Authority>(
        "SELECT did, super_admin, name, created_at FROM authorities WHERE did = $1",
    )
    .bind(did)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("could not load the organization {did}: {e}"))
}

pub async fn all(db: &Db) -> Result<Vec<Authority>, String> {
    sqlx::query_as::<_, Authority>(
        "SELECT did, super_admin, name, created_at FROM authorities ORDER BY created_at",
    )
    .fetch_all(db)
    .await
    .map_err(|e| format!("could not list organizations: {e}"))
}

/// The key an authority signs credentials and service auth with.
pub async fn space_key(state: &AppState, did: &str) -> Result<EcKey, String> {
    let sealed =
        sqlx::query_scalar::<_, String>("SELECT space_key FROM authorities WHERE did = $1")
            .bind(did)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| format!("could not load {did}'s key: {e}"))?
            .ok_or_else(|| format!("we hold no keys for {did}"))?;
    EcKey::from_jwk(&state.secrets.open(&sealed)?)
}

/// Mints an organization's `did:plc` on the PLC directory: rotation keys
/// (the operator's recovery key, then ours), our space key, and us as its
/// space host. Stores its keys and super admin, and returns the DID.
pub async fn mint(
    state: &AppState,
    super_admin: &str,
    recovery_key: &str,
    name: Option<&str>,
) -> Result<String, String> {
    crypto::PublicKey::from_did_key(recovery_key)
        .map_err(|e| format!("the recovery key isn't a did:key: {e}"))?;
    let rotation = EcKey::generate();
    let space = EcKey::generate();
    let (did, op) = genesis(&rotation, &space, recovery_key, &state.oauth.public_url);
    let url = format!("{}/{did}", state.config.plc_url);
    let res = state
        .http
        .trusted
        .post(&url)
        .json(&op)
        .send()
        .await
        .map_err(|e| format!("the PLC directory couldn't be reached: {e}"))?;
    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        return Err(format!("the PLC directory refused the new DID ({status}): {body}"));
    }
    sqlx::query(
        "INSERT INTO authorities (did, super_admin, name, rotation_key, space_key, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(&did)
    .bind(super_admin)
    .bind(name)
    .bind(state.secrets.seal(&rotation.private_jwk()))
    .bind(state.secrets.seal(&space.private_jwk()))
    .bind(now_ms())
    .execute(&state.db)
    .await
    .map_err(|e| format!("could not store the organization's keys: {e}"))?;
    Ok(did)
}

/// A signed genesis operation and the DID it makes.
fn genesis(rotation: &EcKey, space: &EcKey, recovery_key: &str, host: &str) -> (String, Value) {
    let unsigned = |sig: Option<String>| {
        let mut entries = vec![
            ("type".to_owned(), Cbor::Str("plc_operation".into())),
            (
                "rotationKeys".to_owned(),
                Cbor::Array(vec![
                    Cbor::Str(recovery_key.to_owned()),
                    Cbor::Str(rotation.did_key()),
                ]),
            ),
            (
                "verificationMethods".to_owned(),
                Cbor::Map(vec![
                    ("atproto".to_owned(), Cbor::Str(space.did_key())),
                    ("atproto_space".to_owned(), Cbor::Str(space.did_key())),
                ]),
            ),
            ("alsoKnownAs".to_owned(), Cbor::Array(vec![])),
            (
                "services".to_owned(),
                Cbor::Map(vec![(
                    "atproto_space_host".to_owned(),
                    Cbor::Map(vec![
                        ("type".to_owned(), Cbor::Str("AtprotoSpaceHost".into())),
                        ("endpoint".to_owned(), Cbor::Str(host.to_owned())),
                    ]),
                )]),
            ),
            ("prev".to_owned(), Cbor::Null),
        ];
        if let Some(sig) = sig {
            entries.push(("sig".to_owned(), Cbor::Str(sig)));
        }
        Cbor::Map(entries)
    };
    let sig = crate::keys::b64(rotation.sign_bytes(&unsigned(None).encode()));
    let signed = unsigned(Some(sig));
    let hash = crypto::sha256(&signed.encode());
    let did = format!("did:plc:{}", &crypto::base32_lower(&hash)[..24]);
    (did, signed.to_json())
}

/// A service-auth JWT from an authority, signed with its space key.
pub fn service_jwt(key: &EcKey, iss: &str, aud: &str, lxm: &str) -> String {
    let now = crate::keys::now_secs();
    key.sign(
        json!({ "typ": "JWT", "alg": "ES256", "kid": "#atproto_space" }),
        &json!({ "iss": iss, "aud": aud, "lxm": lxm, "iat": now, "exp": now + 60, "jti": random_token(16) }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_genesis_operation_names_its_did_and_keys() {
        let (rotation, space) = (EcKey::generate(), EcKey::generate());
        let recovery = EcKey::generate().did_key();
        let (did, op) = genesis(&rotation, &space, &recovery, "https://eventside.example");
        assert!(did.starts_with("did:plc:") && did.len() == 32, "{did}");
        assert!(crate::identity::is_valid_did(&did));
        assert_eq!(op["rotationKeys"][0], recovery.as_str());
        assert_eq!(op["services"]["atproto_space_host"]["endpoint"], "https://eventside.example");
        assert_eq!(op["verificationMethods"]["atproto_space"], space.did_key().as_str());
        assert!(op["prev"].is_null());
        // Signed by our rotation key over the unsigned operation.
        let mut unsigned = op.clone();
        unsigned.as_object_mut().unwrap().remove("sig");
        let sig = base64::Engine::decode(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD,
            op["sig"].as_str().unwrap(),
        )
        .unwrap();
        assert_eq!(sig.len(), 64);
    }
}
