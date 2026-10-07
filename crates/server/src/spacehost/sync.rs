//! Reading repos into the index: a writer's ops in one of our spaces, from
//! its own PDS (`com.atproto.space.listRepoOps`), with a credential we mint
//! for ourselves, so the appview reads a space like any other app.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde_json::Value;

use super::credential::{for_self, signed_headers};
use super::index::{self, Org, Rec};
use super::{ADMIN_TYPE, INTAKE_TYPE, SpaceUri};
use crate::AppState;
use crate::crypto::tid_micros;
use crate::db::now_ms;

/// The most of a page of ops read at once.
const PAGE: usize = 100;
/// The most bytes a page of ops may be.
const PAGE_CAP: usize = 4 * 1024 * 1024;

/// The most bytes a record in an intake space may be. A join or leave is a
/// few fields; anything bigger isn't one.
const INTAKE_RECORD_CAP: usize = 4 * 1024;
/// The most records one repo may have in an intake space. Anyone can write
/// there, so this bounds what one writer can make us keep and re-derive.
const INTAKE_RECORDS_PER_REPO: i64 = 1000;
/// The most bytes of ops read from one repo in an intake space at once.
/// A read stops at this (or at [`INTAKE_RECORDS_PER_REPO`] ops) and the next
/// goes on from there, so a writer filling one with junk costs a bounded
/// read per notification.
const INTAKE_READ_CAP: usize = 4 * 1024 * 1024;

/// Whether a record version in a space goes into the index. Intake spaces
/// take writes from anyone: only joins and leaves of a sensible size are
/// kept there. Anything else is still read (up to the intake read caps),
/// but isn't stored, and a read with nothing stored doesn't change the
/// organization's index.
fn indexed(space: &str, collection: &str, version: &Version) -> bool {
    if SpaceUri::parse(space).is_none_or(|s| s.kind != INTAKE_TYPE) {
        return true;
    }
    matches!(collection, index::JOIN | index::LEAVE)
        && version.value.as_ref().is_none_or(|v| v.len() <= INTAKE_RECORD_CAP)
}

/// A record's state after an op: its revision, CID and value (none when
/// deleted).
#[derive(Debug, Clone)]
struct Version {
    rev: String,
    cid: Option<String>,
    value: Option<String>,
}

/// What an op says about a record, if it says anything: `None` for a write
/// whose value a later op carries.
fn version(op: &Value) -> Option<(String, String, Version)> {
    let (Some(collection), Some(rkey), Some(rev)) = (
        op.get("collection").and_then(Value::as_str),
        op.get("rkey").and_then(Value::as_str),
        op.get("rev").and_then(Value::as_str),
    ) else {
        return None;
    };
    let cid = op.get("cid").and_then(Value::as_str);
    let value = match (cid, op.get("value")) {
        // Written, and still current: its value is inlined.
        (Some(_), Some(value)) => Some(value.to_string()),
        // Written, but changed since: a later op carries what it is now.
        (Some(_), None) => return None,
        // Deleted.
        (None, _) => None,
    };
    Some((
        collection.to_owned(),
        rkey.to_owned(),
        Version { rev: rev.to_owned(), cid: cid.map(str::to_owned), value },
    ))
}

