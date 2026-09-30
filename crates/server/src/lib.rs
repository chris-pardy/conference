use std::time::Duration;

use axum::{Json, Router, extract::State, routing::get};

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

use health::{AtprotoStatus, HealthReport, health_report};

/// How long a health check waits for the atproto service before calling it unreachable.
const ATPROTO_PROBE_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone)]
pub struct AppState {
    atproto_url: String,
    http: reqwest::Client,
}

impl AppState {
    pub fn new(atproto_url: impl Into<String>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(ATPROTO_PROBE_TIMEOUT)
            .build()
            .expect("the HTTP client has a valid static configuration");
        Self { atproto_url: atproto_url.into().trim_end_matches('/').to_owned(), http }
    }

    async fn atproto_status(&self) -> AtprotoStatus {
        let url = format!("{}/xrpc/_health", self.atproto_url);
        match self.http.get(&url).send().await {
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
    Router::new().route("/health", get(health)).with_state(state)
}

async fn health(State(state): State<AppState>) -> Json<HealthReport> {
    Json(health_report(state.atproto_status().await))
}
