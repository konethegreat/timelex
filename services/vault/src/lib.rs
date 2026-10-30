// Written by Kone Tshivhinda & Claude
//! TimeLex vault service (library first).
//!
//! Custody of each firm's own provider keys (ADR-0004). The rules are in `timelex-vault`; the tables
//! are in `services/vault/migrations`; the contract is `contracts/openapi/vault.v1.yaml`. This
//! library holds the store, the HTTP layer and the provider check.

mod error;
pub mod http;
mod store;
pub mod validator;

pub use error::StoreError;
pub use store::{
    Actor, Clock, LoadedSecret, PutOutcome, PutSecretRequest, SecretMetadata, TestOutcome, VaultStore,
};
pub use validator::{CheckResult, LiveValidator, ProviderValidator};

/// The vault schema's migrations, embedded at build time. Run them as `timelex_migrator` with
/// `search_path = vault` (see `deploy/postgres/README.md`).
pub static MIGRATIONS: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
