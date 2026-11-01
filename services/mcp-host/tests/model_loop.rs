// Written by Kone Tshivhinda & Claude
//! The model loop's contract: a terminal state, a Rand cap that really stops spending, and an
//! escalation ladder that skips models this tenant or this key cannot use.

use std::collections::VecDeque;
use std::sync::Mutex;

use async_trait::async_trait;
use timelex_mcp_host::{
    run, Completion, Ladder, ModelError, ModelProvider, ModelTier, Price, PriceList, Rand, Report,
    ReviewReason, RunInput, RunPolicy, StopReason, TenantPolicy, Terminal, Usage, Validator, Verdict,
};

/// A provider that answers from a script and records which tier each call asked for.
struct Scripted {
    replies: Mutex<VecDeque<Result<Completion, ModelError>>>,
    asked: Mutex<Vec<ModelTier>>,
}

impl Scripted {
    fn new(replies: Vec<Result<Completion, ModelError>>) -> Self {
        Self {
            replies: Mutex::new(replies.into()),
            asked: Mutex::new(Vec::new()),
        }
    }

    fn asked(&self) -> Vec<ModelTier> {
        self.asked.lock().unwrap().clone()
    }
}

#[async_trait]
impl ModelProvider for Scripted {
    async fn complete(&self, request: timelex_mcp_host::CompletionRequest) -> Result<Completion, ModelError> {
        self.asked.lock().unwrap().push(request.model);
        self.replies
            .lock()
            .unwrap()
            .pop_front()
            .expect("the loop asked for more replies than the script has")
    }
}

fn reply(input: u64, output: u64) -> Completion {
    Completion {
        text: "a reply".into(),
        usage: Usage {
            input_tokens: input,
            output_tokens: output,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        },
        stop_reason: StopReason::EndTurn,
    }
}

/// 2 000 input and 400 output tokens on Haiku costs R0.0072 (research 04 §7).
fn haiku_reply() -> Completion {
    reply(2_000, 400)
}

fn input() -> RunInput {
    RunInput {
        system: "You attribute time.".into(),
        prompt: "Attribute this block.".into(),
        max_tokens: 1_024,
    }
}

fn policy(max_rand: Rand, max_steps: u32) -> RunPolicy {
    RunPolicy::for_tenant(
        TenantPolicy::default(),
        PriceList::published(),
        max_rand,
        max_steps,
    )
}

fn accept(_: &Completion) -> Verdict {
    Verdict::Accept
}

fn escalate(_: &Completion) -> Verdict {
    Verdict::Escalate
}

async fn run_script(
    replies: Vec<Result<Completion, ModelError>>,
    policy: RunPolicy,
    validator: &dyn Validator,
) -> (Scripted, Report) {
    let provider = Scripted::new(replies);
    let report = run(&provider, &policy, input(), validator).await;
    (provider, report)
}

#[tokio::test]
async fn an_accepted_reply_ends_the_run_on_the_first_rung() {
    let (provider, report) = run_script(
        vec![Ok(haiku_reply())],
        policy(Rand::from_micros(1_000_000), 8),
        &accept,
    )
    .await;

    assert_eq!(
        report.terminal,
        Terminal::Done {
            output: "a reply".into()
        }
    );
    assert_eq!(provider.asked(), vec![ModelTier::Haiku]);
    assert_eq!(report.calls.len(), 1);
    assert_eq!(report.spent, Rand::from_micros(7_200));
}

#[tokio::test]
async fn escalation_moves_to_the_next_rung() {
    let replies = vec![Ok(haiku_reply()), Ok(reply(100, 20))];
    // The scripted validator escalates once, then accepts.
    struct Once(Mutex<u32>);
    impl Validator for Once {
        fn inspect(&self, _: &Completion) -> Verdict {
            let mut seen = self.0.lock().unwrap();
            *seen += 1;
            if *seen == 1 {
                Verdict::Escalate
            } else {
                Verdict::Accept
            }
        }
    }

    let (provider, report) = run_script(
        replies,
        policy(Rand::from_micros(1_000_000), 8),
        &Once(Mutex::new(0)),
    )
    .await;

    assert!(matches!(report.terminal, Terminal::Done { .. }));
    assert_eq!(provider.asked(), vec![ModelTier::Haiku, ModelTier::Sonnet]);
    assert_eq!(report.calls.len(), 2);
}

