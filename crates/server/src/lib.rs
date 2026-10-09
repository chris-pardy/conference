use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;

use axum::routing::{get, post};
use axum::{Json, Router, extract::State, middleware};

pub mod auth;
pub mod conference;
pub mod config;
pub mod crypto;
pub mod db;
pub mod identity;
pub mod keys;
pub mod net;
pub mod oauth;

pub mod health {
    use serde::Serialize;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
    #[serde(rename_all = "lowercase")]
    pub enum AtprotoStatus {
        Reachable,
        Unreachable,
    }

    #[derive(Debug, Clone, Serialize)]
    pub struct HealthReport {
        pub status: &'static str,
        pub atproto: AtprotoStatus,
    }

    /// The body of `GET /health`: the server is up whenever it can answer.
    pub fn health_report(atproto: AtprotoStatus) -> HealthReport {
        HealthReport { status: "up", atproto }
    }
}

use config::Config;
use health::{AtprotoStatus, HealthReport, health_report};

/// How long a health check waits for the atproto service before calling it unreachable.
const ATPROTO_PROBE_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone)]
pub struct AppState(Arc<Inner>);

pub struct Inner {
    pub config: Config,
    pub db: db::Db,
    pub http: net::Http,
    pub oauth: oauth::OAuthClient,
    pub resolver: identity::Resolver,
    probe: reqwest::Client,
}

impl Deref for AppState {
    type Target = Inner;

    fn deref(&self) -> &Inner {
        &self.0
    }
}

impl AppState {
    /// Connects the database and loads the signing key. Never touches atproto,
    /// so the server starts even when the network is down.
    pub async fn build(config: Config, public_url: String) -> Result<Self, String> {
        let db = db::connect(&config.database_url).await?;
        let key = oauth::signing_key(&db, config.signing_key.as_deref()).await?;
        let http = net::Http::new(config.allow_private_network);
        let oauth = oauth::OAuthClient::new(&public_url, &config.scopes, key, http.clone())?;
        let resolver = identity::Resolver {
            http: http.clone(),
            plc_url: config.plc_url.clone(),
            handle_resolver_url: config.handle_resolver_url.clone(),
        };
        let probe = reqwest::Client::builder()
            .timeout(ATPROTO_PROBE_TIMEOUT)
            .build()
            .expect("the HTTP client has a valid static configuration");
        Ok(Self(Arc::new(Inner { config, db, http, oauth, resolver, probe })))
    }

    /// Cookies are `Secure` and `__Host-` prefixed when the app is served over HTTPS.
    pub fn secure_cookies(&self) -> bool {
        self.oauth.public_url.starts_with("https://")
    }

    async fn atproto_status(&self) -> AtprotoStatus {
        let url = format!("{}/xrpc/_health", self.config.atproto_url);
        match self.probe.get(&url).send().await {
            Ok(res) if res.status().is_success() => AtprotoStatus::Reachable,
            Ok(res) => {
                eprintln!("atproto health probe: {url} answered {}", res.status());
                AtprotoStatus::Unreachable
            }
            Err(err) => {
                eprintln!("atproto health probe: {url} failed: {err}");
                AtprotoStatus::Unreachable
            }
        }
    }
}

/// The app: every route, inside the checks every route gets.
pub fn router(state: AppState) -> Router {
    secure(routes(), state)
}

/// Every route the server answers. A feature adds its routes here, and
/// `router` puts all of them behind the CSRF check.
fn routes() -> Router<AppState> {
    use auth::routes;
    Router::new()
        .route("/health", get(health))
        .route("/oauth-client-metadata.json", get(routes::client_metadata))
        .route("/oauth/jwks.json", get(routes::jwks))
        .route("/oauth/login", get(routes::login))
        .route("/oauth/signup", get(routes::signup))
        .route("/oauth/connect", get(routes::connect))
        .route("/oauth/callback", get(routes::callback))
        .route("/oauth/logout", post(routes::logout))
        .route("/xrpc/app.eventside.auth.getSession", get(routes::get_session))
        .route("/xrpc/app.eventside.conference.get", get(conference::api::get))
        .route("/xrpc/app.eventside.conference.join", post(conference::api::join))
        .route("/xrpc/app.eventside.conference.leave", post(conference::api::leave))
        .route("/xrpc/app.eventside.conference.getMembership", get(conference::api::get_membership))
        .route(
            "/xrpc/com.atproto.simplespace.checkUserAccess",
            get(conference::api::check_user_access),
        )
        .route("/.well-known/did.json", get(conference::api::did_document))
}

