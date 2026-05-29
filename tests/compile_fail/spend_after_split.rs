use token_budgets::{Budget, BudgetMint};

fn main() {
    let budget = { let mint = BudgetMint::take_authority(); Budget::<10000>::mint(&mint, 500) }.unwrap();
    let (taken, kept) = budget.split(200).unwrap();
    let _ = budget.spend(50);
    let _ = taken.micro_cents();
    let _ = kept.micro_cents();
}