use std::io::Write;
use std::process::ExitCode;

use conference_server::config::Config;
use conference_server::{AppState, auth, conference, router};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> ExitCode {
    // `conference-server admin …`: the operator's CLI (conference-space).
    if std::env::args().nth(1).as_deref() == Some("admin") {
        let args: Vec<String> = std::env::args().skip(2).collect();
        return conference_server::conference::cli::main(args).await;
    }
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("conference-server: {err}");
            return ExitCode::from(2);
        }
    };

    let listener = TcpListener::bind(("127.0.0.1", config.port))
        .await
        .expect("failed to bind the listening port");
    let addr = listener.local_addr().expect("a bound listener has an address");
    let public_url = config.public_url.clone().unwrap_or_else(|| format!("http://{addr}"));

    let state = match AppState::build(config, public_url).await {
        Ok(state) => state,
        Err(err) => {
            eprintln!("conference-server: {err}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(err) = conference::attest::ensure_key(&state.db).await {
        eprintln!("conference-server: {err}");
        return ExitCode::FAILURE;
    }
    auth::renew::spawn(state.clone());
    // Writes what decisions left for the organizations' repos, starting with
    // anything an earlier run (or a stopped CLI) didn't get to.
    conference::outbox::spawn(state.clone());
    // Keeps the conferences' records index current.
    conference::sync::spawn(state.clone());

    // Tests and tooling wait for this exact line to learn the port.
    let mut stdout = std::io::stdout();
    writeln!(stdout, "listening on http://{addr}").ok();
    stdout.flush().ok();

    axum::serve(listener, router(state)).await.expect("server error");
    ExitCode::SUCCESS
}
