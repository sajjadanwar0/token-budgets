use token_budgets::{Budget, BudgetMint};

fn main() {
    let budget = { let mint = BudgetMint::take_authority(); Budget::<10000>::mint(&mint, 500) }.unwrap();
    let snapshot_ref = &budget;
    let (taken, kept) = budget.split(200).unwrap();
    println!("snapshot: {}", snapshot_ref.micro_cents());
    println!("taken={}, kept={}", taken.micro_cents(), kept.micro_cents());
}