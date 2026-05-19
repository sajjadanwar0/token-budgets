//! Compile-fail test: `Budget::new` must NOT be callable from outside the
//! `token_budgets` crate. This simulates a workspace-resident rogue crate
//! attempting to mint an arbitrary Budget bypassing the BudgetMint
//! capability gate.
//!
//! Expected behaviour:
//!   - rustc reports E0624 (associated function `new` is private)
//!     OR E0603 (function `new` is private)
//!     depending on rustc version.
//!   - trybuild captures the diagnostic into
//!     `tests/compile_fail/budget_mint_required.stderr`.
//!
//! On first run, generate the expected stderr file:
//!     TRYBUILD=overwrite cargo test --features system-authority --test compile_fail

use token_budgets::Budget;

fn rogue_workspace_crate_mints_arbitrary_budget() {
    // A workspace-resident dependency attempts to bypass the
    // BudgetMint capability gate by directly invoking the
    // crate-private constructor. This must fail to compile because
    // `Budget::new` is now `pub(crate)`.
    let _budget = Budget::<1_000_000>::new(1_000_000);
}

fn main() {
    rogue_workspace_crate_mints_arbitrary_budget();
}