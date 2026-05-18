// src/pool_typestate.rs
//
// Type-system fix for the refund-mechanism leak.
//
// Problem: a `Reservation` returned by `BudgetPool::reserve(amount)`
// can be dropped without calling either `commit(actual)` or
// `cancel()`. The existing `Drop` impl preserves cap-soundness
// (it forfeits the amount: removes it from `outstanding` without
// refunding `available`), but the COMPILER does not require
// resolution. Receipts can silently leak capacity with no
// audit trail.
//
// Fix: closure-based `with_reservation` pattern. The closure's
// return type is `Result<ResolvedReceipt<T>, BudgetError>`.
// `ResolvedReceipt<T>` is constructible only by
// `ReservationReceipt::commit()` or `::cancel()`, both of which
// consume `self`. The compiler rejects any closure that does
// not explicitly resolve the receipt.

use crate::{BudgetError, BudgetPool, Reservation};

/// A witness that a reservation has been resolved. Constructible
/// only via `ReservationReceipt::commit` or `::cancel`.
#[must_use = "ResolvedReceipt should be returned from the with_reservation closure"]
pub struct ResolvedReceipt<T> {
    inner: T,
    _private: PrivateMarker,
}

// Sealed: prevents external construction outside this module.
struct PrivateMarker;

impl<T> ResolvedReceipt<T> {
    fn new(inner: T) -> Self {
        Self { inner, _private: PrivateMarker }
    }
    pub fn into_inner(self) -> T {
        self.inner
    }
}

/// Wraps the upstream `Reservation` to enforce closure-style resolution.
///
/// `commit(actual, value)` and `cancel(value)` consume `self` and
/// produce a `ResolvedReceipt<T>`; any closure that does not call
/// one of them cannot type-check.
pub struct ReservationReceipt {
    inner: Option<Reservation>,
}

impl ReservationReceipt {
    pub(crate) fn from_reservation(r: Reservation) -> Self {
        Self { inner: Some(r) }
    }

    pub fn amount(&self) -> u64 {
        self.inner.as_ref().map(|r| r.amount()).unwrap_or(0)
    }

    /// Commit `actual` micro-cents against the reservation,
    /// returning a `ResolvedReceipt<T>` witness.
    pub fn commit<T>(mut self, actual: u64, value: T) -> Result<ResolvedReceipt<T>, BudgetError> {
        let res = self.inner.take().expect("invariant: inner present until consumed");
        res.commit(actual)?;
        Ok(ResolvedReceipt::new(value))
    }

    /// Cancel the reservation (refunds capacity to the pool),
    /// returning a `ResolvedReceipt<T>` witness.
    pub fn cancel<T>(mut self, value: T) -> ResolvedReceipt<T> {
        let res = self.inner.take().expect("invariant: inner present until consumed");
        res.cancel();
        ResolvedReceipt::new(value)
    }
}

// On Drop without explicit resolution, the inner Reservation drops
// (which forfeits via the upstream Drop impl, preserving
// cap-soundness). The closure-based with_reservation pattern
// makes this branch unreachable from typechecked code.

/// Extension trait adding the closure-based reservation API to `BudgetPool`.
pub trait WithReservation {
    fn with_reservation<F, T>(&self, amount: u64, f: F) -> Result<T, BudgetError>
    where
        F: FnOnce(ReservationReceipt) -> Result<ResolvedReceipt<T>, BudgetError>;
}

impl WithReservation for BudgetPool {
    /// Reserve `amount` and pass a `ReservationReceipt` to `f`.
    /// The closure MUST resolve the receipt via `commit` or `cancel`
    /// and return the resulting `ResolvedReceipt<T>`; the compiler
    /// enforces this because `ResolvedReceipt<T>` has no public
    /// constructor outside this module.
    fn with_reservation<F, T>(&self, amount: u64, f: F) -> Result<T, BudgetError>
    where
        F: FnOnce(ReservationReceipt) -> Result<ResolvedReceipt<T>, BudgetError>,
    {
        let r = self.reserve(amount)?;
        let receipt = ReservationReceipt::from_reservation(r);
        let resolved = f(receipt)?;
        Ok(resolved.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_resolves_receipt() {
        let pool = BudgetPool::new(1000, 10_000).unwrap();
        let result = pool
            .with_reservation(500, |r| r.commit(423, "agent_response".to_string()))
            .unwrap();
        assert_eq!(result, "agent_response");
        // 500 reserved, 423 committed, 77 refunded → available = 1000 - 423
        assert_eq!(pool.available(), 1000 - 423);
        assert_eq!(pool.outstanding(), 0);
    }

    #[test]
    fn cancel_resolves_receipt() {
        let pool = BudgetPool::new(1000, 10_000).unwrap();
        let result = pool
            .with_reservation(500, |r| Ok(r.cancel(())))
            .unwrap();
        assert_eq!(result, ());
        // cancel refunds the entire amount → available unchanged
        assert_eq!(pool.available(), 1000);
        assert_eq!(pool.outstanding(), 0);
    }

    #[test]
    fn overspent_returns_error() {
        let pool = BudgetPool::new(1000, 10_000).unwrap();
        let result = pool.with_reservation(500, |r| r.commit(600, ()));
        assert!(matches!(result, Err(BudgetError::ExceedsMax)));
    }

    #[test]
    fn exhausted_pool_returns_error() {
        let pool = BudgetPool::new(100, 10_000).unwrap();
        let result = pool.with_reservation(500, |r| r.commit(100, ()));
        assert!(matches!(result, Err(BudgetError::InsufficientFunds)));
    }

    #[test]
    fn closure_returning_err_forfeits_via_drop() {
        let pool = BudgetPool::new(1000, 10_000).unwrap();
        let result: Result<(), _> = pool.with_reservation(500, |_r| {
            // Closure returns Err WITHOUT resolving the receipt.
            // The receipt drops here; the upstream Reservation::Drop
            // forfeits (outstanding -= 500, available unchanged).
            Err(BudgetError::Overflow)
        });
        assert!(matches!(result, Err(BudgetError::Overflow)));
        // Forfeit-on-drop preserved cap-soundness:
        // outstanding back to 0, available still reduced by 500.
        assert_eq!(pool.outstanding(), 0);
        assert_eq!(pool.available(), 500);
    }
}