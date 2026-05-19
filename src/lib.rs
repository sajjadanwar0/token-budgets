//! # token-budgets
//!
//! Affine resource discipline for LLM cost control in Rust.
//! Provides `Budget<const MAX: u64>` with compile-time A2 cap
//! enforcement and runtime ownership semantics. Includes:
//!
//! - Core `Budget<const MAX>`: spend / split / merge / consume
//! - `BudgetMint` capability-token constructor pattern (feature-gated)
//! - `Receipt`/`Refund` for reserve-confirm-refund (A1 enforcement)
//! - `BudgetPool` for multi-tenant atomic reservation
//! - `StreamingReceipt` for per-chunk refund during streaming
//! - `ReasoningProvider` for o1 / DeepSeek-R1 hidden-token handling
//!
//! ## Capability gate (since v0.5)
//!
//! Construction of a `Budget` now requires either:
//!   - A `BudgetMint` capability (preferred): `Budget::mint(&mint, n)`
//!   - A `CapAuthority` (legacy alias): `Budget::new_sealed(&auth, n)`
//!
//! Acquiring a `BudgetMint` requires the `system-authority` Cargo feature,
//! which should be enabled only in the top-level binary's `Cargo.toml`.
//! Library crates and transitive dependencies cannot enable this feature
//! transparently: feature enablement appears in the workspace `Cargo.lock`
//! and is reviewable in PR diffs.
//!
//! `Budget::new` is now `pub(crate)`: workspace-resident rogue crates
//! cannot mint arbitrary Budget values bypassing the capability gate.
//! See [`mint`] for the threat model.

pub mod estimator;
pub mod pool_typestate;
pub mod mint;

pub use pool_typestate::{ResolvedReceipt, ReservationReceipt};

pub use crate::estimator::{AnthropicEstimator, ByteLength, TokenEstimator};

pub use crate::mint::{BudgetMint, CapAuthority};

#[cfg(feature = "tiktoken")]
pub use estimator::Tiktoken;

use std::sync::{Arc, Mutex};


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetError {
    InsufficientFunds,
    ExceedsMax,
    Overflow,
}

impl std::fmt::Display for BudgetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InsufficientFunds => write!(f, "insufficient funds in budget"),
            Self::ExceedsMax => write!(f, "operation would exceed budget cap"),
            Self::Overflow => write!(f, "arithmetic overflow in budget operation"),
        }
    }
}

impl std::error::Error for BudgetError {}

// ============================================================================
// Budget<const MAX: u64>: core affine resource
// ============================================================================

#[derive(Debug)]
pub struct Budget<const MAX: u64> {
    micro_cents: u64,
}

impl<const MAX: u64> Budget<MAX> {
    const _A2_HOLDS: () = {
        assert!(MAX < (1u64 << 63), "Budget<MAX>: MAX must be < 2^63 for A2 safety");
    };

    /// Crate-private constructor. Callable only from within the
    /// `token_budgets` crate.
    ///
    /// Downstream crates must use [`Budget::mint`] (paper-consistent
    /// terminology) or [`Budget::new_sealed`] (legacy alias) instead.
    /// Both require a `BudgetMint` capability.
    pub(crate) fn new(micro_cents: u64) -> Result<Self, BudgetError> {
        let _: () = Self::_A2_HOLDS;
        if micro_cents > MAX {
            return Err(BudgetError::ExceedsMax);
        }
        Ok(Self { micro_cents })
    }

    /// Mint a new `Budget` from a capability token.
    ///
    /// The `&BudgetMint` parameter is unused at runtime; its presence
    /// is a compile-time proof that the caller possesses mint authority.
    /// Acquiring a `BudgetMint` requires the `system-authority` Cargo
    /// feature (enabled only in top-level binaries).
    pub fn mint(auth: &BudgetMint, micro_cents: u64) -> Result<Self, BudgetError> {
        Self::new_sealed(auth, micro_cents)
    }

    /// Legacy alias for [`Budget::mint`].
    ///
    /// Retained for backward compatibility with v0.4 callers.
    /// Prefer [`Budget::mint`] in new code; the paper uses that name.
    pub fn new_sealed(_auth: &CapAuthority, micro_cents: u64) -> Result<Self, BudgetError> {
        Self::new(micro_cents)
    }

    pub fn micro_cents(&self) -> u64 {
        self.micro_cents
    }

    pub const fn max() -> u64 {
        MAX
    }

