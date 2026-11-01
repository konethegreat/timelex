// Written by Kone Tshivhinda & Claude
//! The `mcp-host` model loop: one provider trait, integer Rand accounting, and an escalation
//! ladder that degrades safely.
//!
//! A run takes a [`RunInput`], walks the tenant's [`Ladder`], and ends in exactly one of two
//! states: [`Terminal::Done`] with an accepted output, or [`Terminal::NeedsReview`] with a reason.
//! A model this key cannot use is skipped; a rejected key, an unreachable provider or a spent
//! budget stops the run; escalation past the last rung is a human decision.
//!
//! Costs are integer micro-rand so a cap is exact:
//!
//! ```
//! use timelex_mcp_host::{Price, Rand, Usage};
//!
//! let usage = Usage {
//!     input_tokens: 2_000,
//!     output_tokens: 400,
//!     ..Usage::default()
//! };
//! // Haiku 5.5: R1.80 per million input tokens and R9 per million output tokens.
//! let price = Price::new(1_800_000, 9_000_000);
//! assert_eq!(price.cost(&usage), Rand::from_micros(7_200));
//! ```
//!
//! The key path is a lease from the vault (ADR-0004, ADR-0020): the caller uses a
//! [`VaultKeySource`] to lease the tenant's `anthropic_api_key`, builds an [`AnthropicProvider`]
//! with it for the run, and drops it afterwards. `SecretValue` wipes the key on drop and redacts
//! its `Debug`.

mod anthropic;
mod ladder;
mod price;
mod provider;
mod rand;
mod run;
mod vault;

pub use anthropic::{AnthropicProvider, DEFAULT_BASE_URL};
pub use ladder::{Ladder, TenantPolicy};
pub use price::{Price, PriceList};
pub use provider::{
    Completion, CompletionRequest, ModelCatalog, ModelError, ModelProvider, ModelTier, StopReason, Usage,
};
pub use rand::Rand;
pub use run::{
    run, Call, CallOutcome, Report, ReviewReason, RunInput, RunPolicy, Terminal, Validator, Verdict,
};
pub use vault::{Lease, LeaseError, LeasePurpose, VaultKeySource};
