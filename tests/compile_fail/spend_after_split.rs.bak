// tests/compile_fail/spend_after_split.rs
//
// Compile-fail test #3: demonstrates that the original Budget
// binding cannot be used after split() has consumed it. The
// caller must use the (taken, kept) tuple returned from split.
//
// Expected rejection: E0382 (use of moved value: `budget`)
//
// This is the conservation property at the type level: after
// split, the original capacity exists only as the sum of the
// two child Budgets. Attempting to use the parent binding is
// rejected because split consumed it by value.

use token_budgets::Budget;

fn main() {
    let budget = Budget::<10000>::new(500).unwrap();

    // split consumes `budget` by value, returns (taken, kept).
    let (taken, kept) = budget.split(200).unwrap();

    // ERROR: `budget` was moved into the split() call; it is
    // no longer accessible here.
    let _ = budget.spend(50);

    // Sink uses for taken and kept so the compiler doesn't
    // optimize them away.
    let _ = taken.micro_cents();
    let _ = kept.micro_cents();
}