# token-budgets

Compile-time affine typing for LLM cost caps in Rust. The main library crate.

This is the implementation accompanying the paper *Token Budgets: An Empirical
Catalog of 63 LLM-Agent Budget-Overrun Incidents, with an Affine-Typed Rust
Mitigation as a Case Study* (preprint, 2026).

## What this crate gives you

A `Budget` type that you cannot duplicate, cannot use twice, and cannot exceed:

```rust
use token_budgets::{Budget, BudgetError, BudgetMint};

// Construction is gated behind the `system-authority` feature (see Cargo.toml).

let mint = BudgetMint::take_authority();
let b: Budget = Budget::new(&mint, 1000)?;   // cap of 1000 micro-cents

let (parent, child) = b.split(400)?;         // two budgets summing to 1000
let parent = parent.spend(350)?;             // consume 350; returns remainder Budget
// let _ = parent.spend(100)?;               // would ALSO move `parent`;
//                                           // the prior binding is gone (E0382)
```

> **Note**: `Budget::new` is gated behind the `BudgetMint` capability token
> (constructed via `BudgetMint::take_authority()`), itself behind the
> `system-authority` Cargo feature. See `docs/trust-boundary.md` for the threat
> model.

The type system enforces that:

- A `Budget` cannot be cloned (no `Clone` impl). Once consumed, it's gone.
- Sub-budgets sum to the parent (`split` and `merge` are conservation laws).
- `spend(amount)` consumes `self` and returns a **new** `Budget` carrying the
  remainder; the prior binding is moved and cannot be used again. A separate
  `spend_with_receipt(amount)` returns a `ReservationReceipt` for the
  reserve-then-reconcile refund path.
- A pool with reservations cannot oversubscribe (`Pool` typestate machine).

This sits one layer below `tokio` and one layer above your model client.

## Quick start

```toml
[dependencies]
token-budgets = "0.5"
```

```rust
use token_budgets::{Budget, BudgetMint, AnthropicEstimator, TokenEstimator};

let mint = BudgetMint::take_authority();
let budget: Budget = Budget::new(&mint, 5000)?;
let estimator = AnthropicEstimator::default();      // 2.0x margin (load-bearing; paper §5.30)
let estimate = estimator.estimate(&prompt);
let (for_call, remaining) = budget.split(estimate)?;
let response = call_model(prompt, &for_call).await?;
let for_call = for_call.spend(response.tokens_used)?; // returns the remaining Budget
```

## Reproducibility

This repository ships `reproduce.sh`. The anonymised artifact bundles all five
component directories as siblings of the script (no network or GitHub account
required):

```bash
bash reproduce.sh                 # offline replication (~10 min)
bash reproduce.sh --with-live     # also run live-API cells (~$0.50, 30 min)
bash reproduce.sh --formal-only   # only verify formal proofs (~5 min)
```

`reproduce.sh` audits **20 paper-backing claims** (catalogue size, A1 calibration,
the IRR κ, the trybuild rustc-code coverage, the Forgetful-Operator Condition-E
result, the A7 fault-injection table, and more), compiles the formal proofs
(Coq/Dafny; Verus optional), runs the offline microbenchmarks, and optionally
runs the live-API replication. Live-API smoke-test cost is under $0.005.

## Microbenchmarks

```bash
cargo bench --features system-authority --bench spend_bench
```

Operations on AMD Ryzen 7 PRO 6850U, Linux (Ubuntu), rustc 1.93.1 stable,
release build, Criterion (median of 100 samples):

| Operation             | Median time | 95% CI               |
|-----------------------|-------------|----------------------|
| `Budget::spend` (ok)  | 1.180 ns    | [1.177, 1.184] ns    |
| `Budget::spend` (err) | 1.198 ns    | [1.190, 1.209] ns    |
| `Budget::merge`       | 1.447 ns    | —                    |
| `Budget::split`       | 4.836 ns    | —                    |

Sub-2-ns figures are below the noise floor of a non-optimised call (LLVM
partially folds even under `black_box`); the load-bearing claim is that
per-operation overhead is negligible relative to LLM-API network latency
(< 0.001%).

## Loom interleaving evidence

`tests/loom_concurrent.rs` exercises the concurrent split/merge/spend/Drop
operations under Loom's exhaustive scheduler. At `LOOM_MAX_PREEMPTIONS=4`, Loom
enumerates **5,966 distinct interleavings** with zero assertion failures
(spot-checked at preemption 5, ~32k schedules, still zero). Loom is bounded
model checking, not proof.

To re-run:

```bash
RUSTFLAGS="--cfg loom" LOOM_MAX_PREEMPTIONS=4 \
  cargo test --release --target-dir target-loom \
  --features system-authority --test loom_concurrent
```

## Companion components (single artifact bundle)

The replication artifact is organised as five sibling directories:

- `token-budgets` — main library and the 110-row catalogue (`data/catalogue.csv`).
- `token-budgets-formals` — formal verification (Verus 66 obligations; TLAPS 497
  obligations; TLC 252 states at B0=5; Coq and Dafny re-encodings) plus the IRR
  package (codebook v1.0, blinded coding sheets, **κ = 0.837 on N = 113**).
- `token-budgets-experiments` — multi-runtime evaluation harness, refund-live
  results, A1 calibration, A7 fault injection.
- `token-budgets-python` — runtime-only Python port (no compile-time guarantees).
- `token-budgets-extensions` — adaptive estimator and Verus skeleton.

## Known issues

Acknowledged in the paper, not pretended away:

- **Conjecture 1** (binary-level cap-soundness on the running Tokio binary) is
  **open**. The abstract-machine cap-soundness is cross-checked by TLAPS (497
  obligations) and TLC, with a preliminary Verus source-level mechanisation (66
  obligations, 0 errors). Closing Conjecture 1 is an estimated ~12 person-months
  of Iris/RustBelt work.
- **Assumption A1** (UTF-8 byte-length dominance) is calibrated with a 2.0×
  margin, not formally proven. The margin is load-bearing: at margin 1.0, A1
  holds on 1/3 of the audited classes; at margin 2.0, A1 holds 30/30.
- **Loom fresh rebuild** can hit a tokio + loom feature-gate incompatibility;
  the shipped `loom_run*.log` files are the authoritative artifact for the
  interleavings claim.

## Citation

```bibtex
@unpublished{khan2026tokenbudgets,
  author = {Khan, Sajjad},
  title  = {Token Budgets: An Empirical Catalog of 63 LLM-Agent Budget-Overrun
            Incidents, with an Affine-Typed Rust Mitigation as a Case Study},
  year   = {2026},
  note   = {Preprint. Reproduction artifact at
            \url{https://github.com/sajjadanwar0/token-budgets}.}
}
```

## License

Dual MIT/Apache-2.0. See `LICENSE-MIT` and `LICENSE-APACHE`.

## Contact

Sajjad Khan — sajjadanwar200@gmail.com