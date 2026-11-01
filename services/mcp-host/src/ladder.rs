// Written by Kone Tshivhinda & Claude
//! The escalation ladder, as the tenant's policy and our price list allow it.
//!
//! The ladder is Haiku, Sonnet, Opus, Fable. A rung is only present when it can be used *and*
//! bounded: Fable needs the tenant's 30-day-retention flag (ADR-0003 amendment) and a price in
//! this deployment. Without both, Opus is the top rung and escalation goes to a human, which is
//! the terminal state the workflow layer records as `needs_review`.

use crate::price::PriceList;
use crate::provider::ModelTier;

/// What the tenant's onboarding says. Only settings that change the ladder live here.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TenantPolicy {
    /// The firm has a 30-day-retention workspace and may use Fable-class models.
    pub fable: bool,
}

/// The rungs a run may use, weakest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ladder {
    tiers: Vec<ModelTier>,
}

impl Ladder {
    /// Build the ladder from the tenant's policy and this deployment's prices.
    pub fn for_tenant(policy: TenantPolicy, prices: &PriceList) -> Self {
        let tiers = ModelTier::ALL
            .into_iter()
            .filter(|tier| *tier != ModelTier::Fable || policy.fable)
            .filter(|tier| prices.price(*tier).is_some())
            .collect();
        Self { tiers }
    }

    /// The rungs, weakest first.
    pub fn tiers(&self) -> &[ModelTier] {
        &self.tiers
    }
}
