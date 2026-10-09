//! What the conference tests share: app state on a database of its own,
//! and a conference in it.

use super::Conference;
use crate::AppState;
use crate::config::Config;

/// A directory removed when the test ends, pass or fail.
pub struct TempDir(std::path::PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub async fn state() -> (AppState, TempDir) {
    let dir = TempDir(
        std::env::temp_dir().join(format!("eventside-conference-{}", crate::keys::random_token(8))),
    );
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
        session_idle_timeout: std::time::Duration::from_secs(60),
        token_renew_interval: std::time::Duration::from_secs(60),
        token_refresh_skew: std::time::Duration::from_secs(60),
    };
    let state = AppState::build(config, "http://127.0.0.1:3100".into()).await.unwrap();
    (state, dir)
}

/// A conference taking the given join methods.
pub async fn conference(state: &AppState, methods: &str) -> Conference {
    let space = super::SpaceUri::conference("did:plc:aaaaaaaaaaaaaaaaaaaaaaaa", "3kx");
    sqlx::query(
        "INSERT INTO conferences (space, org, rkey, event, name, starts_at, ends_at, city, theme, \
         methods, created_by, created_at) VALUES ($1, 'did:plc:aaaaaaaaaaaaaaaaaaaaaaaa', '3kx', \
         'at://x/y/3kx', 'Conf', '2027-04-29T07:00:00.000Z', '2027-05-02T16:00:00.000Z', \
         'Amsterdam', '{}', $2, 'did:plc:aaaaaaaaaaaaaaaaaaaaaaaa', 0)",
    )
    .bind(&space)
    .bind(methods)
    .execute(&state.db)
    .await
    .unwrap();
    super::load(&state.db, &space).await.unwrap().unwrap()
}
