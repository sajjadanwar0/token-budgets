// tests/compile_fail/borrow_after_split.rs
//
// Compile-fail test #6 (NEW): demonstrates that holding an
// immutable reference to a Budget and then attempting to call
// split (which consumes the Budget by move) is rejected by
// the borrow checker.
//
// Expected rejection: E0505 (cannot move out of `budget` because it is borrowed)
//
// This catches the pattern where code reads Budget state (e.g.,
// to log the current balance) and then tries to mutate via
// split/merge/spend. The compiler enforces the affine discipline
// even when the user "promises" not to use the reference after
// the move — the borrow checker requires the borrow to end
// before the move can happen.

use token_budgets::Budget;

fn main() {
    let budget = Budget::<10000>::new(500).unwrap();
    let snapshot_ref = &budget;

    // ERROR: cannot move `budget` because it is still borrowed by snapshot_ref.
    let (taken, kept) = budget.split(200).unwrap();

    // This line forces snapshot_ref to remain live across the split,
    // making the error inevitable.
    println!("snapshot: {}", snapshot_ref.micro_cents());
    println!("taken={}, kept={}", taken.micro_cents(), kept.micro_cents());
}