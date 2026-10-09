//! Signed records (design rounds 1 and 2): eventside signs the member, ban
//! and apps records it writes into a conference's space with an
//! `#eventside_attest*` key from its own DID document, so any app the
//! organizer allows can check who's a member and who organizes, and the
//! records can move unchanged into another repo later.
//!
//! The signature is a badge.blue inline attestation: the record carries a
//! `signatures` array, each entry `{$type, key, signedAt, signature}`.
//! What's signed is the record without `signatures`, plus `$sig` (the entry
//! without `signature`, plus `repository`, the DID of the repo that holds
//! the record), as canonical DAG-CBOR, by its CIDv1 (dag-cbor, sha2-256):
//! a low-S P-256 signature over the 36 CID bytes. So a record copied into
//! another repo doesn't verify. A reader checks the key against eventside's
//! DID document as it is now, and that eventside is the space's managing app.
//!
//! Keys are only added (`admin keys add`); the newest signs.

use serde_json::{Value, json};

use crate::crypto::{self, PublicKey};
use crate::db::{Db, now_ms};
use crate::keys::EcKey;

/// The `$type` of a `signatures` entry.
pub const SIGNATURE: &str = "app.eventside.conference.defs#signature";
/// The fragment every attestation key's starts with; the first key's is exactly this.
pub const KEY_PREFIX: &str = "eventside_attest";

/// Makes the first key, unless there is one. Every process can call it at
/// once: they all end up with the same key.
pub async fn ensure_key(db: &Db) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO attest_keys (fragment, private_jwk, created_at) VALUES ($1, $2, $3) \
         ON CONFLICT (fragment) DO NOTHING",
    )
    .bind(KEY_PREFIX)
    .bind(EcKey::generate().private_jwk())
    .bind(now_ms())
    .execute(db)
    .await
    .map(drop)
    .map_err(|e| format!("could not make eventside's signing key: {e}"))
}

/// Every attestation key, oldest first: `(fragment, key)`. Only reads:
/// the server and the CLI make the first key when they start ([`ensure_key`]).
pub async fn keys(db: &Db) -> Result<Vec<(String, EcKey)>, String> {
    let rows = sqlx::query_as::<_, (String, String, i64)>(
        "SELECT fragment, private_jwk, created_at FROM attest_keys",
    )
    .fetch_all(db)
    .await
    .map_err(|e| e.to_string())?;
    let mut keys = rows
        .into_iter()
        .map(|(fragment, jwk, created_at)| {
            let order = key_number(&fragment);
            EcKey::from_jwk(&jwk).map(|key| (order, created_at, fragment, key))
        })
        .collect::<Result<Vec<_>, _>>()?;
    keys.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
    Ok(keys.into_iter().map(|(_, _, fragment, key)| (fragment, key)).collect())
}

/// A key's place in line: `eventside_attest` is 1, `eventside_attest_2` is 2.
fn key_number(fragment: &str) -> u64 {
    fragment
        .strip_prefix(KEY_PREFIX)
        .and_then(|rest| rest.strip_prefix('_'))
        .and_then(|n| n.parse().ok())
        .unwrap_or(1)
}

/// The key that signs now: the newest.
pub async fn current(db: &Db) -> Result<(String, EcKey), String> {
    keys(db).await?.pop().ok_or_else(|| "eventside has no signing key".to_owned())
}

/// Adds another key, which signs from now on. Returns its fragment.
pub async fn add_key(db: &Db) -> Result<String, String> {
    for _ in 0..5 {
        let next = keys(db).await?.iter().map(|(f, _)| key_number(f)).max().unwrap_or(1) + 1;
        let fragment = format!("{KEY_PREFIX}_{next}");
        let added = sqlx::query(
            "INSERT INTO attest_keys (fragment, private_jwk, created_at) VALUES ($1, $2, $3) \
             ON CONFLICT (fragment) DO NOTHING",
        )
        .bind(&fragment)
        .bind(EcKey::generate().private_jwk())
        .bind(now_ms())
        .execute(db)
        .await
        .map_err(|e| e.to_string())?;
        if added.rows_affected() == 1 {
            return Ok(fragment);
        }
    }
    Err("couldn't add a signing key: another one was being added at the same time".into())
}

