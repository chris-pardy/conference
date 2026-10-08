//! Signed decisions (design review round 5). Our host checks every
//! permission record, conference role, rules and sidecar record, and intake
//! join or leave when it's taken, then signs it with the authority's
//! `#eventside_attest*` key. Readers count only what verifies.
//!
//! The signature is a badge.blue inline attestation: the record carries a
//! `signatures` array, each entry
//! `{$type: "app.eventside.attest.signature", key, space, seq, role,
//! signedAt, signature}`. What's signed is the record without `signatures`,
//! plus `$sig` (the entry without `signature`, plus `repository`, the DID of
//! the repo that holds the record), as canonical DAG-CBOR, by its CIDv1
//! (dag-cbor, sha2-256): low-S P-256 over the 36 CID bytes. So a record
//! copied into another repo, or another space, doesn't verify.
//!
//! Signing is check-then-sign: in one transaction (`BEGIN IMMEDIATE`, since
//! the admin CLI is another process), the action is checked against the
//! index, the authority's next `seq` is taken, and a pending entry goes into
//! the signing journal. The record is then written without the lock, and
//! the entry committed, or voided if the write failed.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Mutex;

use serde_json::{Value, json};

use super::index::{self, Org, Role};
use crate::AppState;
use crate::crypto::{self, PublicKey};
use crate::db::{Backend, Db, now_ms};
use crate::keys::EcKey;

/// A `signatures` entry's `$type`.
pub const SIGNATURE: &str = "app.eventside.attest.signature";
/// Every attestation key's fragment starts with this.
pub const KEY_PREFIX: &str = "eventside_attest";
/// How far ahead of a reader's clock a `signedAt` may be.
pub const FUTURE_US: u64 = 5 * 60 * 1_000_000;
/// How long a pending journal entry blocks others. One older than this is
/// from a signing that never finished (a crash), and counts for nothing.
const PENDING_MS: i64 = 2 * 60_000;
/// How long a finished journal entry that isn't a code's use is kept.
const PRUNE_MS: i64 = 24 * 60 * 60_000;

/// The rank a signature records: the role of whoever acted when our host
/// signed, or `self` for a person's own join or leave.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rank {
    Person,
    Staff,
    Owner,
    SuperAdmin,
}

impl Rank {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Person => "self",
            Self::Staff => "staff",
            Self::Owner => "owner",
            Self::SuperAdmin => "superAdmin",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "self" => Some(Self::Person),
            "staff" => Some(Self::Staff),
            "owner" => Some(Self::Owner),
            "superAdmin" => Some(Self::SuperAdmin),
            _ => None,
        }
    }

    /// An admin's rank in an organization now, if they're one.
    pub fn of_admin(org: &Org, did: &str) -> Option<Self> {
        if did == org.super_admin {
            return Some(Self::SuperAdmin);
        }
        match org.admin_role(did)? {
            Role::Owner => Some(Self::Owner),
            Role::Staff => Some(Self::Staff),
        }
    }
}

/// What a verified signature says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sig {
    pub seq: u64,
    pub rank: Rank,
    pub signed_us: u64,
}

/// An authority's attestation keys, as its DID document lists them now.
#[derive(Debug, Clone, Default)]
pub struct Keys {
    pub authority: String,
    keys: BTreeMap<String, PublicKey>,
    fingerprint: [u8; 32],
}

impl Keys {
    /// From `(fragment, did:key)` pairs; ones that don't parse are left out.
    pub fn new(authority: &str, keys: &[(String, String)]) -> Self {
        let mut seed = authority.to_owned();
        let mut parsed = BTreeMap::new();
        for (fragment, key) in keys {
            if let Ok(public) = PublicKey::from_did_key(key) {
                seed.push_str(&format!("\n{fragment} {key}"));
                parsed.insert(fragment.clone(), public);
            }
        }
        Self {
            authority: authority.to_owned(),
            keys: parsed,
            fingerprint: crypto::sha256(seed.as_bytes()),
        }
    }

    pub fn fragments(&self) -> impl Iterator<Item = &String> {
        self.keys.keys()
    }
}

