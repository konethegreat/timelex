// Written by Kone Tshivhinda & Claude
//! The matters service binary.
//!
//! ```text
//! timelex-matters-service            # serve (default)
//! timelex-matters-service migrate    # run the matters migrations, then exit
//! ```
//!
//! Configuration is from the environment; secrets never live in files in the repository:
//! * `TIMELEX_MATTERS_DATABASE_URL`: connection URL for the `svc_matters` role (serve).
//! * `TIMELEX_MIGRATOR_DATABASE_URL`: connection URL for the `timelex_migrator` role (migrate).
//! * `TIMELEX_GATEWAY_PUBLIC_KEYS`: `kid=base64,...` public keys the gateway signs with (serve).
//! * `TIMELEX_MATTERS_ADDR`: listen address, default `127.0.0.1:8083`. Keep it private: only the
//!   gateway should be able to reach this service (ADR-0012). Change the default when the port is
//!   already taken.

use std::str::FromStr;

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use timelex_auth::Verifier;
use timelex_db::TenantPool;
use timelex_matters_service::{http, Store, MIGRATIONS};

fn env(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|_| format!("{name} is not set"))
}

async fn migrate() -> Result<(), String> {
    let options = PgConnectOptions::from_str(&env("TIMELEX_MIGRATOR_DATABASE_URL")?)
        .map_err(|e| format!("TIMELEX_MIGRATOR_DATABASE_URL is not a valid URL: {e}"))?
        // sqlx keeps its bookkeeping table in the first schema of the search path.
        .options([("search_path", "matters")]);
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(|e| format!("cannot connect: {e}"))?;
    MIGRATIONS
        .run(&pool)
        .await
        .map_err(|e| format!("migration failed: {e}"))?;
    println!("matters migrations are up to date");
    Ok(())
}

async fn serve() -> Result<(), String> {
    let _telemetry = timelex_telemetry::init("matters").map_err(|e| e.to_string())?;
    let options = PgConnectOptions::from_str(&env("TIMELEX_MATTERS_DATABASE_URL")?)
        .map_err(|e| format!("TIMELEX_MATTERS_DATABASE_URL is not a valid URL: {e}"))?;
    let pool = TenantPool::connect(options, 10)
        .await
        .map_err(|e| format!("cannot connect to the database: {e}"))?;
    let verifier =
        Verifier::from_config("matters", &env("TIMELEX_GATEWAY_PUBLIC_KEYS")?).map_err(|e| e.to_string())?;
    let addr = std::env::var("TIMELEX_MATTERS_ADDR").unwrap_or_else(|_| "127.0.0.1:8083".into());
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("cannot listen on {addr}: {e}"))?;
    println!("timelex-matters-service listening on http://{addr}");
    axum::serve(listener, http::router(Store::new(pool), verifier))
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
        Some("migrate") => migrate().await,
        Some(other) => Err(format!("unknown command {other:?}; use `serve` or `migrate`")),
    };
    if let Err(message) = result {
        eprintln!("error: {message}");
        std::process::exit(1);
    }
}
