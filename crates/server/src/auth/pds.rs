//! Calls to a signed-in person's PDS, as them: DPoP-bound access tokens,
//! refreshed on demand when the background renewer hasn't got there first.

use std::time::Duration;

use reqwest::{Method, Response};

use super::renew::{self, Renewal};
use super::session;
use crate::AppState;
use crate::db::{ms, now_ms};
use crate::keys::{EcKey, dpop_proof};
use crate::oauth::OAuthClient;

#[derive(Debug)]
pub enum PdsError {
    /// The session ended; the person has to sign in again.
    SessionExpired,
    /// The PDS or authorization server couldn't be reached.
    Unavailable(String),
}

pub struct PdsClient {
    http: reqwest::Client,
    /// For the DPoP nonces each server last handed out.
    oauth: OAuthClient,
    pds: String,
    access_token: String,
    dpop_key: EcKey,
}

impl PdsClient {
    /// A client for a session, refreshing its tokens first if they're about to expire.
    pub async fn for_session(state: &AppState, id_hash: &str) -> Result<Self, PdsError> {
        let skew = ms(state.config.token_refresh_skew);
        // The skew decides when to start a refresh. Once one has happened,
        // its token is used as long as it hasn't expired, even when the
        // server's tokens don't outlive the skew.
        let mut renewed = false;
        for _ in 0..40 {
            let row = session::load(&state.db, id_hash)
                .await
                .map_err(|e| PdsError::Unavailable(e.to_string()))?
                .ok_or(PdsError::SessionExpired)?;
            if row.ended_at.is_some() {
                return Err(PdsError::SessionExpired);
            }
            let margin = if renewed { 0 } else { skew };
            let fresh = row.token_expires_at.is_some_and(|at| at > now_ms() + margin);
            if let (true, Some(pds), Some(token), Some(key)) =
                (fresh, &row.pds, &row.access_token, &row.dpop_key)
            {
                let pds = pds.clone();
                let http = state.http.guarded(&pds).map_err(PdsError::Unavailable)?.clone();
                let dpop_key = EcKey::from_jwk(key).map_err(PdsError::Unavailable)?;
                let oauth = state.oauth.clone();
                return Ok(Self { http, oauth, pds, access_token: token.clone(), dpop_key });
            }
            // Nothing to refresh with: the grant is over once its access token is.
            if row.refresh_token.is_none() {
                session::end(state, &row)
                    .await
                    .map_err(|e| PdsError::Unavailable(e.to_string()))?;
                return Err(PdsError::SessionExpired);
            }
            match renew::refresh(state, id_hash).await {
                Renewal::Renewed => renewed = true,
                Renewal::Ended => return Err(PdsError::SessionExpired),
                // Someone else is refreshing: wait for their tokens.
                Renewal::Busy => {
                    renewed = true;
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
                Renewal::Failed(why) => return Err(PdsError::Unavailable(why)),
            }
        }
        Err(PdsError::Unavailable("timed out waiting for a token refresh".into()))
    }

    /// Sends an XRPC request (`path` like `/xrpc/com.atproto.repo.createRecord`),
    /// retrying once with the PDS's DPoP nonce.
    pub async fn send(
        &self,
        method: Method,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<Response, PdsError> {
        let url = format!("{}{path}", self.pds);
        let mut nonce = self.oauth.nonce(&url);
        for _ in 0..2 {
            let proof = dpop_proof(
                &self.dpop_key,
                method.as_str(),
                &url,
                nonce.as_deref(),
                Some(&self.access_token),
            );
            let mut req = self
                .http
                .request(method.clone(), &url)
                .header("Authorization", format!("DPoP {}", self.access_token))
                .header("DPoP", proof);
            if let Some(body) = body {
                req = req.json(body);
            }
            let res = req.send().await.map_err(|e| PdsError::Unavailable(e.to_string()))?;
            let wants_nonce = res.status() == reqwest::StatusCode::UNAUTHORIZED
                && res
                    .headers()
                    .get("www-authenticate")
                    .and_then(|v| v.to_str().ok())
                    .is_some_and(|v| v.contains("use_dpop_nonce"));
            let offered =
                res.headers().get("dpop-nonce").and_then(|v| v.to_str().ok()).map(str::to_owned);
            if let Some(new) = &offered {
                self.oauth.remember_nonce(&url, new);
            }
            match (wants_nonce, offered) {
                (true, Some(new)) if nonce.as_deref() != Some(new.as_str()) => nonce = Some(new),
                _ => return Ok(res),
            }
        }
        Err(PdsError::Unavailable("the PDS kept asking for a new DPoP nonce".into()))
    }
}