/// A repo's ops in a space after `since` (all of them without it), from its
/// PDS, and how far they go. No repo in the space yet is no ops.
///
/// With `capped`, a read of an intake repo stops after
/// [`INTAKE_RECORDS_PER_REPO`] ops or [`INTAKE_READ_CAP`] bytes, so a write
/// notification costs a bounded read; reindex reads everything.
async fn fetch_ops(
    state: &AppState,
    space: &str,
    repo: &str,
    since: Option<&str>,
    capped: bool,
) -> Result<Read, String> {
    let parsed = SpaceUri::parse(space).ok_or_else(|| format!("{space} isn't a space"))?;
    let pds = state.host.pds_of(state, repo).await?;
    let credential = for_self(state, &parsed).await?;
    let url = format!("{pds}/xrpc/com.atproto.space.listRepoOps");
    let client = state.http.guarded(&url)?;
    let authorization = format!("Atproto-Space {credential}");
    let intake = capped && parsed.kind == INTAKE_TYPE;
    let mut cursor: Option<String> = None;
    let mut all = Vec::new();
    let mut bytes = 0;
    for _ in 0..1000 {
        let mut query = vec![
            ("space", space.to_owned()),
            ("repo", repo.to_owned()),
            ("limit", PAGE.to_string()),
        ];
        if let Some(since) = since {
            query.push(("since", since.to_owned()));
        }
        if let Some(cursor) = &cursor {
            query.push(("cursor", cursor.clone()));
        }
        let mut req = client.get(&url).query(&query);
        for (name, value) in signed_headers(&state.oauth.key, &authorization, Some(repo)) {
            req = req.header(name, value);
        }
        let res = req.send().await.map_err(|e| format!("{url}: {e}"))?;
        let status = res.status();
        let (body, size) = read(res).await?;
        bytes += size;
        if !status.is_success() {
            // No repo in the space yet: nothing to read.
            if body.get("error").and_then(Value::as_str) == Some("RepoNotFound") {
                return Ok(Read::whole(all));
            }
            return Err(format!("listRepoOps of {repo} in {space} answered {status}: {body}"));
        }
        let ops = body.get("ops").and_then(Value::as_array).cloned().unwrap_or_default();
        let empty = ops.is_empty();
        all.extend(ops);
        cursor = body.get("cursor").and_then(Value::as_str).map(str::to_owned);
        if cursor.is_none() || empty {
            break;
        }
        // The rest of an intake repo is read next time, from where this
        // stopped (ops come oldest first).
        if intake && (all.len() as i64 >= INTAKE_RECORDS_PER_REPO || bytes >= INTAKE_READ_CAP) {
            return Ok(Read::cut_short(all));
        }
    }
    Ok(Read::whole(all))
}

/// Ops read from a repo, and the revision the next read goes on from.
struct Read {
    ops: Vec<Value>,
    through: Option<String>,
}

impl Read {
    /// Everything there was: the next read goes on from the latest.
    fn whole(ops: Vec<Value>) -> Self {
        let through = latest_rev(&ops);
        Self { ops, through }
    }

    /// A read stopped at a cap, perhaps partway through a commit (every op
    /// in a commit has its revision, and the next read asks for later
    /// ones): it goes on from the last commit read whole. A commit too big
    /// to read whole within the caps is junk, and is skipped rather than
    /// read again and again.
    fn cut_short(ops: Vec<Value>) -> Self {
        let last = latest_rev(&ops);
        let through = latest_rev(
            ops.iter().filter(|op| op.get("rev").and_then(Value::as_str) != last.as_deref()),
        )
        .or(last);
        Self { ops, through }
    }
}

/// The latest revision among some ops.
fn latest_rev<'a>(ops: impl IntoIterator<Item = &'a Value>) -> Option<String> {
    ops.into_iter().filter_map(|op| op.get("rev").and_then(Value::as_str)).max().map(str::to_owned)
}