/// An authority's current attestation keys, from our copy of what its DID
/// document publishes (we write every operation that changes them).
pub async fn keys(db: &Db, authority: &str) -> Result<Keys, String> {
    let rows = sqlx::query_as::<_, (String, String)>(
        "SELECT fragment, public_key FROM attest_keys WHERE authority = $1 AND removed_at IS NULL",
    )
    .bind(authority)
    .fetch_all(db)
    .await
    .map_err(|e| format!("could not read {authority}'s attestation keys: {e}"))?;
    Ok(Keys::new(authority, &rows))
}

/// The key new records are signed with: the authority's newest.
pub async fn signing_key(state: &AppState, authority: &str) -> Result<(String, EcKey), String> {
    let row = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT fragment, private_key FROM attest_keys WHERE authority = $1 AND removed_at IS NULL \
         ORDER BY created_at DESC, fragment DESC LIMIT 1",
    )
    .bind(authority)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("could not read {authority}'s attestation key: {e}"))?;
    let Some((fragment, Some(sealed))) = row else {
        return Err(format!("we hold no attestation key for {authority}"));
    };
    Ok((format!("{authority}#{fragment}"), EcKey::from_jwk(&state.secrets.open(&sealed)?)?))
}

/// A `signatures` entry before it's signed: what `$sig` is made from.
fn entry(key_id: &str, space: &str, seq: u64, rank: Rank, signed_at: &str) -> Value {
    json!({
        "$type": SIGNATURE,
        "key": key_id,
        "space": space,
        "seq": seq,
        "role": rank.as_str(),
        "signedAt": signed_at,
    })
}

/// The 36-byte CIDv1 a `signatures` entry signs, for a record read from
/// `repository`.
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

/// Adds a signature to a record, by `key` (named `key_id`), for the record
/// as it will be held in `repository`, in `space`.
#[allow(clippy::too_many_arguments)]
pub fn add_signature(
    record: &mut Value,
    key_id: &str,
    key: &EcKey,
    space: &str,
    seq: u64,
    rank: Rank,
    signed_at: &str,
    repository: &str,
) -> Result<(), String> {
    let mut entry = entry(key_id, space, seq, rank, signed_at);
    let cid = attestation_cid(record, &entry, repository)?;
    entry["signature"] = json!({ "$bytes": crypto::encode_b64(&key.sign_bytes(&cid)) });
    let map = record.as_object_mut().ok_or("a record is an object")?;
    match map.get_mut("signatures") {
        Some(Value::Array(entries)) => entries.push(entry),
        _ => {
            map.insert("signatures".into(), json!([entry]));
        }
    }
    Ok(())
}

/// Checks one `signatures` entry against the keys, for a record read from
/// `repository` in `space`. Not the clock: see [`verify`].
fn check_entry(
    record: &Value,
    entry: &Value,
    repository: &str,
    space: &str,
    keys: &Keys,
) -> Option<Sig> {
    let (did, fragment) = entry.get("key")?.as_str()?.split_once('#')?;
    if did != keys.authority || !fragment.starts_with(KEY_PREFIX) {
        return None;
    }
    let public = keys.keys.get(fragment)?;
    if entry.get("space")?.as_str()? != space {
        return None;
    }
    let seq = entry.get("seq")?.as_u64()?;
    let rank = Rank::parse(entry.get("role")?.as_str()?)?;
    let signed_us = index::parse_iso_us(entry.get("signedAt")?.as_str()?)?;
    let bytes = crypto::decode_b64(entry.get("signature")?.get("$bytes")?.as_str()?).ok()?;
    let signature = p256::ecdsa::Signature::from_slice(&bytes).ok()?;
    // Low S only, as atproto's verifiers insist.
    if signature.normalize_s().is_some() {
        return None;
    }
    let cid = attestation_cid(record, entry, repository).ok()?;
    public.verify(&cid, &bytes).then_some(Sig { seq, rank, signed_us })
}

