use std::rc::Rc;
use std::thread;
use token_budgets::{Budget, BudgetMint};

fn main() {
    let budget = { let mint = BudgetMint::take_authority(); Budget::<10000>::mint(&mint, 500) }.unwrap();
    let shared = Rc::new(budget);
    let s1 = Rc::clone(&shared);
    let handle = thread::spawn(move || {
        let _ = s1.micro_cents();
    });

    handle.join().unwrap();
}