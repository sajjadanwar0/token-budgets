use token_budgets::{Budget, BudgetMint};

fn main() {
    let budget = { let mint = BudgetMint::take_authority(); Budget::<10000>::mint(&mint, 500) }.unwrap();

    let closure_a = move || {
        let _ = budget.spend(100);
    };

    let closure_b = move || {
        let _ = budget.spend(200);
    };

    closure_a();
    closure_b();
}