/// The 36-byte CIDv1 a `signatures` entry signs, for a record held in `repository`.
pub fn attestation_cid(
    record: &Value,
    entry: &Value,
    repository: &str,
) -> Result<[u8; 36], String> {
    let mut sig = entry.clone();
    let sig_map = sig.as_object_mut().ok_or("a signature entry is an object")?;
    sig_map.remove("signature");
    sig_map.insert("repository".into(), json!(repository));
    let mut unsigned = record.clone();
    let map = unsigned.as_object_mut().ok_or("a record is an object")?;
    map.remove("signatures");
    map.insert("$sig".into(), sig);
    Ok(crypto::cid_bytes(&crypto::dag_cbor(&unsigned)?))
}

/// Signs a record as it will be held in `repository`, replacing any
/// signatures it had: `key_id` is `{eventside did}#{fragment}`.
pub fn sign(record: &mut Value, key_id: &str, key: &EcKey, repository: &str) -> Result<(), String> {
    let map = record.as_object_mut().ok_or("a record is an object")?;
    map.remove("signatures");
    let mut entry = json!({
        "$type": SIGNATURE,
        "key": key_id,
        "signedAt": super::iso(now_ms()),
    });
    let cid = attestation_cid(record, &entry, repository)?;
    entry["signature"] = json!({ "$bytes": crypto::encode_b64(&key.sign_bytes(&cid)) });
    record["signatures"] = json!([entry]);
    Ok(())
}

/// The keys (`did#fragment`) whose signatures on a record verify, for the
/// record as read from `repository`, against `signer`'s keys as its DID
/// document lists them now (`fragment`, public key).
pub fn verify(
    record: &Value,
    repository: &str,
    signer: &str,
    keys: &[(String, PublicKey)],
) -> Vec<String> {
    let Some(entries) = record.get("signatures").and_then(Value::as_array) else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| {
            let key_id = entry.get("key")?.as_str()?;
            let (did, fragment) = key_id.split_once('#')?;
            if did != signer || !fragment.starts_with(KEY_PREFIX) {
                return None;
            }
            let (_, public) = keys.iter().find(|(f, _)| f == fragment)?;
            let bytes =
                crypto::decode_b64(entry.get("signature")?.get("$bytes")?.as_str()?).ok()?;
            let signature = p256::ecdsa::Signature::from_slice(&bytes).ok()?;
            // Low S only, as atproto's verifiers insist.
            if signature.normalize_s().is_some() {
                return None;
            }
            let cid = attestation_cid(record, entry, repository).ok()?;
            public.verify(&cid, &bytes).then(|| key_id.to_owned())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVENTSIDE: &str = "did:web:eventside.test";
    const ORG: &str = "did:plc:aaaaaaaaaaaaaaaaaaaaaaaa";

    fn member() -> Value {
        json!({
            "$type": "app.eventside.conference.member",
            "subject": "did:plc:bbbbbbbbbbbbbbbbbbbbbbbb",
            "role": "attendee",
            "seq": 3,
            "createdAt": "2027-04-29T09:00:00.000Z",
        })
    }

    #[test]
    fn tc_37_a_signed_record_verifies_against_the_signing_key() {
        let key = EcKey::generate();
        let mut record = member();
        sign(&mut record, &format!("{EVENTSIDE}#{KEY_PREFIX}"), &key, ORG).unwrap();
        let keys = [(KEY_PREFIX.to_owned(), key.verifying_key())];
        assert_eq!(
            verify(&record, ORG, EVENTSIDE, &keys),
            vec![format!("{EVENTSIDE}#{KEY_PREFIX}")]
        );
    }

    #[test]
    fn tc_38_an_altered_or_copied_record_does_not_verify() {
        let key = EcKey::generate();
        let mut record = member();
        sign(&mut record, &format!("{EVENTSIDE}#{KEY_PREFIX}"), &key, ORG).unwrap();
        let keys = [(KEY_PREFIX.to_owned(), key.verifying_key())];
        let mut altered = record.clone();
        altered["role"] = json!("owner");
        assert!(verify(&altered, ORG, EVENTSIDE, &keys).is_empty());
        // The same record, read from another repo.
        assert!(verify(&record, "did:plc:cccccccccccccccccccccccc", EVENTSIDE, &keys).is_empty());
        // Signed by another key than the one listed.
        let other = [(KEY_PREFIX.to_owned(), EcKey::generate().verifying_key())];
        assert!(verify(&record, ORG, EVENTSIDE, &other).is_empty());
        // Claiming another signer.
        assert!(verify(&record, ORG, "did:web:other.test", &keys).is_empty());
    }

    #[test]
    fn keys_are_numbered_in_order() {
        assert_eq!(key_number("eventside_attest"), 1);
        assert_eq!(key_number("eventside_attest_2"), 2);
        assert_eq!(key_number("eventside_attest_10"), 10);
    }
}