/// Reads a repo's new ops in a space into the index. Safe to run twice at
/// once: a record only ever moves to a later revision. Says whether the
/// organization's view changed: only a stored record it's derived from
/// ([`index::derives_from`]) changes it, so a member's plan, or junk in an
/// intake space (which isn't stored), costs no re-derive or comparison.
///
/// The view is bumped before the repo is marked read: a sync running
/// alongside that finds nothing new to read then never answers from a view
/// older than what this stored.
pub async fn sync_repo(state: &AppState, space: &str, repo: &str) -> Result<bool, String> {
    let parsed = SpaceUri::parse(space).ok_or_else(|| format!("{space} isn't a space"))?;
    let since = sqlx::query_scalar::<_, String>(
        "SELECT synced_rev FROM space_repos WHERE space = $1 AND repo = $2",
    )
    .bind(space)
    .bind(repo)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    let Read { ops, through } = fetch_ops(state, space, repo, since.as_deref(), true).await?;
    if ops.is_empty() {
        return Ok(false);
    }
    let seen_at = now_ms();
    let mut db = state.db.acquire().await.map_err(|e| e.to_string())?;
    let mut room = Room::of(&mut db, space, repo).await?;
    let mut changed = false;
    // Whatever was written before a failure is still bumped for, so it's
    // never stored without the view being re-derived.
    let mut outcome = Ok(());
    for op in &ops {
        if let Some((collection, rkey, version)) = version(op)
            && indexed(space, &collection, &version)
        {
            match apply(&mut db, space, repo, &collection, &rkey, &version, seen_at, room.as_mut())
                .await
            {
                Ok(wrote) => changed |= wrote && index::derives_from(&parsed, &collection),
                Err(why) => {
                    outcome = Err(why);
                    break;
                }
            }
        }
    }
    drop(db);
    if changed && let Err(why) = index::bump(state, &parsed.authority).await {
        // Not marked read: the next sync reads it again, and bumps.
        return Err(why);
    }
    if outcome.is_ok()
        && let Some(latest) = through
    {
        outcome = sqlx::query(
            "INSERT INTO space_repos (space, repo, synced_rev) VALUES ($1, $2, $3) \
             ON CONFLICT (space, repo) DO UPDATE SET synced_rev = excluded.synced_rev \
             WHERE space_repos.synced_rev < excluded.synced_rev",
        )
        .bind(space)
        .bind(repo)
        .bind(&latest)
        .execute(&state.db)
        .await
        .map(drop)
        .map_err(|e| e.to_string());
    }
    outcome.map(|()| changed)
}

/// The records a repo already has in an intake space, read once per sync
/// (not once per op) to hold it to [`INTAKE_RECORDS_PER_REPO`]. Other
/// spaces have no limit.
struct Room(BTreeSet<(String, String)>);

impl Room {
    async fn of(
        db: &mut sqlx::AnyConnection,
        space: &str,
        repo: &str,
    ) -> Result<Option<Room>, String> {
        if SpaceUri::parse(space).is_none_or(|s| s.kind != INTAKE_TYPE) {
            return Ok(None);
        }
        let keys = sqlx::query_as::<_, (String, String)>(
            "SELECT collection, rkey FROM space_records WHERE space = $1 AND repo = $2",
        )
        .bind(space)
        .bind(repo)
        .fetch_all(&mut *db)
        .await
        .map_err(|e| e.to_string())?;
        Ok(Some(Room(keys.into_iter().collect())))
    }

    /// Whether a record may be written: one the repo has, or a new one
    /// while there's room.
    fn admits(&self, collection: &str, rkey: &str) -> bool {
        (self.0.len() as i64) < INTAKE_RECORDS_PER_REPO
            || self.0.contains(&(collection.to_owned(), rkey.to_owned()))
    }
}

/// A code record's HMAC, for finding a code's conference without deriving
/// every organization: only for code records in an admin space, the only
/// ones that count.
fn code_hmac_of(space: &str, collection: &str, value: Option<&str>) -> Option<String> {
    if collection != index::CODE || SpaceUri::parse(space).is_none_or(|s| s.kind != ADMIN_TYPE) {
        return None;
    }
    let value: Value = serde_json::from_str(value?).ok()?;
    value.get("codeHash").and_then(Value::as_str).map(str::to_owned)
}

