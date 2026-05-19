// tests/compile_fail/clone_attempt.rs
//
// Compile-fail test #1: demonstrates that Budget does NOT
// implement Clone, so any attempt to duplicate it via .clone()
// is rejected by the trait-resolution machinery.
//
// Expected rejection: E0599 (no method named `clone` found for
// type `Budget<MAX>` in the current scope)
//
// This is the type-level no-duplication property: an affine
// resource cannot be copied. The borrow checker is not involved
// here — the rejection happens at trait resolution, before any
// borrow analysis runs.

use token_budgets::Budget;

fn main() {
    let budget = Budget::<10000>::new(500).unwrap();
    // ERROR: Budget<10000> does not implement Clone; the .clone()
    // method is not found in scope.
    let duplicate = budget.clone();

    // Sink uses so the compiler doesn't optimise the test away.
    let _ = budget.spend(100);
    let _ = duplicate.spend(100);
}