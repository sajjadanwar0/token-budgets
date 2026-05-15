# Coq Mechanization for `budget-typed-cap`

This directory contains the Coq mechanization of cap-soundness for the
const-generic `Budget<MAX>` type, paired with the Rust const-assertion

```rust
const _A2_HOLDS: () = assert!(MAX < (1u64 << 63), ...);
```

in `../src/lib.rs`. Every concrete `Budget::<K>` in Rust pairs with a
concrete instantiation of these Coq theorems at the same `K`, with the
Coq hypothesis `MAX < 2^63` discharged by rustc's const-eval check.

## Files

```
coq/
├── _CoqProject
├── README.md                        # This file
├── BudgetTypedCap.v                 # Tier A — abstract state machine
├── BudgetIrisTypedCap.v             # Tier B — Iris heap_lang Hoare triples
└── BudgetRustBeltTypedCap.v         # Tier C — lambdaRust structural well-typing
```

## What's mechanized

### Tier A (`BudgetTypedCap.v`)

Pure Coq abstract state machine over a list of live budgets +
committed total. Four theorems, all close with **zero `Admitted`,
zero `Axiom`**:

- `step_invariant` — every step preserves the cap invariant
- `reachable_invariant` — reachable states satisfy the invariant
- `typed_cap_soundness` — for `0 ≤ MAX < 2^63`, `committed s ≤ MAX`
- `typed_cap_conservation` — for `0 ≤ MAX < 2^63`, `total s ≤ MAX`

No external dependencies beyond the Coq standard library
(ZArith, Lia, List, Bool).

### Tier B (`BudgetIrisTypedCap.v`)

Per-method Iris Hoare triples on heap_lang with the type-level cap
bundled in. Defines `budget_inv_cap MAX l v` as

    budget_inv l v  ∗  ⌜0 ≤ v ≤ MAX⌝  ∗  ⌜MAX < 2^63⌝

Lifts the runtime triples to:
- `wp_spend_cap` — capped spend preserves `0 ≤ v ≤ MAX`
- `wp_split_cap` — split yields two capped budgets
- `wp_merge_cap` — merge of two capped budgets stays capped (with the
  precondition `v1 + v2 ≤ MAX`)
- `wp_consume_cap` — consume yields 0, trivially capped
- `budget_inv_cap_excl` — capped budgets are still affine
- `budget_iris_typed_cap_closed` — aggregator of all four triples

### Tier C (`BudgetRustBeltTypedCap.v`)

Parameterized lambdaRust semantic type `budget_max : Z → type` with
predicate `0 ≤ z ≤ MAX ∧ MAX < 2^63`. Lifts the runtime Tier C
well-typings:

- `spend_well_typed_max`, `split_well_typed_max`,
  `merge_well_typed_max`, `consume_well_typed_max`
- `conservation_alloc_max` — invariant-allocation scaffolding
- `rust_to_abstract_refinement_max` — aggregate well-typing
  theorem universally quantified over MAX

The encoding is "MAX is a Coq-meta parameter at the type-former level,
with the `MAX < 2^63` hypothesis discharged at the Rust const-assertion
site." Native const-generic type formers in lambdaRust would require
extending the lambdaRust type system itself; that remains open work.

## Prerequisites

| Tier | Coq | Iris | stdpp | lambda-rust |
|------|------|------|-------|-------------|
| A    | 8.16+ | — | — | — |
| B    | 8.16+ | 4.0+ | dev | — |
| C    | 8.16+ | 4.0+ | dev | a4e89895+ |

For Tier B and C, the files `BudgetAbstract.v`, `BudgetIris.v`,
`BudgetLinearTrace.v`, and `BudgetRustBelt.v` (from the main
mechanization) must also be compiled and findable under the `Top`
logical namespace. Copy them into this directory (or adjust paths).

## Building

```bash
# Set up the build (only needed once):
coq_makefile -f _CoqProject -o Makefile

# Then:
make                          # compiles all three files in order
```

Or compile a single tier:

```bash
coqc -Q . Top BudgetTypedCap.v             # Tier A (no Iris)
coqc -Q . Top BudgetIrisTypedCap.v         # Tier B (needs Iris)
coqc -Q . Top BudgetRustBeltTypedCap.v     # Tier C (needs lambda-rust)
```

## Verifying zero axioms

```bash
echo '
Require Import BudgetTypedCap.
Print Assumptions typed_cap_soundness.
Print Assumptions typed_cap_conservation.
Print Assumptions step_invariant.
Print Assumptions reachable_invariant.

Require Import BudgetIrisTypedCap.
Print Assumptions wp_spend_cap.
Print Assumptions wp_split_cap.
Print Assumptions wp_merge_cap.
Print Assumptions wp_consume_cap.
Print Assumptions budget_inv_cap_excl.
Print Assumptions budget_iris_typed_cap_closed.

Require Import BudgetRustBeltTypedCap.
Print Assumptions spend_well_typed_max.
Print Assumptions split_well_typed_max.
Print Assumptions merge_well_typed_max.
Print Assumptions consume_well_typed_max.
Print Assumptions rust_to_abstract_refinement_max.
' | coqtop -R . Top
```

Expected: each `Print Assumptions` should print
"Closed under the global context".

## Honest scope (same caveats as the runtime Tier C)

These files mechanize the **structural** properties of the
const-generic discipline:

- Tier A: abstract cap-soundness on a state-machine model
- Tier B: per-method Iris Hoare triples preserving the cap bound
- Tier C: structural well-typing of method bodies at a parameterized
  semantic type

What they do NOT mechanize (same as the runtime version):

- The trace-level semantic refinement of the running program against
  the abstract state machine. This is identified as the principal
  remaining open obligation in `BudgetRustBelt.v` and applies equally
  to the const-generic version.
- Native const-generic type formers in the lambdaRust type system
  itself. The encoding here is at the meta level (Coq parameter
  paired with Rust const-assertion).

## Pairing with the Rust const-assertion

Every concrete Rust instantiation `Budget::<K>` pairs with a Coq
instantiation of the relevant theorems at `MAX = K`. The Rust
const-eval check of `_A2_HOLDS` guarantees `K < 2^63` for every
successfully compiled Rust program; the Coq hypothesis `MAX < 2^63`
is therefore mechanically discharged at every concrete instantiation
point. Coq has no way of knowing whether the K passed in was
const-checked, but the rustc-side check is sound.
