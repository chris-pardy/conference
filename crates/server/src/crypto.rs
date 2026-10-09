//! The cryptography conferences need beyond OAuth: did:keys and multikeys
//! (P-256 and secp256k1), verifying ES256 and ES256K JWTs (the service auth
//! a PDS sends eventside), TIDs, and the canonical DAG-CBOR and CIDs that
//! signed records are built from.

use std::sync::atomic::{AtomicU64, Ordering};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand_core::{OsRng, RngCore};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// A public key named by a did:key or a DID document's `Multikey`.
#[derive(Clone, Debug)]
pub enum PublicKey {
    P256(p256::ecdsa::VerifyingKey),
    K256(k256::ecdsa::VerifyingKey),
}

const P256_PREFIX: [u8; 2] = [0x80, 0x24];
const K256_PREFIX: [u8; 2] = [0xe7, 0x01];

impl PublicKey {
    /// Parses `did:key:z…`, or the bare multibase `z…` a `Multikey` publishes.
    pub fn from_did_key(key: &str) -> Result<Self, String> {
        let multibase = key.strip_prefix("did:key:").unwrap_or(key);
        let encoded = multibase
            .strip_prefix('z')
            .ok_or_else(|| format!("{key} isn't base58btc multibase"))?;
        let bytes =
            bs58::decode(encoded).into_vec().map_err(|e| format!("{key} isn't base58: {e}"))?;
        if bytes.len() < 2 {
            return Err(format!("{key} is too short"));
        }
        let (prefix, point) = bytes.split_at(2);
        if prefix == P256_PREFIX {
            p256::ecdsa::VerifyingKey::from_sec1_bytes(point)
                .map(Self::P256)
                .map_err(|e| format!("{key} isn't a P-256 key: {e}"))
        } else if prefix == K256_PREFIX {
            k256::ecdsa::VerifyingKey::from_sec1_bytes(point)
                .map(Self::K256)
                .map_err(|e| format!("{key} isn't a secp256k1 key: {e}"))
        } else {
            Err(format!("{key} is neither a P-256 nor a secp256k1 key"))
        }
    }

    /// The multibase (`z…`) for a P-256 key, as a `Multikey` publishes it.
    pub fn p256_multibase(key: &p256::PublicKey) -> String {
        use p256::elliptic_curve::sec1::ToEncodedPoint;
        let point = key.to_encoded_point(true);
        let mut bytes = P256_PREFIX.to_vec();
        bytes.extend_from_slice(point.as_bytes());
        format!("z{}", bs58::encode(bytes).into_string())
    }

    pub fn jwt_alg(&self) -> &'static str {
        match self {
            Self::P256(_) => "ES256",
            Self::K256(_) => "ES256K",
        }
    }

    /// Checks a 64-byte `r||s` signature over `message` (hashed with SHA-256).
    /// High-S signatures are accepted, as the spaces proposal says.
    pub fn verify(&self, message: &[u8], signature: &[u8]) -> bool {
        use p256::ecdsa::signature::Verifier;
        match self {
            Self::P256(key) => p256::ecdsa::Signature::from_slice(signature).is_ok_and(|sig| {
                let sig = sig.normalize_s().unwrap_or(sig);
                key.verify(message, &sig).is_ok()
            }),
            Self::K256(key) => k256::ecdsa::Signature::from_slice(signature).is_ok_and(|sig| {
                let sig = sig.normalize_s().unwrap_or(sig);
                key.verify(message, &sig).is_ok()
            }),
        }
    }
}

/// A compact JWS, decoded but not yet checked.
#[derive(Debug, Clone)]
pub struct Jwt {
    pub header: Value,
    pub payload: Value,
    signing_input: String,
    signature: Vec<u8>,
}