#[tokio::test]
async fn a_model_the_key_cannot_use_degrades_to_the_next_rung() {
    let replies = vec![Err(ModelError::ModelUnavailable), Ok(haiku_reply())];
    let (provider, report) = run_script(replies, policy(Rand::from_micros(1_000_000), 8), &accept).await;

    assert!(matches!(report.terminal, Terminal::Done { .. }));
    assert_eq!(provider.asked(), vec![ModelTier::Haiku, ModelTier::Sonnet]);
    assert_eq!(report.calls.len(), 2);
    assert_eq!(
        report.calls[0].cost,
        Rand::ZERO,
        "an unavailable model costs nothing"
    );
    // The successful call was Sonnet, so it is priced at the Sonnet rate.
    assert_eq!(report.spent, Rand::from_micros(144_000));
}

#[tokio::test]
async fn a_rejected_key_stops_the_loop_without_escalating() {
    let replies = vec![Err(ModelError::KeyRejected), Ok(haiku_reply())];
    let (provider, report) = run_script(replies, policy(Rand::from_micros(1_000_000), 8), &accept).await;

    assert_eq!(
        report.terminal,
        Terminal::NeedsReview {
            reason: ReviewReason::KeyRejected
        }
    );
    assert_eq!(provider.asked(), vec![ModelTier::Haiku]);
}

#[tokio::test]
async fn the_rand_cap_aborts_the_loop() {
    // The first reply costs R0.0072, more than a R0.005 cap; the validator would have escalated,
    // so the cap is the only reason the second call does not happen.
    let replies = vec![Ok(haiku_reply()), Ok(haiku_reply())];
    let (provider, report) = run_script(replies, policy(Rand::from_micros(5_000), 8), &escalate).await;

    assert_eq!(
        report.terminal,
        Terminal::NeedsReview {
            reason: ReviewReason::BudgetExceeded
        }
    );
    assert_eq!(
        provider.asked(),
        vec![ModelTier::Haiku],
        "no call after the cap is passed"
    );
    assert_eq!(report.spent, Rand::from_micros(7_200));
}

#[tokio::test]
async fn a_zero_cap_makes_no_call_at_all() {
    let (provider, report) = run_script(vec![Ok(haiku_reply())], policy(Rand::ZERO, 8), &accept).await;

    assert_eq!(
        report.terminal,
        Terminal::NeedsReview {
            reason: ReviewReason::BudgetExceeded
        }
    );
    assert!(provider.asked().is_empty());
}

#[tokio::test]
async fn escalating_past_the_last_rung_needs_a_human() {
    let replies = vec![Ok(haiku_reply()), Ok(haiku_reply()), Ok(haiku_reply())];
    let (provider, report) = run_script(replies, policy(Rand::from_micros(1_000_000), 8), &escalate).await;

    assert_eq!(
        report.terminal,
        Terminal::NeedsReview {
            reason: ReviewReason::EscalatedToHuman
        }
    );
    assert_eq!(
        provider.asked(),
        vec![ModelTier::Haiku, ModelTier::Sonnet, ModelTier::Opus]
    );
}

#[tokio::test]
async fn a_ladder_that_runs_out_of_models_needs_review() {
    let replies = vec![
        Err(ModelError::ModelUnavailable),
        Err(ModelError::ModelUnavailable),
        Err(ModelError::ModelUnavailable),
    ];
    let (provider, report) = run_script(replies, policy(Rand::from_micros(1_000_000), 8), &accept).await;

    assert_eq!(
        report.terminal,
        Terminal::NeedsReview {
            reason: ReviewReason::NoModelAvailable
        }
    );
    assert_eq!(provider.asked().len(), 3, "every rung was tried once");
}

