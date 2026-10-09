//! The conference XRPC the PWA uses. Stubs: every method answers 501.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

fn not_implemented(nsid: &str) -> Response {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({ "error": "NotImplemented", "message": format!("not implemented: {nsid}") })),
    )
        .into_response()
}

/// `app.eventside.conference.get`: a conference's public view, and the
/// viewer's membership when signed in.
pub async fn get() -> Response {
    not_implemented("app.eventside.conference.get")
}

/// `app.eventside.conference.join`: answers `joined`, `pending`, `emailNeeded`
/// or `refused`.
pub async fn join() -> Response {
    not_implemented("app.eventside.conference.join")
}

/// `app.eventside.conference.leave`.
pub async fn leave() -> Response {
    not_implemented("app.eventside.conference.leave")
}

/// `app.eventside.conference.getMembership`: whether the signed-in person is
/// a member now, and in which role.
pub async fn get_membership() -> Response {
    not_implemented("app.eventside.conference.getMembership")
}
