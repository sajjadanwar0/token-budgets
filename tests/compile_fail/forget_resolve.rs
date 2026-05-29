use token_budgets::pool_typestate::WithReservation;
use token_budgets::{BudgetError, BudgetPool};

fn main() {
    let pool = BudgetPool::new(1000, 10_000).unwrap();
    let result: Result<(), BudgetError> = pool.with_reservation(500, |_receipt| {
        Ok(())
    });

    let _ = result;
}