/// Writes a record's version into the index, unless it has a later one, and
/// notes when this version was first seen. A repo already at its limit of
/// records in an intake space (its `room`) gets no new ones. Says whether
/// the index changed.
#[allow(clippy::too_many_arguments)]
async fn apply(
    db: &mut sqlx::AnyConnection,
    space: &str,
    repo: &str,
    collection: &str,
    rkey: &str,
    version: &Version,
    seen_at: i64,
    room: Option<&mut Room>,
) -> Result<bool, String> {
    if room.as_ref().is_some_and(|room| !room.admits(collection, rkey)) {
        return Ok(false);
    }
    let code_hmac = code_hmac_of(space, collection, version.value.as_deref());
    let written = sqlx::query(
        "INSERT INTO space_records (space, repo, collection, rkey, rev, cid, value, code_hmac) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
         ON CONFLICT (space, repo, collection, rkey) DO UPDATE SET rev = excluded.rev, cid = excluded.cid, \
         value = excluded.value, code_hmac = excluded.code_hmac WHERE space_records.rev <= excluded.rev",
    )
    .bind(space)
    .bind(repo)
    .bind(collection)
    .bind(rkey)
    .bind(&version.rev)
    .bind(&version.cid)
    .bind(&version.value)
    .bind(code_hmac)
    .execute(&mut *db)
    .await
    .map_err(|e| e.to_string())?
    .rows_affected()
        > 0;
    if !written {
        return Ok(false);
    }
    if let Some(room) = room {
        room.0.insert((collection.to_owned(), rkey.to_owned()));
    }
    sqlx::query(
        "INSERT INTO space_record_seen (space, repo, collection, rkey, rev, seen_at) VALUES ($1, $2, $3, $4, $5, $6) \
         ON CONFLICT (space, repo, collection, rkey, rev) DO NOTHING",
    )
    .bind(space)
    .bind(repo)
    .bind(collection)
    .bind(rkey)
    .bind(&version.rev)
    .bind(seen_at)
    .execute(&mut *db)
    .await
    .map_err(|e| e.to_string())?;
    Ok(true)
}

/// A response's JSON body, and how many bytes it was.
async fn read(mut res: reqwest::Response) -> Result<(Value, usize), String> {
    let url = res.url().to_string();
    let mut body = Vec::new();
    while let Some(chunk) = res.chunk().await.map_err(|e| format!("{url}: {e}"))? {
        if body.len() + chunk.len() > PAGE_CAP {
            return Err(format!("{url} sent more than {PAGE_CAP} bytes"));
        }
        body.extend_from_slice(&chunk);
    }
    Ok((serde_json::from_slice(&body).unwrap_or(Value::Null), body.len()))
}

/// One repo in one space, as reindex read it: each record's latest version.
type Repo = BTreeMap<(String, String), Version>;

/// Applies ops to what's known of a repo in a space, as `sync_repo` and
/// `apply` do to the index.
fn absorb(space: &str, repo: &mut Repo, ops: &[Value]) {
    let intake = SpaceUri::parse(space).is_some_and(|s| s.kind == INTAKE_TYPE);
    for op in ops {
        if let Some((collection, rkey, version)) = version(op)
            && indexed(space, &collection, &version)
        {
            let key = (collection, rkey);
            let full = intake && repo.len() as i64 >= INTAKE_RECORDS_PER_REPO;
            if full && !repo.contains_key(&key) {
                continue;
            }
            if repo.get(&key).is_none_or(|v| v.rev <= version.rev) {
                repo.insert(key, version);
            }
        }
    }
}

/// The records an organization's permissions are derived from, out of the
/// repos read so far (as `index::records` reads them from the index).
fn recs_of(org: &str, read: &BTreeMap<(String, String), Repo>) -> Vec<Rec> {
    let mut recs = Vec::new();
    for ((space, repo), records) in read {
        let Some(parsed) = SpaceUri::parse(space).filter(|s| s.authority == org) else { continue };
        for ((collection, rkey), version) in records {
            let counted = index::derives_from(&parsed, collection);
            let (Some(value), true) = (&version.value, counted) else { continue };
            let (Some(us), Ok(value)) = (tid_micros(&version.rev), serde_json::from_str(value))
            else {
                continue;
            };
            recs.push(Rec {
                space: space.clone(),
                repo: repo.clone(),
                collection: collection.clone(),
                rkey: rkey.clone(),
                rev: version.rev.clone(),
                us,
                seen_us: None,
                value,
            });
        }
    }
    recs
}

