use crate::{BudgetError, BudgetPool, Reservation};

#[must_use = "ResolvedReceipt should be returned from the with_reservation closure"]
pub struct ResolvedReceipt<T> {
    inner: T,
    _private: PrivateMarker,
}

struct PrivateMarker;

impl<T> ResolvedReceipt<T> {
    fn new(inner: T) -> Self {
        Self { inner, _private: PrivateMarker }
    }
    pub fn into_inner(self) -> T {
        self.inner
    }
}

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

    pub fn commit<T>(mut self, actual: u64, value: T) -> Result<ResolvedReceipt<T>, BudgetError> {
        let res = self.inner.take().expect("invariant: inner present until consumed");
        res.commit(actual)?;
        Ok(ResolvedReceipt::new(value))
    }

    pub fn cancel<T>(mut self, value: T) -> ResolvedReceipt<T> {
        let res = self.inner.take().expect("invariant: inner present until consumed");
        res.cancel();
        ResolvedReceipt::new(value)
    }
}

pub trait WithReservation {
    fn with_reservation<F, T>(&self, amount: u64, f: F) -> Result<T, BudgetError>
    where
        F: FnOnce(ReservationReceipt) -> Result<ResolvedReceipt<T>, BudgetError>;
}

impl WithReservation for BudgetPool {
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
            Err(BudgetError::Overflow)
        });
        assert!(matches!(result, Err(BudgetError::Overflow)));
        assert_eq!(pool.outstanding(), 0);
        assert_eq!(pool.available(), 500);
    }
}