/// The `seq` a record claims for `authority` in `space`, whether or not its
/// signature verifies: the first `signatures` entry by one of the
/// authority's keys, for that space.
pub fn claimed_seq(record: &Value, authority: &str, space: &str) -> Option<u64> {
    record.get("signatures")?.as_array()?.iter().find_map(|entry| {
        let (did, _) = entry.get("key")?.as_str()?.split_once('#')?;
        (did == authority && entry.get("space")?.as_str()? == space)
            .then(|| entry.get("seq")?.as_u64())
            .flatten()
    })
}

/// What verified, per record, by everything that decides it.
static VERIFIED: Mutex<Option<HashMap<[u8; 32], Option<Sig>>>> = Mutex::new(None);
const VERIFIED_MAX: usize = 100_000;

/// The signature a record counts by: the first of its `signatures` that
/// verifies against a key the authority's DID document lists now, for the
/// repo it was read from and the space it's in, and isn't dated more than
/// [`FUTURE_US`] after `now_us`. `None` when nothing does.
pub fn verify(
    record: &Value,
    repository: &str,
    space: &str,
    keys: &Keys,
    now_us: u64,
) -> Option<Sig> {
    let entries = record.get("signatures")?.as_array()?;
    if entries.is_empty() {
        return None;
    }
    let mut seed = keys.fingerprint.to_vec();
    for part in [repository, space, &record.to_string()] {
        seed.extend_from_slice(part.as_bytes());
        seed.push(0);
    }
    let id = crypto::sha256(&seed);
    let cached = VERIFIED
        .lock()
        .expect("the signature cache isn't poisoned")
        .as_ref()
        .and_then(|c| c.get(&id).copied());
    let sig = match cached {
        Some(sig) => sig,
        None => {
            let sig = entries.iter().find_map(|e| check_entry(record, e, repository, space, keys));
            let mut cache = VERIFIED.lock().expect("the signature cache isn't poisoned");
            let cache = cache.get_or_insert_with(HashMap::new);
            if cache.len() >= VERIFIED_MAX {
                cache.clear();
            }
            cache.insert(id, sig);
            sig
        }
    };
    sig.filter(|s| s.signed_us <= now_us.saturating_add(FUTURE_US))
}

/// A record re-signed with another key: a copy with one more `signatures`
/// entry, by `key_id`, with the same `seq`, rank and time as the entry it
/// verifies by now. `None` when nothing on it verifies now, or `key_id`'s
/// signature already does.
#[allow(clippy::too_many_arguments)]
pub fn resign(
    record: &Value,
    repository: &str,
    space: &str,
    keys: &Keys,
    key_id: &str,
    key: &EcKey,
    now_us: u64,
) -> Result<Option<Value>, String> {
    if verify(record, repository, space, keys, now_us).is_none() {
        return Ok(None);
    }
    let entries = record.get("signatures").and_then(Value::as_array).cloned().unwrap_or_default();
    let valid: Vec<&Value> = entries
        .iter()
        .filter(|e| check_entry(record, e, repository, space, keys).is_some())
        .collect();
    if valid.iter().any(|e| e.get("key").and_then(Value::as_str) == Some(key_id)) {
        return Ok(None);
    }
    let Some(first) = valid.first() else { return Ok(None) };
    let field =
        |name: &str| first.get(name).cloned().ok_or_else(|| format!("a signature has no {name}"));
    let seq = field("seq")?.as_u64().ok_or("a signature's seq isn't a number")?;
    let rank = Rank::parse(field("role")?.as_str().unwrap_or_default())
        .ok_or("a signature's role is unknown")?;
    let signed_at = field("signedAt")?;
    let mut copy = record.clone();
    add_signature(
        &mut copy,
        key_id,
        key,
        space,
        seq,
        rank,
        signed_at.as_str().unwrap_or_default(),
        repository,
    )?;
    Ok(Some(copy))
}

