// tests/compile_fail/double_spend.rs
//
// Compile-fail test #2: demonstrates that the same Budget
// binding cannot be spent twice. The first spend() consumes
// self by value; the second use is rejected as use-after-move.
//
// Expected rejection: E0382 (use of moved value: `budget`)
//
// This is the affine no-double-use property: once a Budget is
// spent, the binding is consumed. To continue spending, the
// caller must use the returned new Budget from spend's Ok arm.

use token_budgets::Budget;

fn main() {
    let budget = Budget::<10000>::new(500).unwrap();

    // First spend consumes `budget` by value.
    let _ = budget.spend(100);

    // ERROR: `budget` was moved into the first spend() call;
    // it is no longer accessible here.
    let _ = budget.spend(200);
}