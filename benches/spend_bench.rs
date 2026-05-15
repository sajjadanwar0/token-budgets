//! Microbenchmark for `Budget::spend` with proper `black_box` discipline.
//!
//! This benchmark addresses a methodological concern from reviewers of the
//! main Token Budgets paper: the original paper reported `~900 ps` for
//! `Budget::spend` (success path), which is **below L1 cache access time**
//! on modern x86 and therefore physically implausible for a real call
//! that does a `checked_sub` plus branch plus `Result` wrapping. The
//! probable cause is constant folding by LLVM with loop unrolling and
//! result discarding.
//!
//! This benchmark uses `std::hint::black_box` aggressively to defeat the
//! optimizer:
//!
//! - **Inputs are black-boxed** so the compiler cannot fold them to
//!   constants and pre-compute the result at compile time.
//! - **Outputs are black-boxed** (via the closure return) so the
//!   compiler cannot eliminate the call as dead code.
//! - **The budget is reconstructed on every iteration** (also through
//!   `black_box`) to prevent LLVM from hoisting the cap check or
//!   reusing the result.
//!
//! ## How to interpret results
//!
//! Run with:
//!
//! ```text
//! cargo bench --bench spend_bench
//! ```
//!
//! Expected order-of-magnitude: 1-10 ns on a modern x86 / Apple Silicon
//! CPU. Numbers below 1 ns indicate the benchmark is still being folded;
//! numbers above 100 ns indicate the benchmark is measuring something
//! other than the operation under test.
//!
//! ## To inspect assembly
//!
//! ```text
//! cargo install cargo-show-asm
//! cargo asm --bench spend_bench --rust 'spend_bench::bench_spend_success::*'
//! ```
//!
//! The generated assembly for `Budget::spend` should show: a compare, a
//! conditional branch, a subtraction, and a `Result::Ok` packing. Total
//! ~5-10 instructions, executing in ~1-3 cycles on superscalar hardware,
//! which is ~0.3-1.0 ns at 3 GHz.

use token_budgets::Budget;
use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
/// $1000 in micro-cents; well below the A2 ceiling.
type B = Budget<1_000_000_000>;

/// Benchmark `Budget::spend` on the success path.
///
/// Every input is run through `black_box` to defeat constant folding.
/// The Budget is reconstructed in every iteration; otherwise LLVM hoists
/// the construction out of the loop and we measure only `spend`.
fn bench_spend_success(c: &mut Criterion) {
    c.bench_function("spend_success", |bench| {
        bench.iter(|| {
            let budget = B::new(black_box(1_000_000_000)).unwrap();
            // black_box the spend amount AND the budget so neither
            // can be folded; black_box the result so the call isn't
            // dead-code eliminated.
            let result = black_box(budget).spend(black_box(1));
            black_box(result.unwrap())
        });
    });
}

/// Benchmark `Budget::spend` on the failure path (insufficient funds).
fn bench_spend_failure(c: &mut Criterion) {
    c.bench_function("spend_failure", |bench| {
        bench.iter(|| {
            let budget = B::new(black_box(100)).unwrap();
            // amount > budget, so spend returns InsufficientFunds.
            let result = black_box(budget).spend(black_box(101));
            // We expect Err here; black_box it to keep it.
            black_box(result.unwrap_err())
        });
    });
}

/// Benchmark `Budget::new` (construction with A2 check).
fn bench_new(c: &mut Criterion) {
    c.bench_function("new", |bench| {
        bench.iter(|| {
            let result = B::new(black_box(500_000_000));
            black_box(result.unwrap())
        });
    });
}

/// Benchmark `Budget::merge` (consumes both, allocates a new Budget).
fn bench_merge(c: &mut Criterion) {
    c.bench_function("merge", |bench| {
        bench.iter(|| {
            let a = B::new(black_box(300_000_000)).unwrap();
            let b = B::new(black_box(400_000_000)).unwrap();
            let result = black_box(a).merge(black_box(b));
            black_box(result.unwrap())
        });
    });
}

/// Benchmark `Budget::split` (one input, two outputs).
fn bench_split(c: &mut Criterion) {
    c.bench_function("split", |bench| {
        bench.iter(|| {
            let budget = B::new(black_box(1_000_000_000)).unwrap();
            let result = black_box(budget).split(black_box(300_000_000));
            let (a, b) = result.unwrap();
            black_box((a, b))
        });
    });
}

criterion_group!(
    benches,
    bench_spend_success,
    bench_spend_failure,
    bench_new,
    bench_merge,
    bench_split,
);
criterion_main!(benches);