/// Who a signing is for: an admin acting (at their rank now), or a person
/// writing their own join or leave.
#[derive(Debug, Clone, Copy)]
pub enum Signer<'a> {
    Admin(&'a str),
    Person(&'a str),
}

/// What a signing decides about, so a pending one blocks a conflicting one:
/// a person in a conference, and the code a join uses.
#[derive(Debug, Clone, Default)]
pub struct Claim {
    pub conference: Option<String>,
    pub subject: Option<String>,
    pub code_hash: Option<String>,
}

impl Claim {
    pub fn about(conference: &str, subject: &str) -> Self {
        Self {
            conference: Some(conference.to_owned()),
            subject: Some(subject.to_owned()),
            code_hash: None,
        }
    }
}

/// What the journal knows that the records may not show yet.
#[derive(Debug, Clone, Default)]
pub struct Journal {
    /// Who has used the claimed code in a signing that's pending or
    /// committed, so a code's limits hold while records are being written.
    pub code_users: BTreeSet<String>,
}

/// A check run inside the signing transaction, against the index then.
pub type Check<'a> = Box<dyn FnOnce(&Org, &Journal) -> Result<(), String> + Send + 'a>;

/// A reserved signing: its `seq`, rank and time, and the key to sign with.
pub struct Ticket {
    pub authority: String,
    pub seq: u64,
    pub rank: Rank,
    pub signed_at: String,
    key_id: String,
    key: EcKey,
}

impl Ticket {
    /// Signs a record (complete, `$type` and `createdAt` included) for
    /// `space`, as it will be held in `repository`.
    pub fn sign(&self, record: &mut Value, space: &str, repository: &str) -> Result<(), String> {
        add_signature(
            record,
            &self.key_id,
            &self.key,
            space,
            self.seq,
            self.rank,
            &self.signed_at,
            repository,
        )
    }
}

/// Checks an action and reserves its signing: in one transaction, runs
/// `check` against the index, takes the authority's next `seq`, and logs a
/// pending journal entry. Refuses when another signing about the same
/// person is pending.
pub async fn reserve(
    state: &AppState,
    authority: &str,
    signer: Signer<'_>,
    claim: &Claim,
    check: Option<Check<'_>>,
) -> Result<Ticket, String> {
    let (key_id, key) = signing_key(state, authority).await?;
    let backend = Backend::of(&state.config.database_url)?;
    let mut conn = state.db.acquire().await.map_err(|e| e.to_string())?;
    let begin = match backend {
        Backend::Sqlite => "BEGIN IMMEDIATE",
        Backend::Postgres => "BEGIN",
    };
    sqlx::query(begin)
        .execute(&mut *conn)
        .await
        .map_err(|e| format!("could not start signing: {e}"))?;
    let reserved = reserve_in(state, &mut conn, backend, authority, signer, claim, check).await;
    let end = if reserved.is_ok() { "COMMIT" } else { "ROLLBACK" };
    let ended = sqlx::query(end).execute(&mut *conn).await;
    let (seq, rank, signed_ms) = reserved?;
    ended.map_err(|e| format!("could not record the signing: {e}"))?;
    Ok(Ticket {
        authority: authority.to_owned(),
        seq,
        rank,
        signed_at: index::iso(signed_ms as u64 * 1000),
        key_id,
        key,
    })
}

async fn reserve_in(
    state: &AppState,
    conn: &mut sqlx::AnyConnection,
    backend: Backend,
    authority: &str,
    signer: Signer<'_>,
    claim: &Claim,
    check: Option<Check<'_>>,
) -> Result<(u64, Rank, i64), String> {
    let db = |e: sqlx::Error| format!("could not sign: {e}");
    sqlx::query(
        "INSERT INTO signing_counters (authority, next_seq, last_signed_at) VALUES ($1, 1, 0) \
         ON CONFLICT (authority) DO NOTHING",
    )
    .bind(authority)
    .execute(&mut *conn)
    .await
    .map_err(db)?;
    let lock = if backend == Backend::Postgres { " FOR UPDATE" } else { "" };
    let (next_seq, last_signed) = sqlx::query_as::<_, (i64, i64)>(&format!(
        "SELECT next_seq, last_signed_at FROM signing_counters WHERE authority = $1{lock}"
    ))
    .bind(authority)
    .fetch_one(&mut *conn)
    .await
    .map_err(db)?;
    let now = now_ms();
    let live = now - PENDING_MS;
    // Finished entries matter only for a code's uses; the rest are pruned.
    sqlx::query(
        "DELETE FROM signing_journal WHERE authority = $1 AND code_hash IS NULL \
         AND state <> 'pending' AND created_at < $2",
    )
    .bind(authority)
    .bind(now - PRUNE_MS)
    .execute(&mut *conn)
    .await
    .map_err(db)?;
    if let (Some(conference), Some(subject)) = (&claim.conference, &claim.subject) {
        let pending = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM signing_journal WHERE authority = $1 AND state = 'pending' \
             AND created_at > $2 AND space = $3 AND subject = $4",
        )
        .bind(authority)
        .bind(live)
        .bind(conference)
        .bind(subject)
        .fetch_one(&mut *conn)
        .await
        .map_err(db)?;
        if pending > 0 {
            return Err(format!(
                "another decision about {subject} is being written right now; nothing was \
                 changed, so try again in a moment"
            ));
        }
    }
    let mut journal = Journal::default();
    if let Some(code_hash) = &claim.code_hash {
        journal.code_users = sqlx::query_scalar::<_, String>(
            "SELECT DISTINCT subject FROM signing_journal WHERE authority = $1 AND code_hash = $2 \
             AND subject IS NOT NULL AND (state = 'committed' OR (state = 'pending' AND created_at > $3))",
        )
        .bind(authority)
        .bind(code_hash)
        .bind(live)
        .fetch_all(&mut *conn)
        .await
        .map_err(db)?
        .into_iter()
        .collect();
    }
    let org = index::load(state, authority)
        .await?
        .ok_or_else(|| format!("{authority} isn't an organization we host"))?;
    let rank = match signer {
        Signer::Person(_) => Rank::Person,
        Signer::Admin(did) => Rank::of_admin(&org, did).ok_or_else(|| {
            format!("{did} isn't an admin of {authority} now, so nothing is signed for them")
        })?,
    };
    if let Some(check) = check {
        check(&org, &journal)?;
    }
    let seq = (next_seq.max(1) as u64).max(org.max_seq + 1);
    let signed_ms = now.max(last_signed + 1);
    sqlx::query(
        "UPDATE signing_counters SET next_seq = $2, last_signed_at = $3 WHERE authority = $1",
    )
    .bind(authority)
    .bind((seq + 1) as i64)
    .bind(signed_ms)
    .execute(&mut *conn)
    .await
    .map_err(db)?;
    sqlx::query(
        "INSERT INTO signing_journal (authority, seq, space, subject, code_hash, state, created_at) \
         VALUES ($1, $2, $3, $4, $5, 'pending', $6)",
    )
    .bind(authority)
    .bind(seq as i64)
    .bind(&claim.conference)
    .bind(&claim.subject)
    .bind(&claim.code_hash)
    .bind(now)
    .execute(&mut *conn)
    .await
    .map_err(db)?;
    Ok((seq, rank, signed_ms))
}

