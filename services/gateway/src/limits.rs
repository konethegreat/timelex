// Written by Kone Tshivhinda & Claude
//! Token buckets, one per firm and one for the platform operator (ADR-0013).
//!
//! A bucket holds `burst` tokens and refills at `per_minute` tokens a minute. Each request spends one;
//! with no token left the caller gets 429 and a whole-second wait. Buckets are created only after
//! authentication, so an unauthenticated caller cannot spend another firm's budget.
//!
//! The store is in memory: buckets are lost on restart and every replica has its own. Phase 1 runs one
//! gateway process, so the effective budget is the configured one.
//!
//! "Now" is a parameter, so the refill rules are tested with a fake clock.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

use uuid::Uuid;

/// What a bucket belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RateKey {
    /// One firm, identified by the tenant id identity confirmed.
    Tenant(Uuid),
    /// The platform operator, which has its own smaller budget.
    Operator,
}

/// The answer to one request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decision {
    /// Whether the request may proceed.
    pub allowed: bool,
    /// When refused, how many whole seconds to wait before retrying (at least 1).
    pub retry_after_seconds: u64,
}

struct Budget {
    per_minute: f64,
    burst: f64,
}

struct Bucket {
    tokens: f64,
    last: Instant,
}

/// All buckets for one gateway process.
pub struct RateLimiter {
    firm: Budget,
    operator: Budget,
    buckets: Mutex<HashMap<RateKey, Bucket>>,
}

impl RateLimiter {
    /// Build the limiter. Values are validated by [`crate::config`], which refuses zero and absurd sizes
    /// at start-up, so this cannot divide by zero.
    pub fn new(firm_per_minute: u32, firm_burst: u32, operator_per_minute: u32, operator_burst: u32) -> Self {
        Self {
            firm: Budget {
                per_minute: f64::from(firm_per_minute),
                burst: f64::from(firm_burst),
            },
            operator: Budget {
                per_minute: f64::from(operator_per_minute),
                burst: f64::from(operator_burst),
            },
            buckets: Mutex::new(HashMap::new()),
        }
    }

    /// Spend one token for `key` at `now`, or say how long to wait.
    pub fn check(&self, key: RateKey, now: Instant) -> Decision {
        let budget = match key {
            RateKey::Tenant(_) => &self.firm,
            RateKey::Operator => &self.operator,
        };
        // A panic elsewhere must not stop the gateway from limiting: recover the poisoned map.
        let mut buckets = self.buckets.lock().unwrap_or_else(|e| e.into_inner());
        let bucket = buckets.entry(key).or_insert(Bucket {
            tokens: budget.burst,
            last: now,
        });
        // A clock that appears to go backwards refills nothing.
        let elapsed = now.saturating_duration_since(bucket.last).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * budget.per_minute / 60.0).min(budget.burst);
        bucket.last = now;

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            Decision {
                allowed: true,
                retry_after_seconds: 0,
            }
        } else {
            let wait = (1.0 - bucket.tokens) * 60.0 / budget.per_minute;
            Decision {
                allowed: false,
                retry_after_seconds: wait.ceil().max(1.0) as u64,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn tenant(n: u128) -> RateKey {
        RateKey::Tenant(Uuid::from_u128(n))
    }

    #[test]
    fn a_burst_is_spent_then_the_next_request_waits() {
        let l = RateLimiter::new(60, 3, 60, 3);
        let t0 = Instant::now();
        assert!(l.check(tenant(1), t0).allowed);
        assert!(l.check(tenant(1), t0).allowed);
        assert!(l.check(tenant(1), t0).allowed);
        let refused = l.check(tenant(1), t0);
        assert!(!refused.allowed);
        assert_eq!(refused.retry_after_seconds, 1, "60 a minute refills one a second");
    }

    #[test]
    fn tokens_refill_at_the_configured_rate() {
        let l = RateLimiter::new(60, 2, 60, 2);
        let t0 = Instant::now();
        assert!(l.check(tenant(1), t0).allowed);
        assert!(l.check(tenant(1), t0).allowed);
        assert!(!l.check(tenant(1), t0).allowed);
        // 60 a minute is one a second: half a second is not enough.
        assert!(!l.check(tenant(1), t0 + Duration::from_millis(500)).allowed);
        assert!(l.check(tenant(1), t0 + Duration::from_millis(1000)).allowed);
        // Refill never exceeds the burst: a long quiet spell does not bank tokens.
        let t1 = t0 + Duration::from_secs(3600);
        assert!(l.check(tenant(1), t1).allowed);
        assert!(l.check(tenant(1), t1).allowed);
        assert!(!l.check(tenant(1), t1).allowed);
    }

    #[test]
    fn firms_are_isolated_and_the_operator_has_its_own_budget() {
        let l = RateLimiter::new(60, 1, 6, 1);
        let t0 = Instant::now();
        assert!(l.check(tenant(1), t0).allowed);
        assert!(!l.check(tenant(1), t0).allowed);
        assert!(l.check(tenant(2), t0).allowed, "another firm is unaffected");
        assert!(l.check(RateKey::Operator, t0).allowed);
        let refused = l.check(RateKey::Operator, t0);
        assert!(!refused.allowed);
        assert_eq!(refused.retry_after_seconds, 10, "6 a minute needs ten seconds");
        assert!(l.check(tenant(1), t0 + Duration::from_secs(1)).allowed);
    }

    #[test]
    fn a_clock_going_backwards_grants_nothing() {
        let l = RateLimiter::new(60, 1, 60, 1);
        let t0 = Instant::now();
        assert!(l.check(tenant(1), t0).allowed);
        assert!(!l.check(tenant(1), t0 - Duration::from_secs(5)).allowed);
    }

    #[test]
    fn the_wait_is_always_at_least_one_second() {
        let l = RateLimiter::new(600, 1, 600, 1);
        let t0 = Instant::now();
        assert!(l.check(tenant(1), t0).allowed);
        // A token is ready after 100 ms, but a bare `Retry-After: 0` invites an immediate retry loop.
        let refused = l.check(tenant(1), t0);
        assert!(!refused.allowed);
        assert_eq!(refused.retry_after_seconds, 1);
    }
}
