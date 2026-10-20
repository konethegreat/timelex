// Written by Kone Tshivhinda & Claude
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let addr: SocketAddr = std::env::var("TIMELEX_GATEWAY_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8088".to_string())
        .parse()
        .expect("TIMELEX_GATEWAY_ADDR must be host:port");
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind failed");
    println!("timelex-gateway (pre-alpha) listening on http://{addr}");
    axum::serve(listener, timelex_gateway::app())
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .expect("server error");
}
