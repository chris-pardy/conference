use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;

use axum::routing::{get, post};
use axum::{Json, Router, extract::State, middleware};

pub mod auth;
pub mod config;
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
        let oauth = oauth::OAuthClient::new(&public_url, &config.scopes, key, http.clone());
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
        .route("/oauth/callback", get(routes::callback))
        .route("/oauth/logout", post(routes::logout))
        .route("/xrpc/app.eventside.auth.getSession", get(routes::get_session))
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
            signup_pds_url: local.clone(),
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

    #[tokio::test]
    async fn a_grant_for_another_client_id_is_outdated() {
        let (state, _dir) = state().await;
        let id_hash = ended_session(&state, db::now_ms()).await;
        let mut row = session::load(&state.db, &id_hash).await.unwrap().unwrap();
        assert!(!session::outdated(&state, &row));
        row.client_id = None;
        assert!(!session::outdated(&state, &row));
        // Same scopes, reordered or shrunk: still another client ID.
        row.client_id = Some(format!("{}?scope=atproto", state.oauth.client_id));
        assert!(session::outdated(&state, &row));
    }
}
