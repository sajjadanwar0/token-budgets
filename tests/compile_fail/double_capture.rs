// tests/compile_fail/double_capture.rs
//
// Compile-fail test #4 (NEW): demonstrates that a Budget cannot
// be captured by two distinct closures because the first capture
// consumes the binding (FnOnce move semantics).
//
// Expected rejection: E0382 (use of moved value)
//
// This is the closure-level analogue of the double_spend.rs test
// (which catches sequential statement-level reuse). Here the
// closures may each be called only once, but creating two
// closures over the same Budget binding is already a violation.

use token_budgets::Budget;

fn main() {
    let budget = Budget::<10000>::new(500).unwrap();

    let closure_a = move || {
        let _ = budget.spend(100);
    };

    // ERROR: budget was moved into closure_a; the borrow checker
    // refuses this second move.
    let closure_b = move || {
        let _ = budget.spend(200);
    };

    closure_a();
    closure_b();
}