#[tokio::test]
async fn a_provider_failure_stops_the_run() {
    let replies = vec![Err(ModelError::Unreachable), Ok(haiku_reply())];
    let (provider, report) = run_script(replies, policy(Rand::from_micros(1_000_000), 8), &accept).await;

    assert_eq!(
        report.terminal,
        Terminal::NeedsReview {
            reason: ReviewReason::ProviderUnreachable
        }
    );
    assert_eq!(provider.asked(), vec![ModelTier::Haiku]);
}

#[tokio::test]
async fn an_unusable_reply_stops_the_run() {
    let replies = vec![Err(ModelError::Unusable), Ok(haiku_reply())];
    let (provider, report) = run_script(replies, policy(Rand::from_micros(1_000_000), 8), &accept).await;

    assert_eq!(
        report.terminal,
        Terminal::NeedsReview {
            reason: ReviewReason::UnusableReply
        }
    );
    assert_eq!(provider.asked(), vec![ModelTier::Haiku]);
}

#[tokio::test]
async fn the_step_limit_stops_a_run_that_never_accepts() {
    let replies = vec![Ok(haiku_reply()), Ok(haiku_reply()), Ok(haiku_reply())];
    let (provider, report) = run_script(replies, policy(Rand::from_micros(1_000_000), 2), &escalate).await;

    assert_eq!(
        report.terminal,
        Terminal::NeedsReview {
            reason: ReviewReason::StepLimit
        }
    );
    assert_eq!(provider.asked(), vec![ModelTier::Haiku, ModelTier::Sonnet]);
}

#[test]
fn fable_needs_both_the_tenant_flag_and_a_price() {
    let published = PriceList::published();
    let without_flag = Ladder::for_tenant(TenantPolicy::default(), &published);
    assert_eq!(
        without_flag.tiers(),
        &[ModelTier::Haiku, ModelTier::Sonnet, ModelTier::Opus]
    );

    // The flag alone is not enough while our price list has no Fable price: an unpriced rung
    // cannot be bounded, so it is never attempted.
    let without_price = Ladder::for_tenant(TenantPolicy { fable: true }, &published);
    assert!(!without_price.tiers().contains(&ModelTier::Fable));

    let priced = published.with_fable(Price::new(180_000_000, 900_000_000));
    let with_both = Ladder::for_tenant(TenantPolicy { fable: true }, &priced);
    assert_eq!(
        with_both.tiers(),
        &[
            ModelTier::Haiku,
            ModelTier::Sonnet,
            ModelTier::Opus,
            ModelTier::Fable
        ]
    );

    let flag_off = Ladder::for_tenant(TenantPolicy::default(), &priced);
    assert!(!flag_off.tiers().contains(&ModelTier::Fable));
}

#[test]
fn published_costs_match_the_research_model() {
    let prices = PriceList::published();
    let usage = Usage {
        input_tokens: 2_000,
        output_tokens: 400,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
    };
    assert_eq!(
        prices.cost(ModelTier::Haiku, &usage),
        Some(Rand::from_micros(7_200))
    );
    assert_eq!(
        prices.cost(ModelTier::Sonnet, &usage),
        Some(Rand::from_micros(144_000))
    );
    assert_eq!(
        prices.cost(ModelTier::Opus, &usage),
        Some(Rand::from_micros(288_000))
    );
    assert_eq!(prices.cost(ModelTier::Fable, &usage), None);
}

#[test]
fn cache_tokens_are_priced_at_the_provider_multipliers() {
    let prices = PriceList::published();
    let usage = Usage {
        input_tokens: 0,
        output_tokens: 0,
        cache_read_tokens: 1_000_000,
        cache_write_tokens: 1_000_000,
    };
    // Haiku input is R1.80 per million: a read is a tenth (R0.18) and a write a quarter more
    // (R2.25), so the total is R2.43.
    assert_eq!(
        prices.cost(ModelTier::Haiku, &usage),
        Some(Rand::from_micros(2_430_000))
    );
}
