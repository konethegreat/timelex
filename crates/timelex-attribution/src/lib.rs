// Written by Kone Tshivhinda & Claude
//! Deterministic attribution stages 0 and 1 for TimeLex.
//!
//! Stage 0 resolves exact hard keys: a matter reference in a subject, filename or thread, or a
//! participant domain that names one client. Stage 1 remembers assignments from earlier groups by
//! thread and document, and remembers the client behind a participant set. Neither stage calls a
//! model, reads a clock, or touches the network; both abstain or ask for review when the evidence
//! does not name one matter (ADR-0028).
//!
//! A caller passes the events segmentation grouped together, so the stages see the same evidence a
//! reviewer would. The crate owns no ids, storage or events: those belong to the attribution service
//! that will wrap it.

mod directory;
mod resolve;

pub use directory::{
    AliasKind, AliasRecord, ClientRecord, Directory, DirectoryError, MatterRecord, MatterStatus,
};
pub use resolve::{attribute, Decision, Evidence, History, ReviewReason};
