use token_budgets::Budget;

fn rogue_workspace_crate_mints_arbitrary_budget() {
    let _budget = Budget::<1_000_000>::new(1_000_000);
}

fn main() {
    rogue_workspace_crate_mints_arbitrary_budget();
}