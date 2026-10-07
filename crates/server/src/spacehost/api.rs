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
) -> Result<(SpaceUri, Org), Response> {
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

/// `app.eventside.space.listMembers`: everyone who is or was a member, with
/// their periods.
pub async fn list_members(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<SpaceParams>,
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
    let members: Vec<Value> = if space.kind == CONFERENCE_TYPE {
        let Some(conference) = org.conference(&space.to_string()) else {
            return refuse(StatusCode::NOT_FOUND, "SpaceNotFound", "Space not found");
        };
        conference
            .members
            .iter()
            .map(|(did, periods)| entry(did, periods, conference.is_member(did), true))
            .collect()
    } else {
        // The admin space, and intake spaces (which admins read, and anyone writes).
        let writes = space.kind == super::ADMIN_TYPE;
        org.readers(&space.to_string())
            .iter()
            .map(|did| {
                let since = org.admins.get(did).map_or(org.created_us, |a| a.since_us);
                entry(did, &[Period { since, until: None }], true, writes)
            })
            .collect()
    };
    Json(json!({ "members": members })).into_response()
}