    pub fn spend(self, amount: u64) -> Result<Self, BudgetError> {
        let _: () = Self::_A2_HOLDS;
        if amount > self.micro_cents {
            return Err(BudgetError::InsufficientFunds);
        }
        Ok(Self { micro_cents: self.micro_cents - amount })
    }

    pub fn split(self, amount: u64) -> Result<(Self, Self), BudgetError> {
        let _: () = Self::_A2_HOLDS;
        if amount > self.micro_cents {
            return Err(BudgetError::InsufficientFunds);
        }
        let remainder = self.micro_cents - amount;
        Ok((Self { micro_cents: amount }, Self { micro_cents: remainder }))
    }

    pub fn merge(self, other: Self) -> Result<Self, BudgetError> {
        let _: () = Self::_A2_HOLDS;
        let headroom = MAX - self.micro_cents;
        if other.micro_cents > headroom {
            return Err(BudgetError::ExceedsMax);
        }
        Ok(Self { micro_cents: self.micro_cents + other.micro_cents })
    }

    pub fn consume(self) -> u64 {
        self.micro_cents
    }
}

// ============================================================================
// Receipt / Refund: affine resources for reserve-confirm-refund
// ============================================================================

#[must_use = "Receipt must be consumed via confirm(), forfeit(), or refund_to()"]
pub struct Receipt<const MAX: u64> {
    reserved: u64,
}

impl<const MAX: u64> Receipt<MAX> {
    pub fn reserved(&self) -> u64 {
        self.reserved
    }

    pub fn confirm(self, actual: u64) -> Result<Refund<MAX>, BudgetError> {
        if actual > self.reserved {
            return Err(BudgetError::ExceedsMax);
        }
        Ok(Refund { amount: self.reserved - actual })
    }

    pub fn forfeit(self) {
        // Drop self; reserved amount stays debited from budget.
    }
}

#[must_use = "Refund must be applied via apply_to() or it will be lost"]
pub struct Refund<const MAX: u64> {
    amount: u64,
}

impl<const MAX: u64> Refund<MAX> {
    pub fn amount(&self) -> u64 {
        self.amount
    }

    pub fn apply_to(self, b: Budget<MAX>) -> Result<Budget<MAX>, BudgetError> {
        let headroom = MAX - b.micro_cents;
        if self.amount > headroom {
            return Err(BudgetError::ExceedsMax);
        }
        Ok(Budget { micro_cents: b.micro_cents + self.amount })
    }
}

impl<const MAX: u64> Budget<MAX> {
    pub fn spend_with_receipt(self, reserved: u64) -> Result<(Self, Receipt<MAX>), BudgetError> {
        let _: () = Self::_A2_HOLDS;
        if reserved > self.micro_cents {
            return Err(BudgetError::InsufficientFunds);
        }
        Ok((
            Self { micro_cents: self.micro_cents - reserved },
            Receipt { reserved },
        ))
    }
}

// ============================================================================
// BudgetPool: multi-tenant atomic reservation
// ============================================================================

#[derive(Clone)]
pub struct BudgetPool {
    state: Arc<Mutex<PoolState>>,
    cap: u64,
}

struct PoolState {
    available: u64,
    outstanding: u64,
    initial: u64,
}

impl BudgetPool {
    pub fn new(initial: u64, cap: u64) -> Result<Self, BudgetError> {
        if cap >= 1u64 << 63 {
            return Err(BudgetError::ExceedsMax);
        }
        if initial > cap {
            return Err(BudgetError::ExceedsMax);
        }
        Ok(Self {
            state: Arc::new(Mutex::new(PoolState {
                available: initial,
                outstanding: 0,
                initial,
            })),
            cap,
        })
    }

    pub fn reserve(&self, amount: u64) -> Result<Reservation, BudgetError> {
        let mut state = self.state.lock().unwrap();
        if amount > state.available {
            return Err(BudgetError::InsufficientFunds);
        }
        state.available -= amount;
        state.outstanding += amount;
        Ok(Reservation {
            pool: self.clone(),
            amount,
            consumed: false,
        })
    }

    pub fn available(&self) -> u64 {
        self.state.lock().unwrap().available
    }

    pub fn outstanding(&self) -> u64 {
        self.state.lock().unwrap().outstanding
    }

    pub fn cap(&self) -> u64 {
        self.cap
    }

    pub fn invariant_holds(&self) -> bool {
        let s = self.state.lock().unwrap();
        s.available + s.outstanding <= s.initial && s.available <= self.cap
    }
}

