// Written by Kone Tshivhinda & Claude
//! The attribution service.
//!
//! The deterministic stages live in [`timelex_attribution`]; this crate carries the stages that
//! need a model and the failure report those stages pass forward. It is a library for now: the
//! HTTP surface and the `attribution` schema arrive with the service, not with this increment.

pub mod model_stages;
