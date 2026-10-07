//! Reading repos into the index: a writer's ops in one of our spaces, from
//! its own PDS (`com.atproto.space.listRepoOps`), with a credential we mint
//! for ourselves, so the appview reads a space like any other app.

use serde_json::Value;

use super::credential::{for_self, signed_headers};
use super::{SpaceUri, index::Org};
use crate::AppState;

/// The most of a page of ops read at once.
const PAGE: usize = 100;
/// The most bytes a page of ops may be.
const PAGE_CAP: usize = 4 * 1024 * 1024;

/// Reads a repo's new ops in a space into the index. Safe to run twice at
/// once: a record only ever moves to a later revision.
pub async fn sync_repo(state: &AppState, space: &str, repo: &str) -> Result<(), String> {
    let parsed = SpaceUri::parse(space).ok_or_else(|| format!("{space} isn't a space"))?;
    let since = sqlx::query_scalar::<_, String>(
        "SELECT synced_rev FROM space_repos WHERE space = $1 AND repo = $2",
    )
    .bind(space)
    .bind(repo)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    let pds = state.host.pds_of(state, repo).await?;
    let credential = for_self(state, &parsed).await?;
    let url = format!("{pds}/xrpc/com.atproto.space.listRepoOps");
    let client = state.http.guarded(&url)?;
    let authorization = format!("Atproto-Space {credential}");
    let mut cursor: Option<String> = None;
    let mut latest = since.clone();
    for _ in 0..1000 {
        let mut query = vec![
            ("space", space.to_owned()),
            ("repo", repo.to_owned()),
            ("limit", PAGE.to_string()),
        ];
        if let Some(since) = &since {
            query.push(("since", since.clone()));
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
        let body = read(res).await?;
        if !status.is_success() {
            // No repo in the space yet: nothing to read.
            if body.get("error").and_then(Value::as_str) == Some("RepoNotFound") {
                return Ok(());
            }
            return Err(format!("listRepoOps of {repo} in {space} answered {status}: {body}"));
        }
        let ops = body.get("ops").and_then(Value::as_array).cloned().unwrap_or_default();
        for op in &ops {
            apply(state, space, repo, op).await?;
            if let Some(rev) = op.get("rev").and_then(Value::as_str)
                && latest.as_deref().is_none_or(|l| rev > l)
            {
                latest = Some(rev.to_owned());
            }
        }
        cursor = body.get("cursor").and_then(Value::as_str).map(str::to_owned);
        if cursor.is_none() || ops.is_empty() {
            break;
        }
    }
    if let Some(latest) = latest {
        sqlx::query(
            "INSERT INTO space_repos (space, repo, synced_rev) VALUES ($1, $2, $3) \
             ON CONFLICT (space, repo) DO UPDATE SET synced_rev = excluded.synced_rev \
             WHERE space_repos.synced_rev < excluded.synced_rev",
        )
        .bind(space)
        .bind(repo)
        .bind(&latest)
        .execute(&state.db)
        .await
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Applies one op: a record's value at a revision, or its deletion.
async fn apply(state: &AppState, space: &str, repo: &str, op: &Value) -> Result<(), String> {
    let (Some(collection), Some(rkey), Some(rev)) = (
        op.get("collection").and_then(Value::as_str),
        op.get("rkey").and_then(Value::as_str),
        op.get("rev").and_then(Value::as_str),
    ) else {
        return Ok(());
    };
    let cid = op.get("cid").and_then(Value::as_str);
    let value = match (cid, op.get("value")) {
        // Written, and still current: its value is inlined.
        (Some(_), Some(value)) => Some(value.to_string()),
        // Written, but changed since: a later op carries what it is now.
        (Some(_), None) => return Ok(()),
        // Deleted.
        (None, _) => None,
    };
    sqlx::query(
        "INSERT INTO space_records (space, repo, collection, rkey, rev, cid, value) VALUES ($1, $2, $3, $4, $5, $6, $7) \
         ON CONFLICT (space, repo, collection, rkey) DO UPDATE SET rev = excluded.rev, cid = excluded.cid, \
         value = excluded.value WHERE space_records.rev <= excluded.rev",
    )
    .bind(space)
    .bind(repo)
    .bind(collection)
    .bind(rkey)
    .bind(rev)
    .bind(cid)
    .bind(value)
    .execute(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

async fn read(mut res: reqwest::Response) -> Result<Value, String> {
    let url = res.url().to_string();
    let mut body = Vec::new();
    while let Some(chunk) = res.chunk().await.map_err(|e| format!("{url}: {e}"))? {
        if body.len() + chunk.len() > PAGE_CAP {
            return Err(format!("{url} sent more than {PAGE_CAP} bytes"));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(serde_json::from_slice(&body).unwrap_or(Value::Null))
}

/// Forgets what the index has of a repo in a space, so the next sync reads
/// it all again.
pub async fn forget(state: &AppState, space: &str, repo: &str) -> Result<(), String> {
    for sql in [
        "DELETE FROM space_records WHERE space = $1 AND repo = $2",
        "DELETE FROM space_repos WHERE space = $1 AND repo = $2",
    ] {
        sqlx::query(sql)
            .bind(space)
            .bind(repo)
            .execute(&state.db)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Rebuilds an organization's index by crawling from its super admin: their
/// repo in the admin space names the admins and the spaces; each admin's
/// repo there, and every writer's repo in each space, is read again.
pub async fn reindex(state: &AppState, org: &str) -> Result<Org, String> {
    let authority = super::authority::get(&state.db, org)
        .await?
        .ok_or_else(|| format!("no organization {org}"))?;
    let admin_space = SpaceUri::admin(org).to_string();
    // The repos read before, as well as the writer sets, say where to look.
    let known = sqlx::query_as::<_, (String, String)>(
        "SELECT space, repo FROM space_repos WHERE space LIKE $1",
    )
    .bind(format!("at://{org}/space/%"))
    .fetch_all(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    // Everything the index had for this organization goes.
    for sql in [
        "DELETE FROM space_records WHERE space LIKE $1",
        "DELETE FROM space_repos WHERE space LIKE $1",
    ] {
        sqlx::query(sql)
            .bind(format!("at://{org}/space/%"))
            .execute(&state.db)
            .await
            .map_err(|e| e.to_string())?;
    }
    // From the super admin, out.
    sync_repo(state, &admin_space, &authority.super_admin).await?;
    let mut org_view = super::index::load(state, org).await?.ok_or("the organization vanished")?;
    let mut done = std::collections::BTreeSet::new();
    done.insert((admin_space.clone(), authority.super_admin.clone()));
    // Admins' repos, then every repo in every space, until nothing new turns up.
    let mut known = Some(known);
    for _ in 0..10 {
        let mut todo = known.take().unwrap_or_default();
        for admin in org_view.admins.keys() {
            todo.push((admin_space.clone(), admin.clone()));
        }
        for space in org_view.spaces() {
            for writer in writers(state, &space).await? {
                todo.push((space.clone(), writer));
            }
            // The conference super admin's roles and rules.
            if let Some(conference) = org_view.conference(&space)
                && let Some(super_admin) = conference.super_admin()
            {
                todo.push((space.clone(), super_admin.to_owned()));
            }
        }
        let fresh: Vec<_> = todo.into_iter().filter(|t| !done.contains(t)).collect();
        if fresh.is_empty() {
            break;
        }
        for (space, repo) in fresh {
            if let Err(why) = sync_repo(state, &space, &repo).await {
                eprintln!("reindex: {why}");
            }
            done.insert((space, repo));
        }
        org_view = super::index::load(state, org).await?.ok_or("the organization vanished")?;
    }
    Ok(org_view)
}

/// The DIDs in a space's writer set.
pub async fn writers(state: &AppState, space: &str) -> Result<Vec<String>, String> {
    sqlx::query_scalar::<_, String>("SELECT did FROM space_writers WHERE space = $1 ORDER BY did")
        .bind(space)
        .fetch_all(&state.db)
        .await
        .map_err(|e| e.to_string())
}
