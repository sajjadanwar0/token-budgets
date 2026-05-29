use token_budgets::{Budget, BudgetMint};
use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;

type B = Budget<1_000_000_000>;

fn bench_spend_success(c: &mut Criterion) {
    let mint = BudgetMint::take_authority();
    c.bench_function("spend_success", |bench| {
        bench.iter(|| {
            let budget = B::mint(&mint, black_box(1_000_000_000)).unwrap();
            let result = black_box(budget).spend(black_box(1));
            black_box(result.unwrap())
        });
    });
}

fn bench_spend_failure(c: &mut Criterion) {
    let mint = BudgetMint::take_authority();
    c.bench_function("spend_failure", |bench| {
        bench.iter(|| {
            let budget = B::mint(&mint, black_box(100)).unwrap();
            let result = black_box(budget).spend(black_box(101));
            black_box(result.unwrap_err())
        });
    });
}

fn bench_new(c: &mut Criterion) {
    let mint = BudgetMint::take_authority();
    c.bench_function("new", |bench| {
        bench.iter(|| {
            let result = B::mint(&mint, black_box(500_000_000));
            black_box(result.unwrap())
        });
    });
}

fn bench_merge(c: &mut Criterion) {
    let mint = BudgetMint::take_authority();
    c.bench_function("merge", |bench| {
        bench.iter(|| {
            let a = B::mint(&mint, black_box(300_000_000)).unwrap();
            let b = B::mint(&mint, black_box(400_000_000)).unwrap();
            let result = black_box(a).merge(black_box(b));
            black_box(result.unwrap())
        });
    });
}

fn bench_split(c: &mut Criterion) {
    let mint = BudgetMint::take_authority();
    c.bench_function("split", |bench| {
        bench.iter(|| {
            let budget = B::mint(&mint, black_box(1_000_000_000)).unwrap();
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