# token-budgets

Compile-time affine typing for LLM cost caps in Rust. The main library crate.

This is the implementation accompanying the paper *Token Budgets: A 4-Tier Formal Verification of Compile-Time Affine Typing for LLM Cost Caps*, currently under review at *Empirical Software Engineering*.

## What this crate gives you

A `Budget<T>` type that you cannot duplicate, cannot use twice, and cannot exceed:

```rust
use token_budgets::{Budget, BudgetError};

let b = Budget::new(1000)?;          // construct with cap of 1000 tokens
let (lhs, rhs) = b.split(400)?;      // splits into two budgets summing to 1000
lhs.spend(350)?;                      // consume 350, returns Receipt
// lhs.spend(100)?;                   // COMPILE ERROR: lhs was moved
```

The type system enforces that:

- A `Budget` cannot be cloned (no `Clone` impl). Once consumed, it's gone.
- Sub-budgets sum to ≤ the parent (`split` and `merge` are conservation laws).
- A spent budget yields a `Receipt`, not a re-usable balance.
- A pool with reservations cannot oversubscribe (`Pool` typestate machine).

This sits one layer below `tokio` and one layer above your model client (Anthropic, OpenAI, Mistral, etc.).

## Quick start

```toml
[dependencies]
token-budgets = "0.5"
```

```rust
use token_budgets::{Budget, AnthropicEstimator, TokenEstimator};

let budget = Budget::new(5000)?;
let estimator = AnthropicEstimator::default();      // 2.0× margin (load-bearing; §5.22)
let estimate = estimator.estimate(&prompt);
let (for_call, remaining) = budget.split(estimate)?;
let response = call_model(prompt, &for_call).await?;
let receipt = for_call.spend(response.tokens_used)?;
```

## Reproducibility (paper Table 21 / Table 30)

This repository ships `reproduce.sh`, which clones the 5 companion repositories and runs every reproducible check from the paper end-to-end. From a clean workspace:

```bash
git clone https://github.com/sajjadanwar0/token-budgets
cd token-budgets
WORKDIR=/tmp/tb-reproduce bash reproduce.sh 2>&1 | tee reproduction.log
```

Expected output: **33 PASS / 0 FAIL / 1 SKIP** (live-API sweeps are opt-in via `--live`). The single SKIP is the live-API replay (~$1 to run; pass `--live` with `OPENAI_API_KEY` and `ANTHROPIC_API_KEY` set).

Optional flags:

```
--skip-{verus,coq,tla,dafny,bench,loom,irr,python,experiments}
--rerun-loom           # attempt fresh Loom rebuild (see Known Issues)
--live                 # run live-API multi-runtime sweep
```

## Microbenchmarks (Table 21)

```bash
cargo bench
```

Operations on a 2026 Apple M3, release build, criterion 0.8.2:

| Operation             | Median time | 95% CI               |
|-----------------------|-------------|----------------------|
| `Budget::new`         | 687 ps      | [686, 688] ps        |
| `Budget::spend(ok)`   | 1.13 ns     | [1.12, 1.13] ns      |
| `Budget::spend(fail)` | 1.13 ns     | [1.12, 1.13] ns      |
| `Budget::merge`       | 1.38 ns     | [1.38, 1.38] ns      |
| `Budget::split`       | 4.75 ns     | [4.74, 4.75] ns      |

These have been reproduced across 8 independent runs; the variance between runs is within Criterion's ±5% noise band.

## Loom interleaving evidence

The `tests/loom_concurrent.rs` test exercises the `Pool` typestate under loom's exhaustive scheduler. Shipped logs in `loom_run*.log` document 5,957 total interleavings explored across four sweeps with zero cap violations.

To re-run: `RUSTFLAGS="--cfg loom" cargo test --test loom_concurrent --release`. The current `Cargo.toml` makes `tokio` a `cfg(not(loom))` dependency to work around a tokio-1.52 + loom-0.7 feature-gate regression in tokio's `task/local.rs`. Without that gating, tokio is pulled into the loom build and fails to compile due to an unrelated tokio-internal issue.

## Companion repositories

- [token-budgets-formals](https://github.com/sajjadanwar0/token-budgets-formals) — 4-tier formal verification (Verus 66 obligations, Coq, TLA+, Dafny) and the IRR study (codebook, rater brief, blinded coding sheet, completed annotations, κ = 0.832 on N = 109).
- [token-budgets-experiments](https://github.com/sajjadanwar0/token-budgets-experiments) — multi-runtime evaluation harness, fair-baseline corpus, refund-live results, A1 calibration, Conjecture 1 stress sweep.
- [token-budgets-baseline](https://github.com/sajjadanwar0/token-budgets-baseline) — fair-baseline candidate corpus.
- [token-budgets-python](https://github.com/sajjadanwar0/token-budgets-python) — Python port with LANG-001 reproductions.
- [token-budgets-extensions](https://github.com/sajjadanwar0/token-budgets-extensions) — adaptive estimator and Verus skeleton for extensions.

## Known issues

These are acknowledged in the paper, not pretended away:

- **Conjecture 1** (operational refinement to the running Tokio binary) is open. The abstract-trace cap-soundness theorem is mechanized in Verus (66 obligations, 0 errors).
- **Assumption A1** (UTF-8 byte-length dominance) is calibrated with a 2.0× margin, not formally proven. The margin is load-bearing — at margin 1.0, A1 holds only 1/3 of cells; at margin 2.0, A1 holds 30/30. Mechanized proof over a defined class of BPE tokenizers is identified as future work (§5.22).
- **Loom fresh rebuild** is blocked by a tokio-1.52 + loom-0.7 feature-gate incompatibility in tokio's `task/local.rs`. The shipped `loom_run*.log` files are the authoritative artifact for the 5,957-interleavings claim.

## Citation

```bibtex
@unpublished{khan2026tokenbudgets,
  author       = {Khan, Sajjad},
  title        = {Token Budgets: A 4-Tier Formal Verification of Compile-Time Affine
                  Typing for LLM Cost Caps, with a 109-Case Empirical Catalog and
                  6-Runtime Production-Tier Evaluation},
  year         = {2026},
  note         = {Under review at Empirical Software Engineering. Reproduction
                  artifact at \url{https://github.com/sajjadanwar0/token-budgets}.}
}
```

## License

[Add license. Apache-2.0 OR MIT recommended for Rust crates.]

## Contact

Sajjad Khan — [email] — [https://sajjadanwar.io](https://sajjadanwar.io)