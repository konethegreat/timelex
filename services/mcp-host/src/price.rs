// Written by Kone Tshivhinda & Claude
//! Published model prices, in Rand per million tokens.
//!
//! The figures are the list prices in [research 04 §7](../../../docs/research/04-autonomous-capture-design.md)
//! at the R18/USD planning rate: Haiku 5.5 R1.80/R9, Sonnet 5.5 R36/R180, Opus 5.5 R72/R360
//! per million input/output tokens. Cache reads cost a tenth of the input rate and cache writes a
//! quarter more, per Anthropic's published multipliers.
//!
//! Fable has **no published figure** in our research, so it has no price by default and the
//! ladder never attempts it (see [`crate::Ladder`]). A deployment that knows its Fable rate adds
//! it with [`PriceList::with_fable`]; the tenant flag alone is not enough to spend unbounded money.

use crate::provider::{ModelTier, Usage};
use crate::rand::Rand;

/// The price of one model, in micro-rand per million tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Price {
    input: u64,
    output: u64,
    cache_write: u64,
    cache_read: u64,
}

impl Price {
    /// A price from the input and output rates. The cache rates follow Anthropic's multipliers:
    /// writes are 1.25x input and reads are 0.1x input, rounded down to the micro-rand.
    pub const fn new(input_micro_rand_per_mtok: u64, output_micro_rand_per_mtok: u64) -> Self {
        Self {
            input: input_micro_rand_per_mtok,
            output: output_micro_rand_per_mtok,
            cache_write: input_micro_rand_per_mtok + input_micro_rand_per_mtok / 4,
            cache_read: input_micro_rand_per_mtok / 10,
        }
    }

    /// What this usage costs at this price.
    pub fn cost(&self, usage: &Usage) -> Rand {
        let micro = tokens_cost(usage.input_tokens, self.input)
            + tokens_cost(usage.output_tokens, self.output)
            + tokens_cost(usage.cache_write_tokens, self.cache_write)
            + tokens_cost(usage.cache_read_tokens, self.cache_read);
        Rand::from_micros(u64::try_from(micro).unwrap_or(u64::MAX))
    }
}

/// `tokens * rate / 1 000 000`, in 128 bits so a large batch cannot overflow on the way.
fn tokens_cost(tokens: u64, micro_rand_per_mtok: u64) -> u128 {
    u128::from(tokens) * u128::from(micro_rand_per_mtok) / 1_000_000
}

/// The price list a run uses. A tier missing from the list cannot be budgeted, so it is never
/// attempted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriceList {
    haiku: Option<Price>,
    sonnet: Option<Price>,
    opus: Option<Price>,
    fable: Option<Price>,
}

impl PriceList {
    /// The rates our research records: the three models every key can reach, and no Fable rate.
    pub fn published() -> Self {
        Self {
            haiku: Some(Price::new(1_800_000, 9_000_000)),
            sonnet: Some(Price::new(36_000_000, 180_000_000)),
            opus: Some(Price::new(72_000_000, 360_000_000)),
            fable: None,
        }
    }

    /// Add a Fable price, for a deployment that has one.
    pub fn with_fable(mut self, price: Price) -> Self {
        self.fable = Some(price);
        self
    }

    /// The price of a tier, if it has one.
    pub fn price(&self, tier: ModelTier) -> Option<Price> {
        match tier {
            ModelTier::Haiku => self.haiku,
            ModelTier::Sonnet => self.sonnet,
            ModelTier::Opus => self.opus,
            ModelTier::Fable => self.fable,
        }
    }

    /// What a reply from `tier` costs, if that tier is priced.
    pub fn cost(&self, tier: ModelTier, usage: &Usage) -> Option<Rand> {
        self.price(tier).map(|price| price.cost(usage))
    }
}
