// Written by Kone Tshivhinda & Claude
//! TimeLex capture-hub service (library first).
//!
//! Raw signals arrive from a connector or from the desktop agent and leave as immutable activity
//! events ([ADR-0006](../../docs/adr/0006-event-sourced-activity-and-ledger.md)). Two rules shape
//! this service:
//!
//! * **An event is written once.** Its id is derived from the signal
//!   (`contracts/events/README.md`), so delivering the same signal twice finds the existing row and
//!   publishes nothing new (ADR-0021).
//! * **A cursor moves only after its page is stored.** The store writes a page's events and the new
//!   cursor in one transaction, so a crash cannot skip events.
//!
//! The tables are in `services/capture-hub/migrations`; the HTTP contract is
//! `contracts/openapi/capture.v1.yaml`. Connectors for real providers arrive in P2-03 and later; this
//! increment provides the framework they plug into.

mod connector;
mod error;
pub mod exclusions;
pub mod http;
pub mod manual;
mod store;

pub use connector::{Connector, ConnectorError, Page, RunReport, Runner, RunnerError};
pub use error::StoreError;
pub use exclusions::{Exclusion, ExclusionKind};
pub use store::{Actor, CaptureStore, Clock, IngestOutcome, Ingested, PageOutcome};

/// The capture schema's migrations, embedded at build time. Run them as `timelex_migrator` with
/// `search_path = capture` (see `deploy/postgres/README.md`).
pub static MIGRATIONS: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