/// Atomic reservation handle. Move-only (no Clone). On Drop without
/// commit/cancel, the reservation is forfeited (amount stays debited
/// from the pool's available balance).
pub struct Reservation {
    pool: BudgetPool,
    amount: u64,
    consumed: bool,
}

impl Reservation {
    pub fn amount(&self) -> u64 {
        self.amount
    }

    pub fn commit(mut self, actual: u64) -> Result<(), BudgetError> {
        if actual > self.amount {
            return Err(BudgetError::ExceedsMax);
        }
        let refund = self.amount - actual;
        {
            let mut state = self.pool.state.lock().unwrap();
            state.outstanding -= self.amount;
            state.available += refund;
        }
        self.consumed = true;
        Ok(())
    }

    pub fn cancel(mut self) {
        {
            let mut state = self.pool.state.lock().unwrap();
            state.outstanding -= self.amount;
            state.available += self.amount;
        }
        self.consumed = true;
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        if !self.consumed {
            let mut state = self.pool.state.lock().unwrap();
            state.outstanding -= self.amount;
        }
    }
}

// ============================================================================
// StreamingReceipt: per-chunk refund
// ============================================================================

pub struct StreamingReceipt<const MAX: u64> {
    initial_reserved: u64,
    confirmed: u64,
}

impl<const MAX: u64> StreamingReceipt<MAX> {
    pub fn reserved(&self) -> u64 {
        self.initial_reserved
    }

    pub fn confirmed(&self) -> u64 {
        self.confirmed
    }

    pub fn remaining(&self) -> u64 {
        self.initial_reserved - self.confirmed
    }

    pub fn confirm_chunk(&mut self, chunk_cost: u64) -> Result<(), BudgetError> {
        let new_confirmed = self
            .confirmed
            .checked_add(chunk_cost)
            .ok_or(BudgetError::Overflow)?;
        if new_confirmed > self.initial_reserved {
            return Err(BudgetError::ExceedsMax);
        }
        self.confirmed = new_confirmed;
        Ok(())
    }

    pub fn close(self) -> Refund<MAX> {
        Refund { amount: self.initial_reserved - self.confirmed }
    }
}

impl<const MAX: u64> Budget<MAX> {
    pub fn spend_streaming(
        self,
        reserved: u64,
    ) -> Result<(Self, StreamingReceipt<MAX>), BudgetError> {
        let _: () = Self::_A2_HOLDS;
        if reserved > self.micro_cents {
            return Err(BudgetError::InsufficientFunds);
        }
        Ok((
            Self { micro_cents: self.micro_cents - reserved },
            StreamingReceipt {
                initial_reserved: reserved,
                confirmed: 0,
            },
        ))
    }
}

// ============================================================================
// ReasoningProvider: o1 / DeepSeek-R1 hidden-token handling
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasoningProvider {
    OpenAIO1 { per_call_reasoning_p99_uc: u64 },
    DeepSeekR1 { per_call_reasoning_p99_uc: u64 },
    Anthropic,
}

impl ReasoningProvider {
    pub fn reasoning_reservation(self) -> u64 {
        match self {
            Self::OpenAIO1 { per_call_reasoning_p99_uc } => per_call_reasoning_p99_uc,
            Self::DeepSeekR1 { per_call_reasoning_p99_uc } => per_call_reasoning_p99_uc,
            Self::Anthropic => 0,
        }
    }
}