impl Jwt {
    pub fn decode(token: &str) -> Result<Self, String> {
        let mut parts = token.split('.');
        let (Some(h), Some(p), Some(s), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err("malformed token: expected 3 parts".into());
        };
        let part = |b64: &str, what: &str| -> Result<Value, String> {
            let bytes = URL_SAFE_NO_PAD.decode(b64).map_err(|_| format!("bad token {what}"))?;
            serde_json::from_slice(&bytes).map_err(|_| format!("bad token {what}"))
        };
        let header = part(h, "header")?;
        let payload = part(p, "payload")?;
        if !header.is_object() || !payload.is_object() {
            return Err("malformed token".into());
        }
        let signature = URL_SAFE_NO_PAD.decode(s).map_err(|_| "bad token signature".to_owned())?;
        Ok(Self { header, payload, signing_input: format!("{h}.{p}"), signature })
    }

    pub fn header_str(&self, name: &str) -> Option<&str> {
        self.header.get(name).and_then(Value::as_str)
    }

    pub fn claim_str(&self, name: &str) -> Option<&str> {
        self.payload.get(name).and_then(Value::as_str)
    }

    pub fn claim_i64(&self, name: &str) -> Option<i64> {
        self.payload.get(name).and_then(Value::as_i64)
    }

    /// Checks the signature with `key`, whose algorithm must be the header's.
    pub fn verify(&self, key: &PublicKey) -> Result<(), String> {
        if self.header_str("alg") != Some(key.jwt_alg()) {
            return Err(format!("the token isn't signed with {}", key.jwt_alg()));
        }
        if key.verify(self.signing_input.as_bytes(), &self.signature) {
            Ok(())
        } else {
            Err("invalid token signature".into())
        }
    }

    /// Whether `exp` has passed, with a few seconds' leeway.
    pub fn expired(&self, now_secs: i64) -> bool {
        self.claim_i64("exp").is_none_or(|exp| now_secs - 5 >= exp)
    }
}

const TID_CHARS: &[u8] = b"234567abcdefghijklmnopqrstuvwxyz";

/// A TID for a time in microseconds since the epoch.
pub fn tid_from_micros(micros: u64, clock: u64) -> String {
    let mut value = ((micros & ((1 << 53) - 1)) << 10) | (clock & 0x3ff);
    let mut out = [b'2'; 13];
    for slot in out.iter_mut().rev() {
        *slot = TID_CHARS[(value & 31) as usize];
        value >>= 5;
    }
    String::from_utf8(out.to_vec()).expect("TID characters are ASCII")
}

/// The microseconds a TID (a repo revision, or a record key) was made at.
pub fn tid_micros(tid: &str) -> Option<u64> {
    if tid.len() != 13 {
        return None;
    }
    let mut value: u64 = 0;
    for c in tid.bytes() {
        let digit = TID_CHARS.iter().position(|&t| t == c)? as u64;
        value = value.checked_mul(32)?.checked_add(digit)?;
    }
    Some(value >> 10)
}

/// The milliseconds a TID was made at.
pub fn tid_ms(tid: &str) -> Option<i64> {
    tid_micros(tid).map(|us| (us / 1000) as i64)
}

static LAST_TID: AtomicU64 = AtomicU64::new(0);

/// A TID for now, never repeating within this process.
pub fn tid_now() -> String {
    let now = crate::db::now_ms() as u64 * 1000;
    let mut last = LAST_TID.load(Ordering::Relaxed);
    loop {
        let next = now.max(last + 1);
        match LAST_TID.compare_exchange(last, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return tid_from_micros(next, OsRng.next_u64() & 0x3ff),
            Err(seen) => last = seen,
        }
    }
}

/// The TID after `tid`: the same time, one clock step later.
pub fn tid_after(tid: &str) -> String {
    let micros = tid_micros(tid).unwrap_or(0);
    let now = crate::db::now_ms() as u64 * 1000;
    tid_from_micros(now.max(micros + 1), 0)
}

