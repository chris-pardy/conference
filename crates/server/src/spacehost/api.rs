//! Our host API (`app.eventside.space.*`), for any app holding a credential
//! for one of our spaces: views derived from the admin space, for apps that
//! can read a conference space but not the admin space.

use axum::Json;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use super::index::{self, Org, Period};
use super::{CONFERENCE_TYPE, SpaceUri, credential, internal, invalid, refuse};
use crate::AppState;

#[derive(serde::Deserialize)]
pub struct SpaceParams {
    space: String,
}

/// Checks the credential, and loads the organization.
async fn authorized(
    state: &AppState,
    headers: &HeaderMap,
    space: &str,
) -> Result<(SpaceUri, std::sync::Arc<Org>), Response> {
    let space = SpaceUri::parse(space).ok_or_else(|| invalid("InvalidRequest", "Invalid space"))?;
    let org = match index::load_for_space(state, &space).await {
        Ok(Some(org)) if org.knows(&space) => org,
        Ok(_) => return Err(refuse(StatusCode::NOT_FOUND, "SpaceNotFound", "Space not found")),
        Err(why) => return Err(internal(why)),
    };
    credential::check(state, headers, &space).await?;
    Ok((space, org))
}

/// `app.eventside.space.getSpace`: a space's policies, app access, and the
/// organization's admins.
pub async fn get_space(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<SpaceParams>,
) -> Response {
    let (space, org) = match authorized(&state, &headers, &params.space).await {
        Ok(found) => found,
        Err(res) => return res,
    };
    let (read, write) = org.policies(&space);
    let admins: Vec<Value> = org
        .admins
        .iter()
        .map(|(did, admin)| json!({ "did": did, "role": admin.role.as_str(), "since": index::iso(admin.since_us) }))
        .collect();
    Json(json!({
        "space": space.to_string(),
        "readPolicy": read,
        "writePolicy": write,
        "appAccess": org.app_access(&space).to_json(),
        "superAdmin": org.super_admin,
        "admins": admins,
    }))
    .into_response()
}

#[derive(serde::Deserialize)]
pub struct MembersParams {
    space: String,
    limit: Option<i64>,
    cursor: Option<String>,
}

/// How many members a page of `listMembers` has, unless asked for fewer.
const MEMBERS_PAGE: i64 = 100;
/// The most members a page of `listMembers` has.
const MEMBERS_PAGE_MAX: i64 = 500;

/// One page of DIDs, in order: those after the cursor, at most `limit`, and
/// the cursor for the next page if there's more.
fn page_of<'a, T>(
    items: impl Iterator<Item = (&'a String, T)>,
    after: Option<&str>,
    limit: usize,
) -> (Vec<(&'a String, T)>, Option<String>) {
    let mut page: Vec<_> = items
        .filter(|(did, _)| after.is_none_or(|after| did.as_str() > after))
        .take(limit + 1)
        .collect();
    let more = page.len() > limit;
    page.truncate(limit);
    let cursor = more.then(|| page.last().map(|(did, _)| (*did).clone())).flatten();
    (page, cursor)
}

/// `app.eventside.space.listMembers`: everyone who is or was a member, with
/// their periods, by DID, paged by `limit` and `cursor`.
pub async fn list_members(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<MembersParams>,
) -> Response {
    let (space, org) = match authorized(&state, &headers, &params.space).await {
        Ok(found) => found,
        Err(res) => return res,
    };
    let entry = |did: &str, periods: &[Period], current: bool, write: bool| {
        json!({
            "did": did,
            "read": current,
            "write": current && write,
            "periods": periods.iter().map(Period::to_json).collect::<Vec<_>>(),
        })
    };
    let limit = params.limit.unwrap_or(MEMBERS_PAGE).clamp(1, MEMBERS_PAGE_MAX) as usize;
    let after = params.cursor.as_deref();
    let (members, cursor): (Vec<Value>, _) = if space.kind == CONFERENCE_TYPE {
        let Some(conference) = org.conference(&space.to_string()) else {
            return refuse(StatusCode::NOT_FOUND, "SpaceNotFound", "Space not found");
        };
        let (page, cursor) = page_of(conference.members.iter(), after, limit);
        let members = page
            .into_iter()
            .map(|(did, periods)| entry(did, periods, conference.is_member(did), true))
            .collect();
        (members, cursor)
    } else {
        // The admin space, and intake spaces (which admins read, and anyone writes).
        let writes = space.kind == super::ADMIN_TYPE;
        let readers = org.readers(&space.to_string());
        let (page, cursor) = page_of(readers.iter().map(|did| (did, ())), after, limit);
        let members = page
            .into_iter()
            .map(|(did, ())| {
                let since = org.admins.get(did).map_or(org.created_us, |a| a.since_us);
                entry(did, &[Period { since, until: None }], true, writes)
            })
            .collect();
        (members, cursor)
    };
    let mut out = json!({ "members": members });
    if let Some(cursor) = cursor {
        out["cursor"] = json!(cursor);
    }
    Json(out).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn members_are_paged_by_did() {
        let dids: Vec<String> = (0..5).map(|i| format!("did:plc:m{i}")).collect();
        let (first, cursor) = page_of(dids.iter().map(|d| (d, ())), None, 2);
        assert_eq!(
            first.iter().map(|(d, _)| d.as_str()).collect::<Vec<_>>(),
            ["did:plc:m0", "did:plc:m1"]
        );
        let cursor = cursor.expect("there's more");
        let (second, _) = page_of(dids.iter().map(|d| (d, ())), Some(&cursor), 2);
        assert_eq!(
            second.iter().map(|(d, _)| d.as_str()).collect::<Vec<_>>(),
            ["did:plc:m2", "did:plc:m3"]
        );
        let (last, cursor) = page_of(dids.iter().map(|d| (d, ())), Some("did:plc:m3"), 2);
        assert_eq!(last.len(), 1);
        assert_eq!(cursor, None);
    }
}