/// Wraps a finished set of routes in the CSRF check. `Router::layer` covers
/// only the routes that exist when it's called, so this runs last, on all of them.
fn secure(routes: Router<AppState>, state: AppState) -> Router {
    routes
        .layer(middleware::from_fn_with_state(state.clone(), auth::require_csrf))
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Json<HealthReport> {
    Json(health_report(state.atproto_status().await))
}

#[cfg(test)]
mod tests {
    use super::*;
    use auth::session::{self, NewSession};

    /// A directory removed when the test ends, pass or fail.
    struct TempDir(std::path::PathBuf);

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// App state on a database of its own, configured explicitly so the
    /// developer's environment can't change the outcome.
    async fn state() -> (AppState, TempDir) {
        state_signing_up_at("http://127.0.0.1:1").await
    }

    /// The same, with sign-up sent to the PDS at `signup_pds_url`.
    async fn state_signing_up_at(signup_pds_url: &str) -> (AppState, TempDir) {
        let dir =
            TempDir(std::env::temp_dir().join(format!("eventside-csrf-{}", keys::random_token(8))));
        let local = "http://127.0.0.1:1".to_owned();
        let config = Config {
            port: 0,
            atproto_url: local.clone(),
            public_url: None,
            database_url: format!("sqlite://{}/eventside.db?mode=rwc", dir.0.display()),
            signing_key: None,
            scopes: vec!["atproto".into()],
            signup_pds_url: signup_pds_url.to_owned(),
            plc_url: local.clone(),
            handle_resolver_url: local,
            allow_private_network: true,
            session_idle_timeout: std::time::Duration::from_secs(30 * 24 * 60 * 60),
            token_renew_interval: std::time::Duration::from_secs(5 * 60),
            token_refresh_skew: std::time::Duration::from_secs(60),
        };
        let state = AppState::build(config, "http://127.0.0.1:3100".into())
            .await
            .expect("the app state builds");
        (state, dir)
    }

    async fn post_to(app: Router, cookie: Option<&str>, csrf: Option<&str>) -> u16 {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let mut req =
            reqwest::Client::new().post(format!("http://{addr}/xrpc/test.changeSomething"));
        if let Some(cookie) = cookie {
            req = req.header("cookie", format!("session={cookie}"));
        }
        if let Some(csrf) = csrf {
            req = req.header("x-csrf-token", csrf);
        }
        req.send().await.unwrap().status().as_u16()
    }

    #[tokio::test]
    async fn instances_generating_the_signing_key_at_once_agree_on_it() {
        let dir =
            TempDir(std::env::temp_dir().join(format!("eventside-key-{}", keys::random_token(8))));
        let url = format!("sqlite://{}/eventside.db?mode=rwc", dir.0.display());
        let one = db::connect(&url).await.unwrap();
        let two = db::connect(&url).await.unwrap();
        let (a, b, c, d) = tokio::join!(
            oauth::signing_key(&one, None),
            oauth::signing_key(&two, None),
            oauth::signing_key(&one, None),
            oauth::signing_key(&two, None),
        );
        let a = a.unwrap();
        for other in [b.unwrap(), c.unwrap(), d.unwrap()] {
            assert_eq!(other.public_jwk(), a.public_jwk());
        }
        let rows: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM client_keys").fetch_one(&one).await.unwrap();
        assert_eq!(rows, 1);
    }

    #[tokio::test]
    async fn a_route_added_by_a_feature_gets_the_csrf_check() {
        let (state, _dir) = state().await;
        let cookie = session::create(
            &state.db,
            NewSession {
                did: "did:plc:test",
                handle: "test.example",
                display_name: None,
                avatar: None,
                pds: "http://127.0.0.1:1",
                dpop_key: "{}",
                issuer: "http://127.0.0.1:1",
                access_token: "access",
                refresh_token: None,
                token_expires_at: i64::MAX,
                scopes: "atproto",
                client_id: &state.oauth.client_id,
                kind: session::ATTENDEE,
            },
        )
        .await
        .unwrap();
        let row = session::load(&state.db, &keys::sha256_b64(&cookie)).await.unwrap().unwrap();
        let app = || {
            let feature =
                routes().route("/xrpc/test.changeSomething", post(|| async { "changed" }));
            secure(feature, state.clone())
        };
        assert_eq!(post_to(app(), None, None).await, 401);
        assert_eq!(post_to(app(), Some(&cookie), None).await, 403);
        assert_eq!(post_to(app(), Some(&cookie), Some("wrong")).await, 403);
        assert_eq!(post_to(app(), Some(&cookie), Some(&row.csrf_token)).await, 200);
    }

    async fn ended_session(state: &AppState, ended_at: i64) -> String {
        let cookie = session::create(
            &state.db,
            NewSession {
                did: "did:plc:test",
                handle: "test.example",
                display_name: None,
                avatar: None,
                pds: "http://127.0.0.1:1",
                dpop_key: "{}",
                issuer: "http://127.0.0.1:1",
                access_token: "access",
                refresh_token: None,
                token_expires_at: i64::MAX,
                scopes: "atproto",
                client_id: &state.oauth.client_id,
                kind: session::ATTENDEE,
            },
        )
        .await
        .unwrap();
        let id_hash = keys::sha256_b64(&cookie);
        sqlx::query("UPDATE sessions SET ended_at = $1, last_seen_at = $1 WHERE id_hash = $2")
            .bind(ended_at)
            .bind(&id_hash)
            .execute(&state.db)
            .await
            .unwrap();
        id_hash
    }

    #[tokio::test]
    async fn an_ended_session_is_kept_as_long_as_its_cookie_could_live() {
        let (state, _dir) = state().await;
        let day = 24 * 60 * 60 * 1000;
        let now = db::now_ms();
        // Ended 40 days ago: past the tombstone, but a cookie last renewed
        // then lives the 30-day idle timeout plus the tombstone.
        let kept = ended_session(&state, now - 40 * day).await;
        let gone = ended_session(&state, now - 61 * day).await;
        session::sweep(&state).await.unwrap();
        assert!(session::load(&state.db, &kept).await.unwrap().is_some());
        assert!(session::load(&state.db, &gone).await.unwrap().is_none());
    }

    /// A live session, as the given client ID, with the given scopes and tokens.
    async fn live_session(
        state: &AppState,
        issuer: &str,
        client_id: &str,
        scopes: &str,
    ) -> (String, String) {
        let dpop_key = keys::EcKey::generate().private_jwk();
        let cookie = session::create(
            &state.db,
            NewSession {
                did: "did:plc:test",
                handle: "test.example",
                display_name: None,
                avatar: None,
                pds: "http://127.0.0.1:1",
                dpop_key: &dpop_key,
                issuer,
                access_token: "access",
                refresh_token: Some("refresh-1"),
                token_expires_at: i64::MAX,
                scopes,
                client_id,
                kind: session::ATTENDEE,
            },
        )
        .await
        .unwrap();
        let id_hash = keys::sha256_b64(&cookie);
        (cookie, id_hash)
    }

    #[tokio::test]
    async fn a_grant_is_outdated_only_when_it_lacks_a_scope() {
        let (state, _dir) = state().await;
        let other = format!("{}?scope=atproto%20transition%3Ageneric", state.oauth.client_id);
        let (_, id_hash) =
            live_session(&state, "http://127.0.0.1:1", &other, "atproto transition:generic").await;
        let mut row = session::load(&state.db, &id_hash).await.unwrap().unwrap();
        // More scopes, and another client ID: still good.
        assert!(!session::outdated(&state, &row));
        row.scopes = "transition:generic".into();
        assert!(session::outdated(&state, &row));
    }

    /// A stand-in authorization server that records the client ID each token
    /// and revocation request was made as.
    async fn recording_auth_server() -> (String, Arc<std::sync::Mutex<Vec<(String, String)>>>) {
        let (issuer, seen, _) = recording_auth_server_with_tokens().await;
        (issuer, seen)
    }

    type Recorded = Arc<std::sync::Mutex<Vec<String>>>;

    /// The same, also recording each token revoked.
    async fn recording_auth_server_with_tokens()
    -> (String, Arc<std::sync::Mutex<Vec<(String, String)>>>, Recorded) {
        use axum::Form;
        use std::collections::HashMap;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let issuer = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let revoked: Recorded = Arc::default();
        let metadata = serde_json::json!({
            "issuer": issuer,
            "authorization_endpoint": format!("{issuer}/authorize"),
            "token_endpoint": format!("{issuer}/token"),
            "pushed_authorization_request_endpoint": format!("{issuer}/par"),
            "revocation_endpoint": format!("{issuer}/revoke"),
            "authorization_response_iss_parameter_supported": true,
            "require_pushed_authorization_requests": true,
            "client_id_metadata_document_supported": true,
            "token_endpoint_auth_methods_supported": ["none", "private_key_jwt"],
            "token_endpoint_auth_signing_alg_values_supported": ["ES256"],
            "dpop_signing_alg_values_supported": ["ES256"],
            "scopes_supported": ["atproto", "transition:generic"],
        });
        let record = |path: &'static str,
                      seen: Arc<std::sync::Mutex<Vec<(String, String)>>>,
                      revoked: Recorded| {
            move |Form(form): Form<HashMap<String, String>>| async move {
                let client_id = form.get("client_id").cloned().unwrap_or_default();
                if path == "revoke" {
                    revoked.lock().unwrap().push(form.get("token").cloned().unwrap_or_default());
                }
                seen.lock().unwrap().push((path.to_owned(), client_id));
                Json(serde_json::json!({
                    "access_token": "access-2",
                    "token_type": "DPoP",
                    "refresh_token": "refresh-2",
                    "expires_in": 3600,
                    "scope": "atproto transition:generic",
                    "sub": "did:plc:test",
                }))
            }
        };
        let app = Router::new()
            .route(
                "/.well-known/oauth-authorization-server",
                get(move || async move { Json(metadata) }),
            )
            .route("/token", post(record("token", seen.clone(), revoked.clone())))
            .route("/revoke", post(record("revoke", seen.clone(), revoked.clone())));
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (issuer, seen, revoked)
    }

    #[tokio::test]
    async fn a_grant_is_refreshed_and_revoked_as_the_client_it_was_issued_to() {
        let (state, _dir) = state().await;
        let (issuer, seen) = recording_auth_server().await;
        // Issued by an instance with another scope list, e.g. mid-deploy.
        let other = format!("{}?scope=atproto%20transition%3Ageneric", state.oauth.client_id);
        assert_ne!(other, state.oauth.client_id);
        let (_, id_hash) =
            live_session(&state, &issuer, &other, "atproto transition:generic").await;
        assert!(matches!(
            auth::renew::refresh(&state, &id_hash).await,
            auth::renew::Renewal::Renewed
        ));
        let row = session::load(&state.db, &id_hash).await.unwrap().unwrap();
        assert_eq!(row.refresh_token.as_deref(), Some("refresh-2"));
        assert_eq!(row.client_id, other);
        session::revoke(&state, &row).await;
        let seen = seen.lock().unwrap().clone();
        assert_eq!(seen, vec![("token".to_owned(), other.clone()), ("revoke".to_owned(), other)]);
    }

    #[tokio::test]
    async fn a_code_is_redeemed_as_the_client_its_request_was_pushed_as() {
        let (state, _dir) = state().await;
        let (issuer, seen) = recording_auth_server().await;
        // Pushed by an instance with another scope list, e.g. mid-deploy.
        let other = format!("{}?scope=atproto%20transition%3Ageneric", state.oauth.client_id);
        sqlx::query(
            "INSERT INTO oauth_requests (state, kind, client_id, pkce_verifier, dpop_key, issuer, \
             expected_did, preauth_hash, return_to, expires_at) \
             VALUES ($1, 'login', $2, 'verifier', $3, $4, NULL, $5, '/', $6)",
        )
        .bind("teststate")
        .bind(&other)
        .bind(keys::EcKey::generate().private_jwk())
        .bind(&issuer)
        .bind(keys::sha256_b64("preauth"))
        .bind(db::now_ms() + 60_000)
        .execute(&state.db)
        .await
        .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = router(state.clone());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let res = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap()
            .get(format!("http://{addr}/oauth/callback"))
            .query(&[("state", "teststate"), ("iss", issuer.as_str()), ("code", "code")])
            .header("cookie", "oauth_preauth_teststate=preauth")
            .send()
            .await
            .unwrap();
        assert_eq!(res.status().as_u16(), 302);
        // The DID can't be resolved here, so the unused grant is revoked:
        // as the same client.
        let seen = seen.lock().unwrap().clone();
        assert_eq!(seen, vec![("token".to_owned(), other.clone()), ("revoke".to_owned(), other)]);
    }

    #[tokio::test]
    async fn signing_out_a_session_that_is_gone_clears_its_cookie() {
        let (state, _dir) = state().await;
        let client_id = state.oauth.client_id.clone();
        let (live, _) = live_session(&state, "http://127.0.0.1:1", &client_id, "atproto").await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = router(state.clone());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let logout = |cookie: String| async move {
            reqwest::Client::new()
                .post(format!("http://{addr}/oauth/logout"))
                .header("cookie", format!("session={cookie}"))
                .send()
                .await
                .unwrap()
        };
        let res = logout("no-such-session".into()).await;
        assert_eq!(res.status().as_u16(), 200);
        let set = res.headers().get("set-cookie").and_then(|v| v.to_str().ok()).unwrap_or("");
        assert!(set.starts_with("session=;") && set.contains("Max-Age=0"), "{set:?}");
        // A live session still needs its CSRF token.
        assert_eq!(logout(live.clone()).await.status().as_u16(), 403);
        // A cross-site form POST carries no cookie (it's `SameSite=Lax`), so
        // the cookie it can't see isn't cleared either.
        let res = reqwest::Client::new()
            .post(format!("http://{addr}/oauth/logout"))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status().as_u16(), 200);
        assert!(res.headers().get("set-cookie").is_none(), "{:?}", res.headers());
        assert!(matches!(
            session::lookup(&state, &{
                let mut h = axum::http::HeaderMap::new();
                h.insert("cookie", format!("session={live}").parse().unwrap());
                h
            })
            .await
            .unwrap(),
            session::Lookup::Live(_)
        ));
    }

    #[tokio::test]
    async fn only_the_request_that_ends_a_session_revokes_it() {
        let (state, _dir) = state().await;
        let (issuer, seen) = recording_auth_server().await;
        let client_id = state.oauth.client_id.clone();
        let (_, id_hash) = live_session(&state, &issuer, &client_id, "atproto").await;
        let row = session::load(&state.db, &id_hash).await.unwrap().unwrap();
        // Two requests that both found it idle.
        session::end(&state, &row).await.unwrap();
        session::end(&state, &row).await.unwrap();
        for _ in 0..50 {
            if !seen.lock().unwrap().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(seen.lock().unwrap().clone(), vec![("revoke".to_owned(), client_id)]);
    }

    #[tokio::test]
    async fn ending_a_session_already_ended_does_not_revoke_it_again() {
        let (state, _dir) = state().await;
        let (issuer, seen) = recording_auth_server().await;
        let client_id = state.oauth.client_id.clone();
        let (_, id_hash) = live_session(&state, &issuer, &client_id, "atproto").await;
        let row = session::load(&state.db, &id_hash).await.unwrap().unwrap();
        // The renewer and a request that found it idle, in either order.
        session::end_revoking(&state, &row).await.unwrap();
        session::end(&state, &row).await.unwrap();
        session::end_revoking(&state, &row).await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(seen.lock().unwrap().clone(), vec![("revoke".to_owned(), client_id)]);
    }

    #[tokio::test]
    async fn get_session_renews_the_cookie_on_every_answer() {
        let (state, _dir) = state().await;
        let client_id = state.oauth.client_id.clone();
        // Just created, so `last_seen_at` isn't due to move.
        let (cookie, _) = live_session(&state, "http://127.0.0.1:1", &client_id, "atproto").await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = router(state.clone());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        for _ in 0..2 {
            let res = reqwest::Client::new()
                .get(format!("http://{addr}/xrpc/app.eventside.auth.getSession"))
                .header("cookie", format!("session={cookie}"))
                .send()
                .await
                .unwrap();
            assert_eq!(res.status().as_u16(), 200);
            let set = res.headers().get("set-cookie").and_then(|v| v.to_str().ok()).unwrap_or("");
            assert!(set.starts_with(&format!("session={cookie};")), "{set:?}");
        }
    }

    #[tokio::test]
    async fn ending_a_session_revokes_the_tokens_it_wiped_not_the_ones_read() {
        let (state, _dir) = state().await;
        let (issuer, _, revoked) = recording_auth_server_with_tokens().await;
        let client_id = state.oauth.client_id.clone();
        for end_revoking in [false, true] {
            let (_, id_hash) = live_session(&state, &issuer, &client_id, "atproto").await;
            // Read with refresh-1, then a renewal saves refresh-2 before the
            // session is ended.
            let stale = session::load(&state.db, &id_hash).await.unwrap().unwrap();
            sqlx::query("UPDATE sessions SET refresh_token = 'refresh-2' WHERE id_hash = $1")
                .bind(&id_hash)
                .execute(&state.db)
                .await
                .unwrap();
            if end_revoking {
                session::end_revoking(&state, &stale).await.unwrap();
            } else {
                session::end(&state, &stale).await.unwrap();
            }
            for _ in 0..50 {
                if !revoked.lock().unwrap().is_empty() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            assert_eq!(revoked.lock().unwrap().drain(..).collect::<Vec<_>>(), vec!["refresh-2"]);
        }
    }

    #[tokio::test]
    async fn deleting_a_session_returns_the_tokens_it_deleted() {
        let (state, _dir) = state().await;
        let client_id = state.oauth.client_id.clone();
        let (_, id_hash) = live_session(&state, "http://127.0.0.1:1", &client_id, "atproto").await;
        sqlx::query("UPDATE sessions SET refresh_token = 'refresh-2' WHERE id_hash = $1")
            .bind(&id_hash)
            .execute(&state.db)
            .await
            .unwrap();
        let deleted = session::delete(&state.db, &id_hash).await.unwrap().unwrap();
        assert_eq!(deleted.refresh_token.as_deref(), Some("refresh-2"));
        assert!(session::delete(&state.db, &id_hash).await.unwrap().is_none());
    }

    /// A PDS whose authorization server (at the same origin) serves `metadata`.
    async fn pds_with_auth_server(metadata: impl Fn(&str) -> serde_json::Value) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let resource = serde_json::json!({ "resource": origin, "authorization_servers": [origin] });
        let metadata = metadata(&origin);
        let app = Router::new()
            .route(
                "/.well-known/oauth-protected-resource",
                get(move || async move { Json(resource) }),
            )
            .route(
                "/.well-known/oauth-authorization-server",
                get(move || async move { Json(metadata) }),
            );
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        origin
    }

    /// Where a GET to one of the app's routes redirects.
    async fn redirect_from(state: &AppState, path_and_query: &str, cookie: Option<&str>) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = router(state.clone());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let mut req = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap()
            .get(format!("http://{addr}{path_and_query}"));
        if let Some(cookie) = cookie {
            req = req.header("cookie", cookie);
        }
        let res = req.send().await.unwrap();
        assert_eq!(res.status().as_u16(), 302);
        res.headers()["location"].to_str().unwrap().to_owned()
    }

    #[tokio::test]
    async fn an_authorization_server_without_atproto_capabilities_is_refused() {
        let pds = pds_with_auth_server(|origin| {
            serde_json::json!({
                "issuer": origin,
                "authorization_endpoint": format!("{origin}/authorize"),
                "token_endpoint": format!("{origin}/token"),
                "pushed_authorization_request_endpoint": format!("{origin}/par"),
                "authorization_response_iss_parameter_supported": true,
                "require_pushed_authorization_requests": true,
                "client_id_metadata_document_supported": true,
                // No private_key_jwt.
                "token_endpoint_auth_methods_supported": ["none"],
                "token_endpoint_auth_signing_alg_values_supported": ["ES256"],
                "dpop_signing_alg_values_supported": ["ES256"],
                "scopes_supported": ["atproto"],
            })
        })
        .await;
        let (state, _dir) = state_signing_up_at(&pds).await;
        let location = redirect_from(&state, "/oauth/signup?return_to=/here", None).await;
        assert_eq!(location, "/signin?error=server_unsupported&return_to=%2Fhere");
    }

    #[test]
    fn every_capability_atproto_requires_is_checked() {
        let full = serde_json::json!({
            "issuer": "https://as.example",
            "authorization_endpoint": "https://as.example/authorize",
            "token_endpoint": "https://as.example/token",
            "pushed_authorization_request_endpoint": "https://as.example/par",
            "authorization_response_iss_parameter_supported": true,
            "require_pushed_authorization_requests": true,
            "client_id_metadata_document_supported": true,
            "token_endpoint_auth_methods_supported": ["private_key_jwt"],
            "token_endpoint_auth_signing_alg_values_supported": ["ES256"],
            "dpop_signing_alg_values_supported": ["ES256"],
            "scopes_supported": ["atproto"],
        });
        let server: oauth::AuthServer = serde_json::from_value(full.clone()).unwrap();
        assert_eq!(server.missing_capability(), None);
        for (field, without) in [
            ("authorization_response_iss_parameter_supported", serde_json::json!(false)),
            ("require_pushed_authorization_requests", serde_json::json!(false)),
            ("client_id_metadata_document_supported", serde_json::json!(false)),
            ("token_endpoint_auth_methods_supported", serde_json::json!(["none"])),
            ("token_endpoint_auth_signing_alg_values_supported", serde_json::json!(["RS256"])),
            ("dpop_signing_alg_values_supported", serde_json::json!(["ES256K"])),
            ("scopes_supported", serde_json::json!(["transition:generic"])),
        ] {
            for value in [Some(without), None] {
                let mut metadata = full.clone();
                match value {
                    Some(value) => metadata[field] = value,
                    None => {
                        metadata.as_object_mut().unwrap().remove(field);
                    }
                }
                let server: oauth::AuthServer = serde_json::from_value(metadata).unwrap();
                let missing = server.missing_capability();
                assert!(missing.is_some_and(|m| m.contains(field)), "{field}: {missing:?}");
            }
        }
    }

    async fn pending_request(state: &AppState, request_state: &str, expires_at: i64) {
        sqlx::query(
            "INSERT INTO oauth_requests (state, kind, client_id, pkce_verifier, dpop_key, issuer, \
             expected_did, preauth_hash, return_to, expires_at) \
             VALUES ($1, 'login', $2, 'verifier', '{}', 'http://127.0.0.1:1', NULL, $3, '/here', $4)",
        )
        .bind(request_state)
        .bind(&state.oauth.client_id)
        .bind(keys::sha256_b64("preauth"))
        .bind(expires_at)
        .execute(&state.db)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn a_late_callback_is_reported_as_expired() {
        let (state, _dir) = state().await;
        pending_request(&state, "late", db::now_ms() - 1).await;
        // The pre-auth cookie ran out with the request, so none comes back.
        let location = redirect_from(&state, "/oauth/callback?state=late&code=c", None).await;
        assert_eq!(location, "/signin?error=request_expired&return_to=%2Fhere");
        // It was consumed: trying again finds nothing.
        let location = redirect_from(&state, "/oauth/callback?state=late&code=c", None).await;
        assert!(location.starts_with("/signin?error=invalid_request"), "{location}");
    }

    #[tokio::test]
    async fn a_callback_that_cannot_reach_the_database_is_a_server_error() {
        let (state, _dir) = state().await;
        sqlx::query("DROP TABLE oauth_requests").execute(&state.db).await.unwrap();
        let location = redirect_from(&state, "/oauth/callback?state=any&code=c", None).await;
        assert!(location.starts_with("/signin?error=server_error"), "{location}");
    }
}