/// What reindex did: the organization as rebuilt, and the repos it couldn't
/// read, whose records were kept as they were.
pub struct Reindexed {
    pub org: Arc<Org>,
    pub unread: Vec<String>,
}

/// Rebuilds an organization's index by crawling from its super admin: their
/// repo in the admin space names the admins and the spaces; each admin's
/// repo there, every repo in each space's writer set, and every repo read
/// before, is read again from scratch.
///
/// Everything is read first, and the index is replaced in one transaction,
/// so requests meanwhile see the old index, never a partial one. A repo that
/// can't be read keeps its old records (and is reported), rather than
/// vanishing and taking its author's access with it.
pub async fn reindex(state: &AppState, org: &str) -> Result<Reindexed, String> {
    let authority = super::authority::get(&state.db, org)
        .await?
        .ok_or_else(|| format!("no organization {org}"))?;
    // Anything first seen from now on came from a sync running alongside,
    // and is newer than what this reads: it's kept.
    let started = now_ms();
    let admin_space = SpaceUri::admin(org).to_string();
    let pattern = format!("at://{org}/space/%");
    // The repos read before, as well as the writer sets, say where to look.
    let known = sqlx::query_as::<_, (String, String)>(
        "SELECT space, repo FROM space_repos WHERE space LIKE $1",
    )
    .bind(&pattern)
    .fetch_all(&state.db)
    .await
    .map_err(|e| e.to_string())?;

    let mut read: BTreeMap<(String, String), Repo> = BTreeMap::new();
    let mut synced: BTreeMap<(String, String), String> = BTreeMap::new();
    let mut unread: BTreeSet<(String, String)> = BTreeSet::new();
    let mut fetch = async |space: &str, repo: &str, read: &mut BTreeMap<_, Repo>| {
        // Uncapped: what isn't read here is deleted below, and a capped
        // read's rest would never be read again.
        let Read { ops, through } = fetch_ops(state, space, repo, None, false).await?;
        let key = (space.to_owned(), repo.to_owned());
        if let Some(latest) = through {
            synced.insert(key.clone(), latest);
        }
        absorb(space, read.entry(key).or_default(), &ops);
        Ok::<_, String>(())
    };
    // From the super admin, out. Without their repo there's nothing to
    // rebuild from, so the index is left alone.
    fetch(&admin_space, &authority.super_admin, &mut read).await.map_err(|why| {
        format!("couldn't read the super admin's repo, so nothing was changed: {why}")
    })?;
    let eventside = state.oauth.client_id_for("atproto");
    let derive = |read: &BTreeMap<(String, String), Repo>| {
        index::derive(
            org,
            &authority.super_admin,
            authority.created_at as u64 * 1000,
            &recs_of(org, read),
            &state.secrets,
            &eventside,
        )
    };
    let mut done = BTreeSet::from([(admin_space.clone(), authority.super_admin.clone())]);
    let mut known = Some(known);
    // Admins' repos, then every repo in every space, until nothing new turns up.
    for _ in 0..10 {
        let view = derive(&read);
        let mut todo = known.take().unwrap_or_default();
        for admin in view.admins.keys() {
            todo.push((admin_space.clone(), admin.clone()));
        }
        for space in view.spaces() {
            for writer in writers(state, &space).await? {
                todo.push((space.clone(), writer));
            }
            // The conference super admin's roles and rules.
            if let Some(conference) = view.conference(&space) {
                todo.push((space.clone(), conference.super_admin().to_owned()));
            }
        }
        let fresh: Vec<_> = todo.into_iter().filter(|t| !done.contains(t)).collect();
        if fresh.is_empty() {
            break;
        }
        for (space, repo) in fresh {
            if let Err(why) = fetch(&space, &repo, &mut read).await {
                eprintln!("reindex: {why}");
                unread.insert((space.clone(), repo.clone()));
            }
            done.insert((space, repo));
        }
    }

    // Replace the organization's index at once, keeping what the repos that
    // couldn't be read had, and whatever a sync running alongside wrote:
    // records first seen since this started, and how far it read.
    let mut tx = state.db.begin().await.map_err(|e| e.to_string())?;
    let keeps = |space: &str, repo: &str| unread.contains(&(space.to_owned(), repo.to_owned()));
    let old_repos = sqlx::query_as::<_, (String, String)>(
        "SELECT DISTINCT space, repo FROM space_records WHERE space LIKE $1",
    )
    .bind(&pattern)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    for (space, repo) in old_repos.iter().filter(|(s, r)| !keeps(s, r)) {
        sqlx::query(
            "DELETE FROM space_records WHERE space = $1 AND repo = $2 AND NOT EXISTS ( \
             SELECT 1 FROM space_record_seen s WHERE s.space = space_records.space \
             AND s.repo = space_records.repo AND s.collection = space_records.collection \
             AND s.rkey = space_records.rkey AND s.rev = space_records.rev AND s.seen_at >= $3)",
        )
        .bind(space)
        .bind(repo)
        .bind(started)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        if !read.contains_key(&(space.clone(), repo.clone())) {
            sqlx::query("DELETE FROM space_repos WHERE space = $1 AND repo = $2")
                .bind(space)
                .bind(repo)
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    let seen_at = now_ms();
    for ((space, repo), records) in &read {
        let mut room = Room::of(&mut tx, space, repo).await?;
        for ((collection, rkey), version) in records {
            apply(&mut tx, space, repo, collection, rkey, version, seen_at, room.as_mut()).await?;
        }
        if let Some(latest) = synced.get(&(space.clone(), repo.clone())) {
            sqlx::query(
                "INSERT INTO space_repos (space, repo, synced_rev) VALUES ($1, $2, $3) \
                 ON CONFLICT (space, repo) DO UPDATE SET synced_rev = excluded.synced_rev \
                 WHERE space_repos.synced_rev < excluded.synced_rev",
            )
            .bind(space)
            .bind(repo)
            .bind(latest)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        }
    }
    tx.commit().await.map_err(|e| format!("couldn't replace the index: {e}"))?;
    index::bump(state, org).await?;
    let org = index::load(state, org).await?.ok_or("the organization vanished")?;
    let unread = unread.iter().map(|(space, repo)| format!("{repo} in {space}")).collect();
    Ok(Reindexed { org, unread })
}

/// The DIDs in a space's writer set.
pub async fn writers(state: &AppState, space: &str) -> Result<Vec<String>, String> {
    sqlx::query_scalar::<_, String>("SELECT did FROM space_writers WHERE space = $1 ORDER BY did")
        .bind(space)
        .fetch_all(&state.db)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::spacehost::CONFERENCE_TYPE;

    #[test]
    fn a_repo_keeps_each_records_latest_version() {
        let mut repo = Repo::new();
        let space = "at://did:plc:org/space/app.eventside.conference/3conf";
        absorb(
            space,
            &mut repo,
            &[
                json!({ "collection": "a.b.c", "rkey": "1", "rev": "3aaa", "cid": "x", "value": { "n": 1 } }),
                json!({ "collection": "a.b.c", "rkey": "2", "rev": "3aab", "cid": "y" }),
                json!({ "collection": "a.b.c", "rkey": "3", "rev": "3aac", "cid": "z", "value": { "n": 3 } }),
                json!({ "collection": "a.b.c", "rkey": "3", "rev": "3aad" }),
            ],
        );
        // An older op read later doesn't bring a record back.
        absorb(
            space,
            &mut repo,
            &[
                json!({ "collection": "a.b.c", "rkey": "3", "rev": "3aac", "cid": "z", "value": { "n": 3 } }),
            ],
        );
        let key = |rkey: &str| ("a.b.c".to_owned(), rkey.to_owned());
        assert_eq!(repo[&key("1")].value.as_deref(), Some(r#"{"n":1}"#));
        assert!(!repo.contains_key(&key("2")), "a write whose value a later op carries");
        assert_eq!(repo[&key("3")].rev, "3aad");
        assert!(repo[&key("3")].value.is_none(), "deleted");
    }

    fn intake_space() -> String {
        SpaceUri::new("did:plc:atmosphereorgaaaaaaaaaaa", INTAKE_TYPE, "3conf").to_string()
    }

    #[test]
    fn an_intake_space_keeps_only_joins_and_leaves() {
        let space = intake_space();
        let big = "x".repeat(INTAKE_RECORD_CAP + 1);
        let mut repo = Repo::new();
        absorb(
            &space,
            &mut repo,
            &[
                json!({ "collection": index::JOIN, "rkey": "1", "rev": "3aaa", "cid": "a", "value": {} }),
                json!({ "collection": index::LEAVE, "rkey": "2", "rev": "3aab", "cid": "b", "value": {} }),
                json!({ "collection": "com.example.junk", "rkey": "3", "rev": "3aac", "cid": "c", "value": {} }),
                json!({ "collection": index::JOIN, "rkey": "4", "rev": "3aad", "cid": "d", "value": { "pad": big } }),
            ],
        );
        let kept: Vec<&str> = repo.keys().map(|(_, rkey)| rkey.as_str()).collect();
        assert_eq!(kept, ["1", "2"]);

        // However many a writer makes, only so many are kept.
        let ops: Vec<Value> = (0..INTAKE_RECORDS_PER_REPO + 50)
            .map(|i| json!({ "collection": index::JOIN, "rkey": format!("r{i:05}"), "rev": format!("3b{i:05}"), "cid": "x", "value": {} }))
            .collect();
        let mut repo = Repo::new();
        absorb(&space, &mut repo, &ops);
        assert_eq!(repo.len() as i64, INTAKE_RECORDS_PER_REPO);

        // Nor are other records read back out of an intake space.
        let mut read = BTreeMap::new();
        let mut junk = Repo::new();
        junk.insert(
            ("com.example.junk".to_owned(), "1".to_owned()),
            Version { rev: "3aaaaaaaaaa22".into(), cid: None, value: Some("{}".into()) },
        );
        read.insert((space.clone(), "did:plc:writer".to_owned()), junk);
        assert!(recs_of("did:plc:atmosphereorgaaaaaaaaaaa", &read).is_empty());
    }

    #[tokio::test]
    async fn only_a_write_that_changes_the_index_counts_as_a_change() {
        let dir =
            std::env::temp_dir().join(format!("eventside-sync-{}", crate::keys::random_token(8)));
        let url = format!("sqlite://{}/eventside.db?mode=rwc", dir.display());
        let db = crate::db::connect(&url).await.unwrap();
        let mut conn = db.acquire().await.unwrap();
        let space = intake_space();
        let w = "did:plc:writer";
        let v = |rev: &str| Version {
            rev: rev.into(),
            cid: Some("c".into()),
            value: Some("{}".into()),
        };

        let mut room = Room::of(&mut conn, &space, w).await.unwrap();
        assert!(room.is_some(), "an intake space has a limit");
        let mut apply_join = async |rkey: &str, rev: &str, room: Option<&mut Room>| {
            apply(&mut conn, &space, w, index::JOIN, rkey, &v(rev), 1, room).await.unwrap()
        };
        assert!(apply_join("1", "3aab", room.as_mut()).await);
        // An older version: nothing changed, so nothing is re-derived.
        assert!(!apply_join("1", "3aaa", room.as_mut()).await);
        // The same version again (a sync racing another, which may not
        // have bumped yet) counts, so the caller never reads a stale view.
        assert!(apply_join("1", "3aab", room.as_mut()).await);
        assert!(apply_join("1", "3aac", room.as_mut()).await);

        // A full repo gets no new records, but its own still update.
        let mut full = Room(
            (1..INTAKE_RECORDS_PER_REPO)
                .map(|i| (index::JOIN.to_owned(), format!("x{i}")))
                .chain([(index::JOIN.to_owned(), "1".to_owned())])
                .collect(),
        );
        assert!(!apply_join("2", "3aad", Some(&mut full)).await);
        assert!(apply_join("1", "3aae", Some(&mut full)).await);

        // What's there is read once, for the whole sync.
        let room = Room::of(&mut conn, &space, w).await.unwrap().unwrap();
        assert_eq!(room.0.len(), 1);
        let conference =
            SpaceUri::new("did:plc:atmosphereorgaaaaaaaaaaa", CONFERENCE_TYPE, "3conf");
        assert!(Room::of(&mut conn, &conference.to_string(), w).await.unwrap().is_none());
        drop(conn);
        db.close().await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_read_cut_short_goes_on_from_the_last_commit_read_whole() {
        let op = |rkey: &str, rev: &str| json!({ "collection": index::JOIN, "rkey": rkey, "rev": rev, "cid": "c", "value": {} });
        // Stopped partway through the commit at 3aab: it's read again.
        let read = Read::cut_short(vec![op("1", "3aaa"), op("2", "3aab"), op("3", "3aab")]);
        assert_eq!(read.through.as_deref(), Some("3aaa"));
        // A read that wasn't cut short goes on from the latest.
        let read = Read::whole(vec![op("1", "3aaa"), op("2", "3aab")]);
        assert_eq!(read.through.as_deref(), Some("3aab"));
        // One commit too big for the caps is skipped, not read for ever.
        let read = Read::cut_short(vec![op("1", "3aab"), op("2", "3aab")]);
        assert_eq!(read.through.as_deref(), Some("3aab"));
    }

    #[test]
    fn reindex_reads_only_what_the_index_derives_from() {
        let org = "did:plc:atmosphereorgaaaaaaaaaaa";
        let conference = SpaceUri::new(org, CONFERENCE_TYPE, "3conf").to_string();
        let v = Version { rev: "3aaaaaaaaaa22".into(), cid: None, value: Some("{}".into()) };
        let mut repo = Repo::new();
        repo.insert((index::ROLE.to_owned(), "a".to_owned()), v.clone());
        repo.insert(("community.lexicon.calendar.event".to_owned(), "b".to_owned()), v);
        let read = BTreeMap::from([((conference, "did:plc:writer".to_owned()), repo)]);
        let recs = recs_of(org, &read);
        assert_eq!(recs.iter().map(|r| r.collection.as_str()).collect::<Vec<_>>(), [index::ROLE]);
    }

    #[test]
    fn only_an_admin_spaces_code_records_name_a_code() {
        let value = Some(r#"{"codeHash":"h"}"#);
        let admin = SpaceUri::admin("did:plc:atmosphereorgaaaaaaaaaaa").to_string();
        assert_eq!(code_hmac_of(&admin, index::CODE, value).as_deref(), Some("h"));
        assert_eq!(code_hmac_of(&intake_space(), index::CODE, value), None);
        let conference =
            SpaceUri::new("did:plc:atmosphereorgaaaaaaaaaaa", CONFERENCE_TYPE, "3conf").to_string();
        assert_eq!(code_hmac_of(&conference, index::CODE, value), None);
        assert_eq!(code_hmac_of(&admin, index::JOIN, value), None);
    }
}