/// Marks a reserved signing committed (its record was written) or void.
pub async fn finish(state: &AppState, ticket: &Ticket, written: bool) {
    let outcome = if written { "committed" } else { "void" };
    let done =
        sqlx::query("UPDATE signing_journal SET state = $3 WHERE authority = $1 AND seq = $2")
            .bind(&ticket.authority)
            .bind(ticket.seq as i64)
            .bind(outcome)
            .execute(&state.db)
            .await;
    if let Err(why) = done {
        eprintln!(
            "warning: couldn't mark signing {} of {} {outcome}: {why}",
            ticket.seq, ticket.authority
        );
    }
}

/// Whether records of a collection in a space are signed: everything in an
/// admin space; roles, rules and the sidecar in a conference space; joins
/// and leaves in an intake space.
pub fn signed_in(space: &str, collection: &str) -> bool {
    let Some(parsed) = super::SpaceUri::parse(space) else { return false };
    match parsed.kind.as_str() {
        super::ADMIN_TYPE => true,
        super::CONFERENCE_TYPE => {
            matches!(collection, index::ROLE | index::RULES | crate::conference::SIDECAR)
        }
        super::INTAKE_TYPE => matches!(collection, index::JOIN | index::LEAVE),
        _ => false,
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// A key and the `Keys` that list it, for an authority.
    pub fn test_keys(authority: &str) -> (EcKey, Keys) {
        let key = EcKey::generate();
        let keys = Keys::new(authority, &[(KEY_PREFIX.to_owned(), key.did_key())]);
        (key, keys)
    }

    /// Signs a record as our host would.
    #[allow(clippy::too_many_arguments)]
    pub fn signed(
        mut record: Value,
        key: &EcKey,
        authority: &str,
        space: &str,
        seq: u64,
        rank: Rank,
        us: u64,
        repo: &str,
    ) -> Value {
        add_signature(
            &mut record,
            &format!("{authority}#{KEY_PREFIX}"),
            key,
            space,
            seq,
            rank,
            &index::iso(us),
            repo,
        )
        .unwrap();
        record
    }

    const ORG: &str = "did:plc:atmosphereorgaaaaaaaaaaa";
    const OLGA: &str = "did:plc:olgaaaaaaaaaaaaaaaaaaaaa";
    const MALLORY: &str = "did:plc:malloryaaaaaaaaaaaaaaaaa";

    #[test]
    fn tc_61_a_signature_verifies_only_where_it_was_signed_for() {
        let (key, keys) = test_keys(ORG);
        let space = format!("at://{ORG}/space/app.eventside.admin/self");
        let record = json!({ "$type": "app.eventside.admin.member", "subject": MALLORY, "createdAt": "2026-10-07T12:00:00.000Z" });
        let now = 1_800_000_000_000_000;
        let record = signed(record, &key, ORG, &space, 7, Rank::Owner, now, OLGA);
        assert_eq!(
            verify(&record, OLGA, &space, &keys, now),
            Some(Sig { seq: 7, rank: Rank::Owner, signed_us: now })
        );
        // Copied into another repo, or another space, or edited, it doesn't.
        assert_eq!(verify(&record, MALLORY, &space, &keys, now), None);
        assert_eq!(
            verify(&record, OLGA, &format!("at://{ORG}/space/app.eventside.intake/x"), &keys, now),
            None
        );
        let mut edited = record.clone();
        edited["subject"] = json!(OLGA);
        assert_eq!(verify(&edited, OLGA, &space, &keys, now), None);
        // Nor against a key the DID document no longer lists.
        let (_, other) = test_keys(ORG);
        assert_eq!(verify(&record, OLGA, &space, &other, now), None);
        // Nor when dated more than five minutes ahead.
        assert_eq!(verify(&record, OLGA, &space, &keys, now - FUTURE_US - 1), None);
        // The signature is low-S, 64 bytes, as `$bytes`.
        let bytes =
            crypto::decode_b64(record["signatures"][0]["signature"]["$bytes"].as_str().unwrap())
                .unwrap();
        assert_eq!(bytes.len(), 64);
    }

    #[test]
    fn tc_63_a_second_signature_by_a_new_key_keeps_the_record_counting() {
        let (old, _) = test_keys(ORG);
        let new = EcKey::generate();
        let space = format!("at://{ORG}/space/app.eventside.admin/self");
        let now = 1_800_000_000_000_000;
        let mut record = signed(
            json!({ "subject": MALLORY }),
            &old,
            ORG,
            &space,
            3,
            Rank::SuperAdmin,
            now,
            OLGA,
        );
        add_signature(
            &mut record,
            &format!("{ORG}#{KEY_PREFIX}_2"),
            &new,
            &space,
            3,
            Rank::SuperAdmin,
            &index::iso(now),
            OLGA,
        )
        .unwrap();
        let only_new = Keys::new(ORG, &[(format!("{KEY_PREFIX}_2"), new.did_key())]);
        assert_eq!(verify(&record, OLGA, &space, &only_new, now).map(|s| s.seq), Some(3));
    }
}
