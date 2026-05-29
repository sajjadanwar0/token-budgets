use token_budgets::{Budget, BudgetMint};

struct Agent {
    budget: Budget<10000>,
}

impl Agent {
    fn try_spend(&self, amount: u64) -> Result<u64, &'static str> {
        match self.budget.spend(amount) {
            Ok(after) => Ok(after.micro_cents()),
            Err(_) => Err("insufficient funds"),
        }
    }
}

fn main() {
    let agent = Agent {
        budget: { let mint = BudgetMint::take_authority(); Budget::<10000>::mint(&mint, 500) }.unwrap(),
    };
    let _ = agent.try_spend(100);
}