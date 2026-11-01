// Written by Kone Tshivhinda & Claude
//! The model loop: call, validate, escalate, and stop on a Rand cap or a step limit.
//!
//! The loop always ends in one of two states. `Done` carries the accepted output; `NeedsReview`
//! carries the reason a human (or a later review queue) must look. A budget breach is a terminal
//! `needs_review`, not a retry: the run has already spent money, and asking a human is the only
//! safe next step.
//!
//! The loop never logs a prompt, a system message or a reply. Per-call records carry the tier,
//! token counts and Rand cost.

use crate::ladder::{Ladder, TenantPolicy};
use crate::price::PriceList;
use crate::provider::{Completion, CompletionRequest, ModelError, ModelProvider, ModelTier, Usage};
use crate::rand::Rand;

/// What a run asks the model. The workflow layer builds this from its context pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunInput {
    pub system: String,
    pub prompt: String,
    pub max_tokens: u32,
}

/// What the workflow layer thinks of a reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The reply is good; the run is done.
    Accept,
    /// The reply is not good enough; try the next rung.
    Escalate,
}

/// Decides whether a reply is accepted. In production this is the workflow's output check; a test
/// supplies one that escalates once and then accepts.
pub trait Validator: Send + Sync {
    fn inspect(&self, reply: &Completion) -> Verdict;
}

impl<F> Validator for F
where
    F: Fn(&Completion) -> Verdict + Send + Sync,
{
    fn inspect(&self, reply: &Completion) -> Verdict {
        self(reply)
    }
}

/// The limits and the ladder one run uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunPolicy {
    ladder: Ladder,
    prices: PriceList,
    max_rand: Rand,
    max_steps: u32,
}

impl RunPolicy {
    /// A policy with an explicit ladder.
    pub fn new(ladder: Ladder, prices: PriceList, max_rand: Rand, max_steps: u32) -> Self {
        Self {
            ladder,
            prices,
            max_rand,
            max_steps,
        }
    }

    /// A policy whose ladder follows the tenant's flags and this deployment's prices.
    pub fn for_tenant(policy: TenantPolicy, prices: PriceList, max_rand: Rand, max_steps: u32) -> Self {
        let ladder = Ladder::for_tenant(policy, &prices);
        Self::new(ladder, prices, max_rand, max_steps)
    }

    /// The Rand cap.
    pub fn max_rand(&self) -> Rand {
        self.max_rand
    }

    /// The maximum number of model calls.
    pub fn max_steps(&self) -> u32 {
        self.max_steps
    }
}

/// What happened on one provider call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallOutcome {
    Completed,
    Unavailable,
    Rejected,
    Failed,
}

/// One per-call record, for the event log and the cost report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub tier: ModelTier,
    pub outcome: CallOutcome,
    pub cost: Rand,
    pub usage: Option<Usage>,
}

/// Why a human must look at a run. The strings are the event payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewReason {
    BudgetExceeded,
    EscalatedToHuman,
    NoModelAvailable,
    KeyRejected,
    ProviderUnreachable,
    UnusableReply,
    StepLimit,
}

impl ReviewReason {
    /// The reason as it appears in a `run.needs_review` event.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BudgetExceeded => "budget_exceeded",
            Self::EscalatedToHuman => "escalated_to_human",
            Self::NoModelAvailable => "no_model_available",
            Self::KeyRejected => "key_rejected",
            Self::ProviderUnreachable => "provider_unreachable",
            Self::UnusableReply => "unusable_reply",
            Self::StepLimit => "step_limit",
        }
    }
}

impl std::fmt::Display for ReviewReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where the run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Terminal {
    Done { output: String },
    NeedsReview { reason: ReviewReason },
}

/// Everything the caller needs to persist: the terminal state, every call, and the total spend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub terminal: Terminal,
    pub calls: Vec<Call>,
    pub spent: Rand,
}

fn ended(terminal: Terminal, calls: Vec<Call>, spent: Rand) -> Report {
    Report {
        terminal,
        calls,
        spent,
    }
}

fn needs_review(reason: ReviewReason, calls: Vec<Call>, spent: Rand) -> Report {
    ended(Terminal::NeedsReview { reason }, calls, spent)
}

/// Run one workflow attempt through the ladder.
pub async fn run(
    provider: &dyn ModelProvider,
    policy: &RunPolicy,
    input: RunInput,
    validator: &dyn Validator,
) -> Report {
    let mut calls = Vec::new();
    let mut spent = Rand::ZERO;
    let mut rung = 0;

    loop {
        if calls.len() >= policy.max_steps as usize {
            return needs_review(ReviewReason::StepLimit, calls, spent);
        }
        if spent >= policy.max_rand {
            return needs_review(ReviewReason::BudgetExceeded, calls, spent);
        }
        let Some(&tier) = policy.ladder.tiers().get(rung) else {
            let reason = match calls.last().map(|call| call.outcome) {
                Some(CallOutcome::Unavailable) | None => ReviewReason::NoModelAvailable,
                Some(_) => ReviewReason::EscalatedToHuman,
            };
            return needs_review(reason, calls, spent);
        };

        let request = CompletionRequest {
            model: tier,
            system: input.system.clone(),
            prompt: input.prompt.clone(),
            max_tokens: input.max_tokens,
        };
        match provider.complete(request).await {
            Ok(completion) => {
                let cost = policy.prices.cost(tier, &completion.usage).unwrap_or(Rand::ZERO);
                spent = spent.saturating_add(cost);
                tracing::debug!(
                    tier = tier.name(),
                    input_tokens = completion.usage.input_tokens,
                    output_tokens = completion.usage.output_tokens,
                    cache_read_tokens = completion.usage.cache_read_tokens,
                    cache_write_tokens = completion.usage.cache_write_tokens,
                    cost = %cost,
                    "model call"
                );
                calls.push(Call {
                    tier,
                    outcome: CallOutcome::Completed,
                    cost,
                    usage: Some(completion.usage),
                });
                if spent > policy.max_rand {
                    return needs_review(ReviewReason::BudgetExceeded, calls, spent);
                }
                match validator.inspect(&completion) {
                    Verdict::Accept => {
                        return ended(
                            Terminal::Done {
                                output: completion.text,
                            },
                            calls,
                            spent,
                        )
                    }
                    Verdict::Escalate => rung += 1,
                }
            }
            Err(ModelError::ModelUnavailable) => {
                calls.push(Call {
                    tier,
                    outcome: CallOutcome::Unavailable,
                    cost: Rand::ZERO,
                    usage: None,
                });
                rung += 1;
            }
            Err(ModelError::KeyRejected) => {
                calls.push(Call {
                    tier,
                    outcome: CallOutcome::Rejected,
                    cost: Rand::ZERO,
                    usage: None,
                });
                return needs_review(ReviewReason::KeyRejected, calls, spent);
            }
            Err(ModelError::Unreachable) => {
                calls.push(Call {
                    tier,
                    outcome: CallOutcome::Failed,
                    cost: Rand::ZERO,
                    usage: None,
                });
                return needs_review(ReviewReason::ProviderUnreachable, calls, spent);
            }
            Err(ModelError::Unusable) => {
                calls.push(Call {
                    tier,
                    outcome: CallOutcome::Failed,
                    cost: Rand::ZERO,
                    usage: None,
                });
                return needs_review(ReviewReason::UnusableReply, calls, spent);
            }
        }
    }
}
