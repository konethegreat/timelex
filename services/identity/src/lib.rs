// Written by Kone Tshivhinda & Claude
//! TimeLex identity service (library first).
//!
//! Operations: create a firm, issue an API key, revoke an API key, authenticate a presented key.
//! The contract is `contracts/openapi/identity.v1.yaml`; the rules are in `timelex-identity`; the
//! tables are in `services/identity/migrations`; the design is ADR-0011. The HTTP layer is in [`http`].

mod error;
pub mod http;
mod store;

pub use error::{AuthError, StoreError};
pub use store::{Actor, Clock, CreatedTenant, IdentityStore, IssueKeyRequest, IssuedKey, Principal};

/// The identity schema's migrations, embedded at build time. Run them as `timelex_migrator` with
/// `search_path = identity` (see `deploy/postgres/README.md`).
pub static MIGRATIONS: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
