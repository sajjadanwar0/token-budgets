# Token Budgets

> An affine-resource discipline for LLM cost caps in Rust.
> Type-level integrity of resource accounting combined with runtime
> cap arithmetic. The `Budget` type is non-cloneable and its spend
> operation consumes it by value, producing a `Receipt` that must be
> resolved into either a confirmed refund or a forfeit.

[![TLAPS](https://img.shields.io/badge/TLAPS-497_obligations-brightgreen)](https://github.com/sajjadanwar0/token-budgets-formals)
[![TLC](https://img.shields.io/badge/TLC-252_states-brightgreen)](https://github.com/sajjadanwar0/token-budgets-formals)
[![Dafny](https://img.shields.io/badge/Dafny-23_verified-brightgreen)](https://github.com/sajjadanwar0/token-budgets-formals)
[![Coq](https://img.shields.io/badge/Coq-0_Admitted-brightgreen)](https://github.com/sajjadanwar0/token-budgets-formals)
[![Verus](https://img.shields.io/badge/Verus-66_theorems-brightgreen)](https://github.com/sajjadanwar0/token-budgets-formals)
[![Catalog](https://img.shields.io/badge/catalog-167_failures_/_23_frameworks-blue)](#catalog-of-real-world-failures)
[![License](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-blue)](LICENSE-MIT)

This is the main Rust library of the **Token Budgets** project, an
EMSE-submission artifact. The discipline addresses a documented class
of agent failures where the absence of a first-class cost-bounding
primitive leads to runaway LLM spend (see
[catalog](#catalog-of-real-world-failures)).

The full artifact is a 5-repo set; this is the entry point. See
[Related repositories](#related-repositories) below for the others.

## What it provides

A small affine-typed API for **financial** cost caps (denominated in
nano-cents — 1 nc = $0.00000001):

```rust
use token_budgets::{Budget, ByteLength};

// Compile-time cap. Const-generic guarantees A2 (no u64 overflow on spend).
const CAP: u64 = 1_000_000_000;  // $10 in nano-cents

let budget = Budget::<CAP>::new(CAP)?;

// The affine consume-by-value spend operation
let (after_reserve, receipt) = budget.spend_with_receipt(50_000)?;

// Some time later, after a provider call returns actual usage:
let refund = receipt.confirm(actual_cost_nc)?;
let final_budget = refund.apply_to(after_reserve)?;
```

The compiler enforces that **each `Budget` is consumed exactly once**
per operation and **each `Receipt` is resolved exactly once** (either
`confirm` or `forfeit`, not both, never neither).

## Why a new primitive

The catalog documents **167 production incidents across 23 frameworks
and 18 ecosystems**, including:

- Compaction-loop runaway ($235 in 4 days, claude-code #9579)
- Tool-call hard-cliffs that fire after useful work was already done
  (pydantic-ai #4359: "kill switch... the entire run dies")
- Multi-tenant fanout amplification (CrewAI #2655: >1M tokens
  re-embedded per crew run)
- Provider-asymmetric cache double-counting (pydantic-ai #4364)
- Tool-call argument truncation when output tokens hit max
  (pydantic-ai #3118, Anthropic-only)

Token Budgets addresses these structurally: each call goes through a
receipt/refund cycle that reconciles reserved vs actual usage and
enforces the cap as a type-level invariant.

## API surface

### Core types

| Type | Purpose | Notes |
|---|---|---|
| `Budget<const MAX: u64>` | Affine financial cap | Const-generic MAX enforces A2 (overflow-free) |
| `Receipt<const MAX: u64>` | In-flight reservation | Must be `confirm`ed or `forfeit`ed exactly once |
| `Refund<const MAX: u64>` | Confirmed-but-unrefunded slack | Applied back to a `Budget` via `apply_to` |
| `StreamingReceipt<const MAX: u64>` | Per-chunk refund for streaming | Interim chunks via `confirm_chunk`, finalize via `close` |
| `BudgetPool` | Multi-tenant sharded budget | For pool-of-budgets workloads |
| `Reservation` | Pool-side in-flight reservation | Drop-safe with auto-refund on unwind |
| `CapAuthority` | Sealed-cap binding | Operator-trust-boundary enforcement via `seal_at_startup()` |
| `ReasoningProvider` | Enum for reasoning-model accounting | Used by `spend_with_reasoning` for o3-mini, DeepSeek-R1, etc. |

### Budget spend variants

```rust
// 1. Simple spend (consumes Budget, returns new Budget)
budget.spend(amount) -> Result<Self, BudgetError>

// 2. Spend with receipt (the affine reserve→confirm cycle)
budget.spend_with_receipt(reserved) -> Result<(Self, Receipt<MAX>), BudgetError>

// 3. Streaming spend (per-chunk refund)
budget.spend_streaming(reserved) -> Result<(Self, StreamingReceipt<MAX>), BudgetError>

// 4. Reasoning-aware spend (separate invisible reasoning tokens)
budget.spend_with_reasoning(visible_estimate, provider) -> Result<(Self, Receipt<MAX>), BudgetError>

// Plus pool/split/merge:
budget.split(amount) -> Result<(Self, Self), BudgetError>
budget.merge(other) -> Result<Self, BudgetError>
```

### Estimators

| Estimator | Source | When to use |
|---|---|---|
| `ByteLength` | Always available | Sound upper bound (A1: byte-length dominance) |
| `Tiktoken` | Feature `tiktoken` | Tighter reservation for OpenAI-tokenizer models |

Activate the tiktoken estimator:
```toml
[dependencies]
token-budgets = { version = "0.5", features = ["tiktoken"] }
```

## Worked example

```rust
use token_budgets::{Budget, ByteLength, TokenEstimator};

const CAP: u64 = 1_000_000_000;  // $10 cap

fn budget_aware_call(
    budget: Budget<CAP>,
    prompt: &str,
    input_price_nc: u64,
    output_price_nc: u64,
    max_tokens: u32,
) -> Result<Budget<CAP>, Box<dyn std::error::Error>> {
    // 1. Estimate (sound upper bound via byte-length / A1)
    let est_in_tokens = ByteLength.estimate(prompt);
    let reservation_nc = est_in_tokens * input_price_nc
        + (max_tokens as u64) * output_price_nc;

    // 2. Reserve (consumes budget by value)
    let (after_reserve, receipt) = budget.spend_with_receipt(reservation_nc)?;

    // 3. Perform the actual provider call (placeholder)
    let actual_in_tokens: u64 = 0;   // <- from provider response
    let actual_out_tokens: u64 = 0;  // <- from provider response
    let actual_nc = actual_in_tokens * input_price_nc
        + actual_out_tokens * output_price_nc;

    // 4. Confirm receipt (A1 check happens here; A1 violation fails noisily)
    let refund = receipt.confirm(actual_nc)?;
    let final_budget = refund.apply_to(after_reserve)?;
    Ok(final_budget)
}
```

For end-to-end working examples with mock and real providers, see
[`rig-budget`](https://github.com/sajjadanwar0/rig-budget) and
[`token-budgets-experiments`](https://github.com/sajjadanwar0/token-budgets-experiments).

## Build and test

```bash
cargo build --release
cargo test
cargo bench           # Criterion microbenchmarks (success/failure path)
```

Criterion microbenchmarks in `benches/spend_bench.rs` measure
`Budget::spend` on success and failure paths with proper `black_box`
discipline.

## Catalog of real-world failures

`data/budget-archaeology.csv` contains **167 documented incidents
across 23 frameworks and 18 ecosystems**, spanning 2023–2026. Each
row carries: `issue_id`, `framework`, `date`, `short_url`, `title`,
`prevented_at_compile_time`, `user_dollar_loss`, and free-text
`notes` with verbatim maintainer/reporter quotes.

The codebook at `data/budget-archaeology-codebook.md` defines the
8 mechanism clusters and 4 amplification levels used in the paper.

### Honest scope on the catalog

- **Inter-rater reliability is moderate, not strong.** κ=0.506
  (LLM-on-human, n=30 stratified sample, two raters) — reported as
  a codebook-stability statistic, not as inter-human IRR. An
  independent two-human IRR study on a 50-row sample is open work.
- **The 8 mechanism clusters are post-hoc analytic.** They emerged
  from the data rather than being independently derived.
- **The catalog is a convenience sample** of GitHub issues found
  via systematic search; it is not a probability sample of all
  agent-spend incidents.

These limitations are discussed in paper §V and the codebook.

## Verification

The discipline's cap-soundness theorem is mechanized across
**five independent provers**, all reproducible from
[`token-budgets-formals`](https://github.com/sajjadanwar0/token-budgets-formals):

| Tier | Tool | Verified |
|---|---|---|
| 1 | TLAPS | 497 obligations proved (Zenon+Isabelle, no SMT) |
| 2 | TLC | 252 distinct reachable states at B₀=5 |
| 3 | Coq | 0 Admitted, 0 axioms in `budget.v` |
| 4 | Dafny | 23 verified, 0 errors |
| 5 | Verus | 66 theorems (42 sequential + 11 pool + 13 concurrent) |

Tiers 1-4 verify the abstract Budget state machine. Tier 5 (Verus)
verifies the actual Rust source code.

## Honest scope on the discipline itself

What this discipline **does** provide:
- Type-level integrity of resource accounting (compiler-enforced
  affine consume-by-value)
- Runtime cap arithmetic with overflow-safe checked ops
- Receipt-cycle reconciliation between reserved and actual usage
- Mechanically verified cap-soundness across five independent provers

What this discipline does **NOT** provide:
- **A1 (byte-length dominance)** is empirically supported but the
  fully-mechanized proof for an arbitrary BPE tokenizer construction
  is open work. Informal justification is in paper §IV-B.
- **A3 (provider truthfulness)** is an external assumption. No
  client-side discipline can prevent a malicious provider from
  misreporting usage.
- **Conjecture 1: operational refinement to running Tokio**
  is open work. Partial mechanization is in
  [`token-budgets-formals/coq/BudgetTraceRefinement{,Pure}.v`](https://github.com/sajjadanwar0/token-budgets-formals/tree/master/coq)
  and [`token-budgets-extensions/verus-skeleton/`](https://github.com/sajjadanwar0/token-budgets-extensions/tree/master/verus-skeleton).

The empirical validation in
[`token-budgets-experiments`](https://github.com/sajjadanwar0/token-budgets-experiments)
substitutes for the missing refinement at the EMSE level but does
not subsume it. 5,424 live API row-events observed zero
cap-soundness violations across the corpus, but this is
observational evidence, not proof.

## Related repositories

| Repository | What it contains |
|---|---|
| [`token-budgets`](https://github.com/sajjadanwar0/token-budgets) | This — the main affine-API library + 167-entry catalog |
| [`token-budgets-extensions`](https://github.com/sajjadanwar0/token-budgets-extensions) | Adaptive estimator + Verus Conjecture-1 skeleton (open) |
| [`token-budgets-formals`](https://github.com/sajjadanwar0/token-budgets-formals) | Five-tier mechanized verification: TLAPS, TLC, Coq, Dafny, Verus |
| [`token-budgets-experiments`](https://github.com/sajjadanwar0/token-budgets-experiments) | Empirical validation: 5,424 live API row-events, sweeps, governor-crate comparison |
| [`rig-budget`](https://github.com/sajjadanwar0/rig-budget) | Integration helper threading the discipline through the `rig` LLM framework |

## Paper

```bibtex
@article{khan-token-budgets-2026,
  author  = {Khan, Sajjad},
  title   = {Token Budgets: An Affine-Resource Discipline for LLM Cost Caps in Rust},
  journal = {arXiv preprint arXiv:TBD},
  year    = {2026}
}
```

Sections of the paper that touch this crate directly:
- §III: The Budget/Receipt/Refund affine API (this crate's `src/lib.rs`)
- §IV-B: Assumption A1 (UTF-8 byte-length dominance) — basis for `ByteLength` estimator
- §IV-D: Typed-capability formal foundation + five-tier verification
- §V: Decision framework + the 167-entry catalog
- §VII: Open work (Conjecture 1, A1 mechanization)

## License

Dual MIT/Apache-2.0. See `LICENSE-MIT` and `LICENSE-APACHE`.

## Contributing

Single-author repository for a paper artifact; not currently
accepting external contributions during the review period. After
acceptance/publication, the contribution policy will be revisited.

Issues and discussion are welcome.