impl<const MAX: u64> Budget<MAX> {
    pub fn spend_with_reasoning(
        self,
        visible_estimate: u64,
        provider: ReasoningProvider,
    ) -> Result<(Self, Receipt<MAX>), BudgetError> {
        let total_reserve = visible_estimate
            .checked_add(provider.reasoning_reservation())
            .ok_or(BudgetError::Overflow)?;
        self.spend_with_receipt(total_reserve)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    type B = Budget<1_000_000>;

    // Internal tests use `Budget::new` directly because they live inside
    // the `token_budgets` crate (where `new` is `pub(crate)`). External
    // tests in `tests/` and examples in `examples/` must use
    // `Budget::mint(&BudgetMint, ...)` with the `system-authority` feature.

    #[test]
    fn basic_construction() {
        let b = B::new(500_000).unwrap();
        assert_eq!(b.micro_cents(), 500_000);
    }

    #[test]
    fn construction_above_max_fails() {
        assert!(matches!(B::new(2_000_000), Err(BudgetError::ExceedsMax)));
    }

    #[test]
    fn spend_basic() {
        let b = B::new(1_000_000).unwrap();
        let b = b.spend(300_000).unwrap();
        assert_eq!(b.micro_cents(), 700_000);
    }

    #[test]
    fn spend_above_balance_fails() {
        let b = B::new(100).unwrap();
        assert!(matches!(b.spend(101), Err(BudgetError::InsufficientFunds)));
    }

    #[test]
    fn split_conserves() {
        let b = B::new(1_000_000).unwrap();
        let (a, b2) = b.split(300_000).unwrap();
        assert_eq!(a.micro_cents(), 300_000);
        assert_eq!(b2.micro_cents(), 700_000);
    }

    #[test]
    fn merge_conserves() {
        let a = B::new(300_000).unwrap();
        let b = B::new(400_000).unwrap();
        let m = a.merge(b).unwrap();
        assert_eq!(m.micro_cents(), 700_000);
    }

    #[test]
    fn merge_above_max_fails() {
        let a = B::new(600_000).unwrap();
        let b = B::new(500_000).unwrap();
        assert!(matches!(a.merge(b), Err(BudgetError::ExceedsMax)));
    }

    #[test]
    fn receipt_confirm_refund() {
        let b = B::new(1_000_000).unwrap();
        let (b, r) = b.spend_with_receipt(500).unwrap();
        assert_eq!(b.micro_cents(), 999_500);
        let refund = r.confirm(200).unwrap();
        assert_eq!(refund.amount(), 300);
        let b = refund.apply_to(b).unwrap();
        assert_eq!(b.micro_cents(), 999_800);
    }

    #[test]
    fn a1_violation_rejected() {
        let b = B::new(1_000_000).unwrap();
        let (_, r) = b.spend_with_receipt(500).unwrap();
        assert!(matches!(r.confirm(501), Err(BudgetError::ExceedsMax)));
    }

    #[test]
    #[cfg(feature = "system-authority")]
    fn cap_authority_pattern() {
        let auth = CapAuthority::seal_at_startup();
        let b = B::new_sealed(&auth, 500_000).unwrap();
        assert_eq!(b.micro_cents(), 500_000);
    }

    #[test]
    #[cfg(feature = "system-authority")]
    fn budget_mint_alias_works() {
        let mint = BudgetMint::take_authority();
        let b = Budget::<1_000_000>::mint(&mint, 500_000).unwrap();
        assert_eq!(b.micro_cents(), 500_000);
    }

    #[test]
    fn budget_error_is_std_error() {
        // Compile-time check: BudgetError implements std::error::Error
        fn assert_error<E: std::error::Error>() {}
        assert_error::<BudgetError>();
    }
}

#[cfg(test)]
mod pool_tests {
    use super::*;

    #[test]
    fn pool_reserve_commit_refunds_difference() {
        let pool = BudgetPool::new(1_000_000, 1_000_000).unwrap();
        let r = pool.reserve(300_000).unwrap();
        r.commit(180_000).unwrap();
        assert_eq!(pool.available(), 700_000 + 120_000);
        assert!(pool.invariant_holds());
    }

    #[test]
    fn pool_reserve_cancel_full_refund() {
        let pool = BudgetPool::new(1_000_000, 1_000_000).unwrap();
        let r = pool.reserve(300_000).unwrap();
        r.cancel();
        assert_eq!(pool.available(), 1_000_000);
        assert!(pool.invariant_holds());
    }

    #[test]
    fn pool_reserve_forget_forfeits() {
        let pool = BudgetPool::new(1_000_000, 1_000_000).unwrap();
        {
            let _r = pool.reserve(300_000).unwrap();
        }
        assert_eq!(pool.available(), 700_000);
        assert_eq!(pool.outstanding(), 0);
        assert!(pool.invariant_holds());
    }

    #[test]
    fn pool_commit_above_reserved_fails() {
        let pool = BudgetPool::new(1_000_000, 1_000_000).unwrap();
        let r = pool.reserve(300_000).unwrap();
        assert!(matches!(r.commit(300_001), Err(BudgetError::ExceedsMax)));
    }

    #[test]
    fn pool_reserve_exceeds_available_fails() {
        let pool = BudgetPool::new(100, 1_000_000).unwrap();
        assert!(matches!(pool.reserve(101), Err(BudgetError::InsufficientFunds)));
    }

