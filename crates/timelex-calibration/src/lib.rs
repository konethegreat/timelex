// Written by Kone Tshivhinda & Claude
//! Per-tenant score calibration, threshold derivation and reliability metrics.
//!
//! A raw score from an attribution stage is not a probability (ADR-0029). This crate turns
//! (raw score, outcome) observations into a monotone calibrated probability, measures how far
//! the probabilities sit from observed outcomes, and derives the lowest score that keeps
//! measured precision at or above a floor. The autonomy ladder compares a decision's
//! confidence with that derived threshold (ADR-0005, applied to the stages of ADR-0028).
//!
//! Everything here is deterministic: no clock, no network, no model.

mod calibrator;
mod metrics;
mod threshold;

pub use calibrator::{CalibrationError, Calibrator, Observation, Point};
pub use metrics::{diagram, reliability, Reliability, ReliabilityBin, BINS};
pub use threshold::{derive_threshold, Threshold};
