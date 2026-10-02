//! Signing in: sessions, their cookie and CSRF token, the routes that create
//! and end them, and background token renewal.

pub mod cookies;
pub mod pds;
pub mod renew;
pub mod return_to;
pub mod routes;
pub mod session;

use axum::Json;
use axum::extract::{FromRequestParts, Request, State};
use axum::http::request::Parts;
use axum::http::{Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::AppState;
use session::{Lookup, SessionRow};

/// An XRPC error body.
pub fn xrpc_error(status: StatusCode, error: &str, message: &str) -> Response {
    (status, Json(json!({ "error": error, "message": message }))).into_response()
}

pub fn auth_required() -> Response {
    xrpc_error(StatusCode::UNAUTHORIZED, "AuthRequired", "Sign in to continue.")
}

/// The session ended. Its handle and DID let the PWA sign the same person back in.
pub fn expired(row: &SessionRow) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({
            "error": "SessionExpired",
            "message": "Your session has expired.",
            "handle": row.handle,
            "did": row.did,
        })),
    )
        .into_response()
}

/// The signed-in person, for any route that needs one. Rejects with
/// `AuthRequired` or `SessionExpired`.
pub struct CurrentUser {
    pub did: String,
    pub handle: String,
    pub scopes: Vec<String>,
    session: SessionRow,
}

impl CurrentUser {
    /// An HTTP client for the person's PDS, authenticated as them.
    pub async fn pds_client(&self, state: &AppState) -> Result<pds::PdsClient, pds::PdsError> {
        pds::PdsClient::for_session(state, &self.session.id_hash).await
    }
}

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        match session::lookup(state, &parts.headers).await {
            Ok(Lookup::Live(row)) => {
                if let Err(err) = session::touch(state, &row).await {
                    eprintln!("could not record session use: {err}");
                }
                Ok(Self {
                    did: row.did.clone(),
                    handle: row.handle.clone(),
                    scopes: row.scope_list(),
                    session: row,
                })
            }
            Ok(Lookup::Expired(row)) => Err(expired(&row)),
            Ok(Lookup::None) => Err(auth_required()),
            Err(err) => {
                eprintln!("could not load a session: {err}");
                Err(xrpc_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "InternalServerError",
                    "could not load the session",
                ))
            }
        }
    }
}

/// Every state-changing request (non-GET XRPC, and sign-out) must carry the
/// session's CSRF token in `X-CSRF-Token`.
pub async fn require_csrf(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let path = req.uri().path();
    let changes_state = !matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS);
    if !(changes_state && (path.starts_with("/xrpc/") || path == "/oauth/logout")) {
        return next.run(req).await;
    }
    let row = match session::lookup(&state, req.headers()).await {
        Ok(Lookup::Live(row)) => row,
        // An ended session holds no tokens: signing out of it needs no token either.
        Ok(Lookup::Expired(_)) if path == "/oauth/logout" => return next.run(req).await,
        Ok(Lookup::Expired(row)) => return expired(&row),
        Ok(Lookup::None) => return auth_required(),
        Err(err) => {
            eprintln!("could not load a session: {err}");
            return xrpc_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "InternalServerError",
                "could not load the session",
            );
        }
    };
    let sent = req.headers().get("x-csrf-token").and_then(|v| v.to_str().ok()).unwrap_or_default();
    if !constant_time_eq(sent.as_bytes(), row.csrf_token.as_bytes()) {
        return xrpc_error(
            StatusCode::FORBIDDEN,
            "InvalidCsrfToken",
            "This request needs the session's CSRF token.",
        );
    }
    next.run(req).await
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
