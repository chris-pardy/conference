use std::io::Write;

use conference_server::{AppState, router};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("PORT")
        .ok()
        .map(|p| p.parse().expect("PORT must be a port number"))
        .unwrap_or(3000);
    let atproto_url =
        std::env::var("ATPROTO_URL").unwrap_or_else(|_| "http://localhost:2580".into());

    let listener =
        TcpListener::bind(("127.0.0.1", port)).await.expect("failed to bind the listening port");
    let addr = listener.local_addr().expect("a bound listener has an address");

    // Tests and tooling wait for this exact line to learn the port.
    let mut stdout = std::io::stdout();
    writeln!(stdout, "listening on http://{addr}").ok();
    stdout.flush().ok();

    axum::serve(listener, router(AppState::new(atproto_url))).await.expect("server error");
}
