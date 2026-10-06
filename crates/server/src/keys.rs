//! ES256 keys, and the compact JWS tokens signed with them: the client's
//! assertions and the per-session DPoP proofs.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use p256::SecretKey;
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use rand_core::{OsRng, RngCore};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub fn b64(bytes: impl AsRef<[u8]>) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

/// `n` random bytes, base64url-encoded.
pub fn random_token(n: usize) -> String {
    let mut bytes = vec![0u8; n];
    OsRng.fill_bytes(&mut bytes);
    b64(bytes)
}

/// SHA-256, base64url-encoded: PKCE challenges, `ath`, and stored secrets.
pub fn sha256_b64(input: &str) -> String {
    b64(Sha256::digest(input.as_bytes()))
}

/// A P-256 key that signs ES256 JWTs.
#[derive(Clone)]
pub struct EcKey {
    secret: SecretKey,
    pub kid: Option<String>,
}

impl EcKey {
    pub fn generate() -> Self {
        Self { secret: SecretKey::random(&mut OsRng), kid: None }
    }

    /// Reads a private JWK, keeping its `kid` if it has one.
    pub fn from_jwk(jwk: &str) -> Result<Self, String> {
        let secret =
            SecretKey::from_jwk_str(jwk).map_err(|e| format!("not a P-256 private JWK: {e}"))?;
        let kid = serde_json::from_str::<Value>(jwk)
            .ok()
            .and_then(|v| v.get("kid").and_then(Value::as_str).map(str::to_owned));
        Ok(Self { secret, kid })
    }

    pub fn with_kid(mut self, kid: impl Into<String>) -> Self {
        self.kid = Some(kid.into());
        self
    }

    /// The private JWK, for storage.
    pub fn private_jwk(&self) -> String {
        self.secret.to_jwk_string().to_string()
    }

    /// The public JWK: `kty`, `crv`, `x` and `y`, plus `kid` when set.
    pub fn public_jwk(&self) -> Value {
        let mut jwk: Value = serde_json::from_str(&self.secret.public_key().to_jwk_string())
            .expect("a public JWK is valid JSON");
        if let Some(kid) = &self.kid {
            jwk["kid"] = json!(kid);
        }
        jwk
    }

    /// A compact JWS of `claims` with this key.
    pub fn sign(&self, header: Value, claims: &Value) -> String {
        let input = format!("{}.{}", b64(header.to_string()), b64(claims.to_string()));
        let signature: Signature = SigningKey::from(&self.secret).sign(input.as_bytes());
        format!("{input}.{}", b64(signature.to_bytes()))
    }
}

pub fn now_secs() -> i64 {
    crate::db::now_ms() / 1000
}

/// A `private_key_jwt` client assertion for `audience` (the authorization server's issuer).
pub fn client_assertion(key: &EcKey, client_id: &str, audience: &str) -> String {
    let now = now_secs();
    let mut header = json!({ "alg": "ES256", "typ": "JWT" });
    if let Some(kid) = &key.kid {
        header["kid"] = json!(kid);
    }
    key.sign(
        header,
        &json!({
            "iss": client_id,
            "sub": client_id,
            "aud": audience,
            "jti": random_token(16),
            "iat": now,
            "exp": now + 60,
        }),
    )
}

/// A DPoP proof for one request, bound to an access token when there is one.
pub fn dpop_proof(
    key: &EcKey,
    method: &str,
    url: &str,
    nonce: Option<&str>,
    access_token: Option<&str>,
) -> String {
    // The htu claim is the URL without its query or fragment.
    let htu = url.split(['?', '#']).next().unwrap_or(url);
    let mut claims =
        json!({ "jti": random_token(16), "htm": method, "htu": htu, "iat": now_secs() });
    if let Some(nonce) = nonce {
        claims["nonce"] = json!(nonce);
    }
    if let Some(token) = access_token {
        claims["ath"] = json!(sha256_b64(token));
    }
    let public = key.public_jwk();
    let jwk =
        json!({ "kty": public["kty"], "crv": public["crv"], "x": public["x"], "y": public["y"] });
    key.sign(json!({ "typ": "dpop+jwt", "alg": "ES256", "jwk": jwk }), &claims)
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::ecdsa::VerifyingKey;
    use p256::ecdsa::signature::Verifier;

    fn decode(part: &str) -> Value {
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(part).unwrap()).unwrap()
    }

    #[test]
    fn a_signed_token_verifies_with_the_public_key() {
        let key = EcKey::generate().with_kid("k1");
        let token = key.sign(json!({ "alg": "ES256" }), &json!({ "hello": "world" }));
        let parts: Vec<&str> = token.split('.').collect();
        assert_eq!(parts.len(), 3);
        let signature = Signature::from_slice(&URL_SAFE_NO_PAD.decode(parts[2]).unwrap()).unwrap();
        let verifying = VerifyingKey::from(&key.secret.public_key());
        verifying.verify(format!("{}.{}", parts[0], parts[1]).as_bytes(), &signature).unwrap();
        assert_eq!(decode(parts[1])["hello"], "world");
    }

    #[test]
    fn keys_survive_storage_and_publish_only_public_parts() {
        let key = EcKey::generate();
        let again = EcKey::from_jwk(&key.private_jwk()).unwrap().with_kid("k1");
        let public = again.public_jwk();
        assert_eq!(public["kty"], "EC");
        assert_eq!(public["crv"], "P-256");
        assert_eq!(public["kid"], "k1");
        assert!(public.get("d").is_none());
        assert_eq!(public["x"], key.public_jwk()["x"]);
    }

    #[test]
    fn dpop_proofs_carry_the_request_and_token() {
        let key = EcKey::generate();
        let proof =
            dpop_proof(&key, "POST", "https://pds.example/xrpc/x?y=1", Some("n1"), Some("tok"));
        let parts: Vec<&str> = proof.split('.').collect();
        let header = decode(parts[0]);
        assert_eq!(header["typ"], "dpop+jwt");
        assert!(header["jwk"].get("d").is_none());
        let claims = decode(parts[1]);
        assert_eq!(claims["htu"], "https://pds.example/xrpc/x");
        assert_eq!(claims["htm"], "POST");
        assert_eq!(claims["nonce"], "n1");
        assert_eq!(claims["ath"], sha256_b64("tok"));
    }
}
