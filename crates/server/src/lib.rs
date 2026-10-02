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

pub fn router(state: AppState) -> Router {
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
        .layer(middleware::from_fn_with_state(state.clone(), auth::require_csrf))
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Json<HealthReport> {
    Json(health_report(state.atproto_status().await))
}
