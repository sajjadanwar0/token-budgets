# Token Budgets

[![arXiv](https://img.shields.io/badge/arXiv-2606.04056-b31b1b.svg)](https://arxiv.org/abs/2606.04056)
[![DOI](https://img.shields.io/badge/DOI-10.48550%2FarXiv.2606.04056-blue.svg)](https://doi.org/10.48550/arXiv.2606.04056)
[![Rust](https://img.shields.io/badge/rust-no__unsafe-orange.svg)](https://github.com/sajjadanwar0/token-budgets)

> **Paper:** Sajjad Khan, *Token Budgets: An Empirical Catalog of 63 LLM-Agent
> Budget-Overrun Incidents, with an Affine-Typed Rust Mitigation as a Case
> Study*, arXiv:2606.04056 [cs.SE], 2026. <https://arxiv.org/abs/2606.04056>

Primary artifact for the paper: the **empirical catalog** of LLM-agent
budget-overrun incidents and **`token-budgets`**, a small Rust crate that makes
budget misuse a *compile error* rather than a runtime hazard.

## The problem

LLM-agent budget overruns are a documented production failure class — a single
retry loop can spend thousands of dollars before an operator notices. The
in-process integrity properties that would prevent it (no aliasing, no
double-spend, no use-after-delegation of a cost-bearing value) are usually
enforced, if at all, by ad-hoc wrappers rather than by the type system.

## The empirical contribution

A catalog of **63 confirmed production incidents** from **21 orchestration
frameworks** (2023–2026), each backed by a quoted GitHub issue and, where
reported, a dollar loss — organized into an **eight-cluster failure taxonomy**
(inter-rater Cohen's kappa = 0.837, N = 113), plus **47 supplementary
structural entries** (110 catalog rows total).

| Cluster | Count |
| --- | --- |
| M-retry-loop | 27 |
| M-cost-observability | 22 |
| M-context-amplification | 13 |
| M-storage-amplification | 13 |
| M-budget-primitive-missing | 12 |
| M-delegation-fanout | 11 |
| providerOptions-silently-dropped | 6 |
| M-multimodal-cost-amplification | 6 |

The catalog lives in `data/catalogue.csv` (label column `bf/bu/mf/fr`;
`primary_cluster` column re-derives the taxonomy above).

## The mitigation: `token-budgets`

A ~1,180-line Rust crate (built under `#![forbid(unsafe_code)]`) that
operationalizes **affine ownership** for a cost-bearing `Budget` value:

- cloning a `Budget`,
- double-spending it, or
- using it after delegating it

are **compile errors**, caught by the borrow checker, not runtime hazards an
operator has to remember to avoid. The dollar cap itself is runtime arithmetic
under an estimator assumption; the affine layer is what makes that arithmetic
**non-bypassable**.

### What the evaluation shows

- **Single-agent:** a 4-line Python counter matches the crate (0/30 overshoot),
  so the distinguishing value is *non-bypassability under operator error in
  multi-agent delegation*.
- **Multi-agent delegation:** the delegation-fanout race documented in 11
  incidents is rejected at compile time; the same pattern under `asyncio`
  overshoots 30/30, while three disciplined alternatives overshoot 0/30.
- **Live API:** across five runtimes, three providers, and a
  temperature-stratified test (N = 160), zero cap violations and zero false
  refusals, at operational parity with concurrent work.
- **Cost:** static over-reservation is 4–6x (2.11x adaptive).
- **Open:** binary-level cap-soundness on the running binary is left open.

## Layout

```
token-budgets/
├── src/                              # the affine Budget crate (no unsafe)
├── benches/                          # Criterion microbenchmarks (spend_bench, ~1.15 ns/op)
├── tests/
│   ├── compile_fail/                 # trybuild: 9 cases, 7 rustc codes (E0277/E0308/
│   │                                 #   E0382/E0505/E0507/E0599/E0624)
│   └── loom_concurrent.rs            # loom exhaustive-interleaving tests
├── data/
│   └── catalogue.csv                 # the 110-row incident catalog (63 confirmed + 47 supplementary)
├── docs/
│   └── trust-boundary.md             # what Budget::new trusts; the A1/A2/A6/A7 assumptions
├── tooling/
│   └── cargo-verify-authority/       # checks the Budget::new capability allowlist
├── .token_budgets_authority.toml.example
├── Cargo.toml                        # `system-authority` feature gates the trybuild suite
├── Cargo.lock
├── reproduce.sh                      # one-command reproduction of the paper's artifact claims
└── README.md
```

## Build & test

```bash
cargo build --release                 # builds under forbid(unsafe_code)
cargo test  --release                 # unit + integration tests
cargo test  --release --features system-authority --test compile_fail   # trybuild
cargo bench --bench spend_bench --features system-authority             # ~1.15 ns/op
RUSTFLAGS="--cfg loom" cargo test --release --features system-authority \
    --target-dir target-loom --test loom_concurrent                     # loom
```

## Reproducing the paper

`reproduce.sh` is the single entry point. It clones the companion repos see Related repos, audits
the artifact-level claims, builds the formal proofs, and runs the offline
benchmarks; live-API cells are opt-in.

```bash
./reproduce.sh                  # offline replication (~10 min, no API keys)
./reproduce.sh --with-live      # also run live-API cells (~$0.50, ~30 min)
./reproduce.sh --formal-only    # only the formal proofs (~5 min)
```

Requirements: `git`, `python3.11+`, `rustc 1.95` (pinned), and — for the formal
layer — Coq 8.18 with Iris/stdpp (and a built lambda-rust for the RustBelt tier;
see `token-budgets-formals/coq/README.md`). Set `ANTHROPIC_API_KEY` and
`OPENAI_API_KEY` for `--with-live`.

## Related repos

The end-to-end reproduction (catalog audit, formal proofs, microbenchmarks,
live-API replication) is driven by `reproduce.sh`. The artifact spans several
repositories:

- [`token-budgets-formals`](https://github.com/sajjadanwar0/token-budgets-formals) — Coq / Dafny / Verus mechanization + inter-rater reliability
- [`token-budgets-experiments`](https://github.com/sajjadanwar0/token-budgets-experiments) — estimator validation, over-reservation, multi-runtime sweeps
- [`token-budgets-python`](https://github.com/sajjadanwar0/token-budgets-python) — Python `token_budgets` package
- [`token-budgets-baseline`](https://github.com/sajjadanwar0/token-budgets-baseline) — baseline/control implementations
- [`token-budgets-extensions`](https://github.com/sajjadanwar0/token-budgets-extensions) — framework adapters / capability catalog
- [`token-budgets-rig`](https://github.com/sajjadanwar0/token-budgets-rig) — N=1 deployment case study (Rig + AutoAgents)

## Citation

```bibtex
@misc{khan2026tokenbudgets,
  title         = {Token Budgets: An Empirical Catalog of 63 LLM-Agent
                   Budget-Overrun Incidents, with an Affine-Typed Rust
                   Mitigation as a Case Study},
  author        = {Khan, Sajjad},
  year          = {2026},
  eprint        = {2606.04056},
  archivePrefix = {arXiv},
  primaryClass  = {cs.SE},
  doi           = {10.48550/arXiv.2606.04056},
  url           = {https://arxiv.org/abs/2606.04056}
}
```

## License

Paper: CC BY 4.0 (arXiv). Code: see the repository `LICENSE` file.