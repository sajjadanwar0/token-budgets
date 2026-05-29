#[test]
fn sanity_test_binary_is_wired_up() {
    assert!(true);
}

#[cfg(loom)]
mod loom_tests {
    use loom::sync::atomic::{AtomicU64, Ordering};
    use loom::sync::Arc;
    use loom::thread;

    // std-side counter (NOT loom-instrumented) for interleaving tally.
    use std::sync::atomic::{AtomicUsize, Ordering as StdOrdering};

    use token_budgets::Budget;
use token_budgets::BudgetMint;

// Loom-test mint authority. Requires the `system-authority` feature.
#[cfg(feature = "system-authority")]
fn loom_mint() -> BudgetMint { BudgetMint::take_authority() }
    const TEST_MAX: u64 = 1000;
    static INTERLEAVINGS: AtomicUsize = AtomicUsize::new(0);
    
    fn loop_spend<const MAX: u64>(
        mut b: Budget<MAX>,
        each: u64,
        n: u64,
        total: &Arc<AtomicU64>,
    ) -> Budget<MAX> {
        for _ in 0..n {
            match b.spend(each) {
                Ok(after) => {
                    total.fetch_add(each, Ordering::SeqCst);
                    b = after;
                }
                Err(_) => {
                    return Budget::<MAX>::mint(&loom_mint(), 0).expect("zero budget");
                }
            }
        }
        b
    }
    
    #[test]
    fn loom_split_spawn_spend_merge() {
        INTERLEAVINGS.store(0, StdOrdering::SeqCst);
        loom::model(|| {
            INTERLEAVINGS.fetch_add(1, StdOrdering::SeqCst);

            let original = Budget::<TEST_MAX>::mint(&loom_mint(), 100).expect("init");
            let (child, parent) = original.split(40).expect("split");
            // child=40, parent=60

            let total_spent = Arc::new(AtomicU64::new(0));

            let total_for_child = Arc::clone(&total_spent);
            let child_handle = thread::spawn(move || -> Budget<TEST_MAX> {
                loop_spend(child, 10, 4, &total_for_child)
            });

            // Parent: 6 sequential spends of 10 each, concurrent with child.
            let parent_after = loop_spend(parent, 10, 6, &total_spent);

            let child_after = child_handle.join().expect("child panicked");
            let merged = parent_after.merge(child_after).expect("merge");

            let final_total = total_spent.load(Ordering::SeqCst);
            assert!(
                final_total <= 100,
                "CAP-RESPECTING VIOLATED: total spent {} > cap 100",
                final_total
            );
            assert_eq!(
                final_total, 100,
                "Expected all 10 spends to succeed (total=100), got {}",
                final_total
            );
            assert_eq!(
                merged.micro_cents(),
                0,
                "Conservation violated: merged.micro_cents={} expected 0",
                merged.micro_cents()
            );
        });
        let count = INTERLEAVINGS.load(StdOrdering::SeqCst);
        eprintln!(
            "LOOM_RESULT loom_split_spawn_spend_merge: {} interleavings explored",
            count
        );
    }
    
    #[test]
    fn loom_three_way_split_concurrent_spend() {
        INTERLEAVINGS.store(0, StdOrdering::SeqCst);
        loom::model(|| {
            INTERLEAVINGS.fetch_add(1, StdOrdering::SeqCst);

            let original = Budget::<TEST_MAX>::mint(&loom_mint(), 100).expect("init");
            let (child1, rest) = original.split(30).expect("split1");
            let (child2, parent) = rest.split(30).expect("split2");
            let total_spent = Arc::new(AtomicU64::new(0));

            let t1 = Arc::clone(&total_spent);
            let h1 = thread::spawn(move || -> Budget<TEST_MAX> {
                loop_spend(child1, 10, 3, &t1)
            });

            let t2 = Arc::clone(&total_spent);
            let h2 = thread::spawn(move || -> Budget<TEST_MAX> {
                loop_spend(child2, 10, 3, &t2)
            });

            let parent_after = loop_spend(parent, 10, 4, &total_spent);

            let c1_after = h1.join().expect("child1 panicked");
            let c2_after = h2.join().expect("child2 panicked");

            let merged = parent_after
                .merge(c1_after)
                .expect("merge c1")
                .merge(c2_after)
                .expect("merge c2");

            let final_total = total_spent.load(Ordering::SeqCst);
            assert!(
                final_total <= 100,
                "CAP-RESPECTING VIOLATED: total spent {} > cap 100",
                final_total
            );
            assert_eq!(
                final_total, 100,
                "Expected all 10 spends (3+3+4 of 10 each), got {}",
                final_total
            );
            assert_eq!(
                merged.micro_cents(),
                0,
                "Conservation: merged.micro_cents={} expected 0",
                merged.micro_cents()
            );
        });
        let count = INTERLEAVINGS.load(StdOrdering::SeqCst);
        eprintln!(
            "LOOM_RESULT loom_three_way_split_concurrent_spend: {} interleavings explored",
            count
        );
    }
    
    #[test]
    fn loom_split_drop_in_spawned_task() {
        INTERLEAVINGS.store(0, StdOrdering::SeqCst);
        loom::model(|| {
            INTERLEAVINGS.fetch_add(1, StdOrdering::SeqCst);

            let original = Budget::<TEST_MAX>::mint(&loom_mint(), 100).expect("init");
            let (child, parent) = original.split(30).expect("split");
            let total_spent = Arc::new(AtomicU64::new(0));

            let h = thread::spawn(move || {
                drop(child);
            });

            let parent_after = loop_spend(parent, 10, 7, &total_spent);

            h.join().expect("drop task panicked");

            let final_total = total_spent.load(Ordering::SeqCst);
            assert!(
                final_total <= 100,
                "CAP-RESPECTING VIOLATED: total spent {} > cap 100",
                final_total
            );
            assert_eq!(
                final_total, 70,
                "Expected parent's 7 spends of 10 (total=70), got {}",
                final_total
            );
            assert_eq!(
                parent_after.micro_cents(),
                0,
                "Parent micro_cents={} expected 0 (all 70 spent)",
                parent_after.micro_cents()
            );
        });
        let count = INTERLEAVINGS.load(StdOrdering::SeqCst);
        eprintln!(
            "LOOM_RESULT loom_split_drop_in_spawned_task: {} interleavings explored",
            count
        );
    }
}