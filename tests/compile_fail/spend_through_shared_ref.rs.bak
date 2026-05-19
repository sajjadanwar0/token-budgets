// tests/compile_fail/spend_through_shared_ref.rs
//
// Compile-fail test #7 (NEW): demonstrates that a Budget owned
// by a struct cannot be spent via a shared (&) reference to that
// struct — spend consumes self by value, which requires
// ownership.
//
// Expected rejection: E0507 (cannot move out of `self.budget`
// which is behind a shared reference)
//
// This catches the pattern where a struct holds a Budget as a
// field and tries to call spend() via a method taking &self.
// The discipline forces the API designer to either consume the
// struct (`fn spend(self)`) or hold the Budget by Option<>
// with explicit take()/replace() semantics. Either is fine; the
// shared-reference shortcut is not.

use token_budgets::Budget;

struct Agent {
    budget: Budget<10000>,
}

impl Agent {
    fn try_spend(&self, amount: u64) -> Result<u64, &'static str> {
        // ERROR: cannot move `self.budget` out of `&self`.
        // The Budget::spend method consumes its self by value,
        // which is incompatible with a shared reference.
        match self.budget.spend(amount) {
            Ok(after) => Ok(after.micro_cents()),
            Err(_) => Err("insufficient funds"),
        }
    }
}

fn main() {
    let agent = Agent {
        budget: Budget::<10000>::new(500).unwrap(),
    };
    let _ = agent.try_spend(100);
}