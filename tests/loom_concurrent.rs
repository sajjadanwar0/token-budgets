//! Loom B1: split-spawn-merge concurrent invariant validation.
//!
//! Validates the cap-respecting invariant under exhaustive thread
//! interleavings. The Budget<const MAX: u64> type is affine -- it
//! cannot be aliased across threads at the type level (compile-time
//! guarantee from the Rust borrow checker). The runtime concurrency
//! question is whether the split-spawn-merge pattern preserves the
//! cap-respecting bound across all valid interleavings:
//!
//!   Given:      original Budget with B_0 micro_cents
//!   Operations: split into (taken, kept), spawn child task with
//!               one half, both spend concurrently on owned Budgets,
//!               join, merge child back to parent.
//!   Invariant:  sum of all successful spends <= B_0
//!               merged.micro_cents() == B_0 - total_spent
//!
//! Each thread executes MULTIPLE sequential spend operations on its
//! owned Budget, each followed by an atomic update of a shared
//! `Arc<AtomicU64>` observer. The Budgets themselves are NOT shared
//! between threads; only the observer is. Loom's exhaustive search
//! explores all interleavings of the atomic operations plus the
//! spawn/join synchronisation boundaries.
//!
//! Real Budget API used (from src/lib.rs):
//!   Budget::<MAX>::mint(&loom_mint(), micro_cents)         -> Result<Self, BudgetError>
//!   Budget<MAX>::split(self, amount)        -> Result<(Self, Self), BudgetError>
//!                                              returns (taken, kept)
//!   Budget<MAX>::merge(self, other)         -> Result<Self, BudgetError>
//!   Budget<MAX>::spend(self, amount)        -> Result<Self, BudgetError>
//!   Budget<MAX>::micro_cents(&self)         -> u64
//!
//! Run with:
//!     CARGO_TARGET_DIR=target-loom \
//!     LOOM_MAX_PREEMPTIONS=4 \
//!     RUSTFLAGS="--cfg loom" \
//!     cargo test --release --test loom_concurrent -- --nocapture

/// Sanity test - ALWAYS runs, even without --cfg loom. Used to verify
/// the test binary is built and discovered correctly.
#[test]
fn sanity_test_binary_is_wired_up() {
    assert!(true);
}

// =============================================================
// Loom-only tests. Module-gated so the loom dependency is only
// required when --cfg loom is set.
// =============================================================

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

    /// Type-level MAX overflow bound. Comfortably exceeds runtime caps.
    const TEST_MAX: u64 = 1000;

    /// Per-test interleaving counter. Reset and incremented per test.
    static INTERLEAVINGS: AtomicUsize = AtomicUsize::new(0);

    /// Helper: run `n` sequential spends of `each` micro_cents on the
    /// given Budget, incrementing the shared total on each success.
    /// Returns the resulting Budget (possibly zero-cap if any spend
    /// failed). Multiple sequential spends per thread give Loom a
    /// richer state space to explore than a single-spend pattern.
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

    // ---------------------------------------------------------
    // Test 1: split-spawn-spend-merge (canonical B1 pattern)
    //   Original cap=100, split (40, 60) into (child, parent).
    //   Child:  4 spends of 10 each on its 40 cap (4 fetch_adds).
    //   Parent: 6 spends of 10 each on its 60 cap (6 fetch_adds).
    //   Total: 10 fetch_adds across 2 threads. Loom explores
    //   orderings of all atomic ops plus spawn/join boundaries.
    //   Expected: all 10 spends succeed, total=100, merged=0.
    // ---------------------------------------------------------
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

    // ---------------------------------------------------------
    // Test 2: three-way split with concurrent spend loops
    //   Original cap=100, split into (30, 30, 40) for child1, child2, parent.
    //   child1: 3 spends of 10 each (3 fetch_adds).
    //   child2: 3 spends of 10 each (3 fetch_adds).
    //   parent: 4 spends of 10 each (4 fetch_adds).
    //   Total: 10 fetch_adds across 3 threads. The multinomial
    //   coefficient on operation orderings dominates the state space;
    //   this test produces substantially more interleavings than Test 1.
    // ---------------------------------------------------------
    #[test]
    fn loom_three_way_split_concurrent_spend() {
        INTERLEAVINGS.store(0, StdOrdering::SeqCst);
        loom::model(|| {
            INTERLEAVINGS.fetch_add(1, StdOrdering::SeqCst);

            let original = Budget::<TEST_MAX>::mint(&loom_mint(), 100).expect("init");
            let (child1, rest) = original.split(30).expect("split1");
            let (child2, parent) = rest.split(30).expect("split2");
            // child1=30, child2=30, parent=40

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

    // ---------------------------------------------------------
    // Test 3: drop in spawned task (cancelled-child semantics)
    //   Original cap=100, split (30, 70) into (child, parent).
    //   Spawn task that drops child without spending (simulates a
    //     cancelled subtask).
    //   Parent does 7 spends of 10 each on its 70 cap.
    //
    //   Verifies:
    //   (a) Dropping a child Budget does NOT refund to the parent
    //       (no shared state path between them after split).
    //   (b) Parent's spends proceed independently within its own cap.
    //   (c) Cancellation safety: a task aborting mid-flight cannot
    //       corrupt the parent's accounting.
    //
    //   The interleaving count for this test is small because the
    //   spawned task has no atomic operations (just a drop); Loom
    //   has minimal shared-state ordering to explore.
    // ---------------------------------------------------------
    #[test]
    fn loom_split_drop_in_spawned_task() {
        INTERLEAVINGS.store(0, StdOrdering::SeqCst);
        loom::model(|| {
            INTERLEAVINGS.fetch_add(1, StdOrdering::SeqCst);

            let original = Budget::<TEST_MAX>::mint(&loom_mint(), 100).expect("init");
            let (child, parent) = original.split(30).expect("split");
            // child=30, parent=70

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
