// Written by Kone Tshivhinda & Claude
//! The gateway binary.
//!
//! ```text
//! timelex-gateway           # serve (needs the environment described in `config`)
//! timelex-gateway keygen    # print a fresh Ed25519 key pair for local development, then exit
//! ```

use std::net::SocketAddr;

use timelex_gateway::config::Config;

fn keygen() -> Result<(), String> {
    let kp = timelex_auth::generate_keypair().map_err(|e| e.to_string())?;
    // The private key is printed once, to your terminal. Put it in your untracked environment; never commit it.
    println!("# Gateway (private key: keep secret)");
    println!("TIMELEX_GATEWAY_KEY_ID=k1");
    println!("TIMELEX_GATEWAY_SIGNING_KEY={}", kp.private_pkcs8_base64);
    println!("# Services (public key: safe to share with every service)");
    println!("TIMELEX_GATEWAY_PUBLIC_KEYS=k1={}", kp.public_base64);
    Ok(())
}

async fn serve() -> Result<(), String> {
    let addr: SocketAddr = std::env::var("TIMELEX_GATEWAY_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8088".to_string())
        .parse()
        .map_err(|_| "TIMELEX_GATEWAY_ADDR must be host:port".to_string())?;
    let app = timelex_gateway::app_with(Config::from_env()?)?;
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| format!("cannot listen on {addr}: {e}"))?;
    println!("timelex-gateway (pre-alpha) listening on http://{addr}");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(|e| e.to_string())
}

#[tokio::main]
async fn main() {
    let result = match std::env::args().nth(1).as_deref() {
        None | Some("serve") => serve().await,
        Some("keygen") => keygen(),
        Some(other) => Err(format!("unknown command {other:?}; use `serve` or `keygen`")),
    };
    if let Err(message) = result {
        eprintln!("error: {message}");
        std::process::exit(1);
    }
}