/// RFC 4648 base32, lowercase, unpadded.
pub fn base32_lower(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz234567";
    let mut out = String::new();
    let mut buffer: u32 = 0;
    let mut bits = 0;
    for &byte in bytes {
        buffer = (buffer << 8) | u32::from(byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPHABET[((buffer >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(ALPHABET[((buffer << (5 - bits)) & 31) as usize] as char);
    }
    out
}

/// SHA-256.
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// Canonical DAG-CBOR for a value in atproto's JSON form: `{"$bytes": …}`
/// is a byte string and `{"$link": …}` a CID (tag 42). Map keys sort by
/// length, then bytewise; integers take their shortest form; floats aren't
/// allowed, as in atproto's data model.
pub fn dag_cbor(value: &Value) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    write_json(value, &mut out)?;
    Ok(out)
}

fn write_json(value: &Value, out: &mut Vec<u8>) -> Result<(), String> {
    match value {
        Value::Null => out.push(0xf6),
        Value::Bool(b) => out.push(if *b { 0xf5 } else { 0xf4 }),
        Value::Number(n) => {
            if let Some(u) = n.as_u64() {
                head64(out, 0, u);
            } else if let Some(i) = n.as_i64() {
                head64(out, 1, (-1 - i) as u64);
            } else {
                return Err(format!("DAG-CBOR in atproto has no floats: {n}"));
            }
        }
        Value::String(s) => {
            head64(out, 3, s.len() as u64);
            out.extend_from_slice(s.as_bytes());
        }
        Value::Array(items) => {
            head64(out, 4, items.len() as u64);
            for item in items {
                write_json(item, out)?;
            }
        }
        Value::Object(map) => {
            if map.len() == 1 {
                if let Some(Value::String(b64)) = map.get("$bytes") {
                    let bytes = decode_b64(b64)?;
                    head64(out, 2, bytes.len() as u64);
                    out.extend_from_slice(&bytes);
                    return Ok(());
                }
                if let Some(Value::String(link)) = map.get("$link") {
                    let mut cid = vec![0x00];
                    cid.extend(base32_lower_decode(
                        link.strip_prefix('b')
                            .ok_or_else(|| format!("not a base32 CID: {link}"))?,
                    )?);
                    out.extend([0xd8, 0x2a]);
                    head64(out, 2, cid.len() as u64);
                    out.extend_from_slice(&cid);
                    return Ok(());
                }
            }
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.as_bytes().cmp(b.as_bytes())));
            head64(out, 5, keys.len() as u64);
            for key in keys {
                head64(out, 3, key.len() as u64);
                out.extend_from_slice(key.as_bytes());
                write_json(&map[key], out)?;
            }
        }
    }
    Ok(())
}

fn head64(out: &mut Vec<u8>, major: u8, n: u64) {
    let major = major << 5;
    if n < 24 {
        out.push(major | n as u8);
    } else if n < 0x100 {
        out.extend([major | 24, n as u8]);
    } else if n < 0x10000 {
        out.push(major | 25);
        out.extend((n as u16).to_be_bytes());
    } else if n < 0x1_0000_0000 {
        out.push(major | 26);
        out.extend((n as u32).to_be_bytes());
    } else {
        out.push(major | 27);
        out.extend(n.to_be_bytes());
    }
}

/// Standard base64, padded or not, as atproto's `$bytes` may be.
pub fn decode_b64(text: &str) -> Result<Vec<u8>, String> {
    base64::engine::general_purpose::STANDARD_NO_PAD
        .decode(text.trim_end_matches('='))
        .map_err(|e| format!("not base64: {e}"))
}

/// Standard base64 with padding, as HTTP structured fields' byte sequences take it.
pub fn encode_b64_padded(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Standard base64 without padding: how atproto writes `$bytes`.
pub fn encode_b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD_NO_PAD.encode(bytes)
}

fn base32_lower_decode(text: &str) -> Result<Vec<u8>, String> {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz234567";
    let mut out = Vec::new();
    let mut buffer: u32 = 0;
    let mut bits = 0;
    for c in text.bytes() {
        let digit =
            ALPHABET.iter().position(|&a| a == c).ok_or_else(|| format!("not base32: {text}"))?;
        buffer = (buffer << 5) | digit as u32;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    Ok(out)
}

/// The 36-byte CIDv1 (dag-cbor, sha2-256) of some DAG-CBOR.
pub fn cid_bytes(dag_cbor: &[u8]) -> [u8; 36] {
    let mut cid = [0u8; 36];
    cid[..4].copy_from_slice(&[0x01, 0x71, 0x12, 0x20]);
    cid[4..].copy_from_slice(&sha256(dag_cbor));
    cid
}

/// A CID's string form: multibase base32, lowercase (`b…`).
pub fn cid_string(cid: &[u8]) -> String {
    format!("b{}", base32_lower(cid))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tids_round_trip_their_time() {
        let tid = tid_from_micros(1_700_000_000_000_000, 7);
        assert_eq!(tid.len(), 13);
        assert_eq!(tid_micros(&tid), Some(1_700_000_000_000_000));
        let (a, b) = (tid_now(), tid_now());
        assert!(b > a, "{a} then {b}");
        assert!(tid_after(&b) > b);
    }

    #[test]
    fn did_keys_round_trip() {
        let secret = p256::SecretKey::random(&mut OsRng);
        let did_key = format!("did:key:{}", PublicKey::p256_multibase(&secret.public_key()));
        assert!(did_key.starts_with("did:key:zDn"), "{did_key}");
        assert!(matches!(PublicKey::from_did_key(&did_key), Ok(PublicKey::P256(_))));
        // A secp256k1 key, as atproto accounts publish them.
        let k = k256::SecretKey::random(&mut OsRng);
        use k256::elliptic_curve::sec1::ToEncodedPoint;
        let mut bytes = K256_PREFIX.to_vec();
        bytes.extend_from_slice(k.public_key().to_encoded_point(true).as_bytes());
        let multibase = format!("z{}", bs58::encode(bytes).into_string());
        assert!(matches!(PublicKey::from_did_key(&multibase), Ok(PublicKey::K256(_))));
    }

    #[test]
    fn es256k_tokens_verify() {
        use k256::ecdsa::signature::Signer;
        let key = k256::ecdsa::SigningKey::random(&mut OsRng);
        let input = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(r#"{"alg":"ES256K"}"#),
            URL_SAFE_NO_PAD.encode(r#"{"iss":"did:plc:x"}"#)
        );
        let sig: k256::ecdsa::Signature = key.sign(input.as_bytes());
        let token = format!("{input}.{}", URL_SAFE_NO_PAD.encode(sig.to_bytes()));
        let jwt = Jwt::decode(&token).unwrap();
        assert!(jwt.verify(&PublicKey::K256(*key.verifying_key())).is_ok());
        let other = k256::ecdsa::SigningKey::random(&mut OsRng);
        assert!(jwt.verify(&PublicKey::K256(*other.verifying_key())).is_err());
    }

    #[test]
    fn base32_round_trips() {
        assert_eq!(base32_lower(b"foobar"), "mzxw6ytboi");
        assert_eq!(base32_lower_decode("mzxw6ytboi").unwrap(), b"foobar");
    }

    #[test]
    fn json_is_encoded_as_canonical_dag_cbor() {
        let value = serde_json::json!({
            "bb": null, "a": 1, "ab": [true, -2], "c": { "$bytes": "AQI" }, "zz": 300,
        });
        let map = [
            vec![0xa5, 0x61, b'a', 0x01, 0x61, b'c', 0x42, 0x01, 0x02],
            vec![0x62, b'a', b'b', 0x82, 0xf5, 0x21, 0x62, b'b', b'b', 0xf6],
            vec![0x62, b'z', b'z', 0x19, 0x01, 0x2c],
        ]
        .concat();
        assert_eq!(dag_cbor(&value).unwrap(), map);
        assert!(dag_cbor(&serde_json::json!({ "f": 1.5 })).is_err());
        let cid = cid_bytes(b"x");
        assert_eq!(&cid[..4], &[0x01, 0x71, 0x12, 0x20]);
        assert!(cid_string(&cid).starts_with("bafyrei"));
    }
}
