// Written by Kone Tshivhinda & Claude
//! The vault service binary.
//!
//! ```text
//! timelex-vault-service            # serve (default)
//! timelex-vault-service migrate    # run the vault migrations, then exit
//! ```
//!
//! Configuration is from the environment; secrets never live in files in the repository:
//! * `TIMELEX_VAULT_DATABASE_URL`: connection URL for the `svc_vault` role (serve).
//! * `TIMELEX_MIGRATOR_DATABASE_URL`: connection URL for the `timelex_migrator` role (migrate).
//! * `TIMELEX_GATEWAY_PUBLIC_KEYS`: `kid=base64,...` public keys the gateway signs with (serve).
//! * `TIMELEX_VAULT_MASTER_KEY`: base64 of exactly 32 bytes, the key-encryption key (serve). Until a
//!   KMS adapter lands (ADR-0004), this is the only thing that can unwrap a stored data key. Never
//!   printed, never logged; rotate it only with a re-wrap migration.
//! * `TIMELEX_VAULT_ADDR`: listen address, default `127.0.0.1:8082`. Keep it private: only the
//!   gateway should be able to reach this service (ADR-0012).
//! * `TIMELEX_OUTBOX_RELAY`: `0` turns off the in-process outbox relay; on by default.
//! * `TIMELEX_OUTBOX_INTERVAL_MS`: how often the relay polls, default 500.

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use timelex_auth::Verifier;
use timelex_db::TenantPool;
use timelex_outbox::{JsonLinePublisher, Relay};
use timelex_vault::LocalKeyWrapper;
use timelex_vault_service::{http, LiveValidator, VaultStore, MIGRATIONS};
use zeroize::Zeroizing;

fn env(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|_| format!("{name} is not set"))
}

/// Decode the master key. The errors name the variable and never the value it holds.
fn master_key() -> Result<[u8; 32], String> {
    let raw = env("TIMELEX_VAULT_MASTER_KEY")?;
    let bytes = Zeroizing::new(
        STANDARD
            .decode(raw.trim())
            .map_err(|_| "TIMELEX_VAULT_MASTER_KEY must be base64".to_string())?,
    );
    <[u8; 32]>::try_from(bytes.as_slice()).map_err(|_| {
        format!(
            "TIMELEX_VAULT_MASTER_KEY must decode to 32 bytes, not {}",
            bytes.len()
        )
    })
}

async fn migrate() -> Result<(), String> {
    let options = PgConnectOptions::from_str(&env("TIMELEX_MIGRATOR_DATABASE_URL")?)
        .map_err(|e| format!("TIMELEX_MIGRATOR_DATABASE_URL is not a valid URL: {e}"))?
        // sqlx keeps its bookkeeping table in the first schema of the search path.
        .options([("search_path", "vault")]);
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(|e| format!("cannot connect: {e}"))?;
    MIGRATIONS
        .run(&pool)
        .await
        .map_err(|e| format!("migration failed: {e}"))?;
    println!("vault migrations are up to date");
    Ok(())
}

async fn serve() -> Result<(), String> {
    let options = PgConnectOptions::from_str(&env("TIMELEX_VAULT_DATABASE_URL")?)
        .map_err(|e| format!("TIMELEX_VAULT_DATABASE_URL is not a valid URL: {e}"))?;
    let pool = TenantPool::connect(options.clone(), 10)
        .await
        .map_err(|e| format!("cannot connect to the database: {e}"))?;
    let verifier =
        Verifier::from_config("vault", &env("TIMELEX_GATEWAY_PUBLIC_KEYS")?).map_err(|e| e.to_string())?;
    let wrapper = Arc::new(LocalKeyWrapper::new(master_key()?));
    let validator = Arc::new(LiveValidator::new()?);
    if outbox_relay_enabled() {
        let relay_pool = PgPoolOptions::new()
            .max_connections(2)
            .connect_with(options)
            .await
            .map_err(|e| format!("cannot connect the outbox relay: {e}"))?;
        let relay = Arc::new(Relay::new(relay_pool, JsonLinePublisher::stdout()));
        let interval = relay_interval();
        tokio::spawn(async move { relay.run(interval).await });
        println!("timelex-vault-service relaying the outbox");
    }
    let addr = std::env::var("TIMELEX_VAULT_ADDR").unwrap_or_else(|_| "127.0.0.1:8082".into());
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("cannot listen on {addr}: {e}"))?;
    println!("timelex-vault-service listening on http://{addr}");
    axum::serve(
        listener,
        http::router(VaultStore::new(pool, wrapper), verifier, validator),
    )
    .with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await
    .map_err(|e| e.to_string())
}

/// The relay runs by default; `TIMELEX_OUTBOX_RELAY=0` turns it off (for an operator who runs one
/// relay elsewhere).
fn outbox_relay_enabled() -> bool {
    std::env::var("TIMELEX_OUTBOX_RELAY").map_or(true, |v| v != "0")
}

/// Poll interval, with a floor so a misread value cannot spin the process.
fn relay_interval() -> Duration {
    let millis = std::env::var("TIMELEX_OUTBOX_INTERVAL_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(500);
    Duration::from_millis(millis.max(10))
}

#[tokio::main]
async fn main() {
    let result = match std::env::args().nth(1).as_deref() {
        None | Some("serve") => serve().await,
        Some("migrate") => migrate().await,
        Some(other) => Err(format!("unknown command {other:?}; use `serve` or `migrate`")),
    };
    if let Err(message) = result {
        eprintln!("error: {message}");
        std::process::exit(1);
    }
}
