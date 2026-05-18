// tests/compile_fail/forget_resolve.rs
//
// Compile-fail test #8: demonstrates that the closure-based
// with_reservation pattern requires the closure to return a
// ResolvedReceipt<T>, which can only be produced by calling
// either .commit() or .cancel() on the receipt. A closure
// that "forgets" to resolve cannot type-check.
//
// Expected rejection: type mismatch — closure body does not
// return Result<ResolvedReceipt<T>, BudgetError>.
//
// This is the type-system fix for the latent receipt-leak.

use token_budgets::pool_typestate::WithReservation;
use token_budgets::{BudgetError, BudgetPool};

fn main() {
    let pool = BudgetPool::new(1000, 10_000).unwrap();

    // ERROR: the closure must call .commit() or .cancel() on
    // the receipt before returning. The expected return type
    // is Result<ResolvedReceipt<T>, BudgetError>, and
    // ResolvedReceipt<T> has no public constructor outside
    // the pool_typestate module.
    let result: Result<(), BudgetError> = pool.with_reservation(500, |_receipt| {
        // We "forget" to call .commit() or .cancel().
        // The closure returns Result<(), BudgetError> implicitly,
        // but the expected type is Result<ResolvedReceipt<()>, BudgetError>.
        Ok(())
    });

    let _ = result;
}