    #[test]
    fn pool_concurrent_reservations_preserve_invariant() {
        use std::thread;
        let pool = BudgetPool::new(1_000_000, 1_000_000).unwrap();

        let pool1 = pool.clone();
        let h1 = thread::spawn(move || {
            for _ in 0..1000 {
                if let Ok(r) = pool1.reserve(100) {
                    let _ = r.commit(80);
                }
            }
        });

        let pool2 = pool.clone();
        let h2 = thread::spawn(move || {
            for _ in 0..1000 {
                if let Ok(r) = pool2.reserve(200) {
                    let _ = r.commit(150);
                }
            }
        });

        h1.join().unwrap();
        h2.join().unwrap();
        assert!(pool.invariant_holds());
    }
}

#[cfg(test)]
mod streaming_tests {
    use super::*;
    type B = Budget<1_000_000>;

    #[test]
    fn streaming_basic_three_chunks() {
        let b = B::new(1_000_000).unwrap();
        let (b, mut r) = b.spend_streaming(1000).unwrap();
        assert_eq!(b.micro_cents(), 999_000);
        r.confirm_chunk(100).unwrap();
        r.confirm_chunk(200).unwrap();
        r.confirm_chunk(50).unwrap();
        let refund = r.close();
        assert_eq!(refund.amount(), 650);
        let b = refund.apply_to(b).unwrap();
        assert_eq!(b.micro_cents(), 1_000_000 - 350);
    }

    #[test]
    fn streaming_exact_consumes_all() {
        let b = B::new(1_000_000).unwrap();
        let (b, mut r) = b.spend_streaming(500).unwrap();
        r.confirm_chunk(500).unwrap();
        let refund = r.close();
        assert_eq!(refund.amount(), 0);
        let b = refund.apply_to(b).unwrap();
        assert_eq!(b.micro_cents(), 1_000_000 - 500);
    }

    #[test]
    fn streaming_overconfirm_rejects() {
        let b = B::new(1_000_000).unwrap();
        let (_b, mut r) = b.spend_streaming(500).unwrap();
        r.confirm_chunk(400).unwrap();
        assert!(matches!(r.confirm_chunk(101), Err(BudgetError::ExceedsMax)));
        assert_eq!(r.confirmed(), 400);
        assert_eq!(r.remaining(), 100);
    }

    #[test]
    fn streaming_forfeit_keeps_reservation_debited() {
        let b = B::new(1_000_000).unwrap();
        let (b_after, r) = b.spend_streaming(1000).unwrap();
        assert_eq!(b_after.micro_cents(), 999_000);
        drop(r);
        assert_eq!(b_after.micro_cents(), 999_000);
    }
}

#[cfg(test)]
mod reasoning_tests {
    use super::*;
    type B = Budget<10_000_000>;

    #[test]
    fn reasoning_reservation_includes_overhead() {
        let provider = ReasoningProvider::OpenAIO1 { per_call_reasoning_p99_uc: 1000 };
        let b = B::new(10_000_000).unwrap();
        let (b_after, receipt) = b.spend_with_reasoning(500, provider).unwrap();
        assert_eq!(receipt.reserved(), 1500);
        assert_eq!(b_after.micro_cents(), 10_000_000 - 1500);
    }

    #[test]
    fn reasoning_confirm_with_low_actual_refunds() {
        let provider = ReasoningProvider::DeepSeekR1 { per_call_reasoning_p99_uc: 800 };
        let b = B::new(10_000_000).unwrap();
        let (b_after, receipt) = b.spend_with_reasoning(500, provider).unwrap();
        let refund = receipt.confirm(300).unwrap();
        assert_eq!(refund.amount(), 1000);
        let b = refund.apply_to(b_after).unwrap();
        assert_eq!(b.micro_cents(), 10_000_000 - 300);
    }

    #[test]
    fn reasoning_overflow_fails() {
        let provider = ReasoningProvider::OpenAIO1 { per_call_reasoning_p99_uc: u64::MAX };
        let b = B::new(100).unwrap();
        assert!(matches!(
            b.spend_with_reasoning(100, provider),
            Err(BudgetError::Overflow)
        ));
    }

    #[test]
    fn reasoning_anthropic_no_overhead() {
        let provider = ReasoningProvider::Anthropic;
        let b = B::new(10_000_000).unwrap();
        let (b_after, receipt) = b.spend_with_reasoning(500, provider).unwrap();
        assert_eq!(receipt.reserved(), 500);
        assert_eq!(b_after.micro_cents(), 10_000_000 - 500);
    }
}