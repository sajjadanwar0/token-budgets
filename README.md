# budget-typed-cap: Compile-Time A2 Enforcement

A const-generic variant of the Token Budgets `Budget` type that lifts
Assumption A2 (overflow-free regime) from a deployment precondition to
a compile-time guarantee.

## What A2 is

Token Budgets' cap-soundness lemma requires every `Budget` to satisfy
`micro_cents < 2^63` so that pairs of budgets can be added in `u64`
without overflow. The base library treats this as a deployment
precondition — operators must cap `Budget::new(n)` at `n < 2^60` at
their trust boundary. The base library has correct runtime
`checked_add` behaviour everywhere, so A2 violations don't corrupt
state; they just turn into `BudgetError::Overflow` returns.

## What this crate does

`Budget<const MAX: u64>` carries the cap as part of the type. The
`_A2_HOLDS` const assertion

```rust
const _A2_HOLDS: () = assert!(MAX < (1u64 << 63), "...");
```

monomorphizes at every instantiation. A program that names
`Budget::<{ u64::MAX }>::new(0)` is **rejected by rustc at const-eval
time**.

## Running

```bash
cd budget-typed-cap
cargo test --release
```

This runs:

- **15 unit tests** of the affine `spend`/`split`/`merge`/`new`
  operations at type-level cap
- **3 doctests** in the lib docstring:
  - 2 `compile_fail` doctests that verify `Budget::<{ u64::MAX }>` and
    `Budget::<{ 1u64 << 63 }>` are rejected by rustc — these doctests
    PASS when compilation FAILS (that's the demonstration)
  - 1 normal doctest that verifies the boundary case
    `Budget::<{ (1u64 << 63) - 1 }>` compiles

All 18 tests should pass.

## What you'll see

```
running 15 tests
test tests::a2_compiles_for_safe_max ... ok
test tests::merge_within_cap_succeeds ... ok
... (13 more, all ok)
test result: ok. 15 passed; 0 failed

   Doc-tests budget-typed-cap

running 3 tests
test src/lib.rs - (line 35) ... ok
test src/lib.rs - (line 45) ... ok
test src/lib.rs - (line 53) ... ok

test result: ok. 3 passed; 0 failed
```

The `compile_fail` doctests each show "ok" because their underlying
compile failed exactly as expected.

## What the type-level enforcement gets you

| Property | Runtime `Budget` | `Budget<MAX>` |
|---|---|---|
| `micro_cents <= MAX` for every value | runtime check at constructor | type-level invariant |
| A2 (`micro_cents < 2^63`) | deployment precondition | const-assertion (compile-time) |
| Operator can deploy violating A2 | yes, with `Budget::new(n)` for `n >= 2^63` (will return `Overflow` later) | no, program fails to compile |
| Heterogeneous-cap collections | yes (cap is runtime) | no (cap is type-level) |
| Dynamic cap reconfiguration | yes | requires re-compile |

## Layout

```
budget-typed-cap/
├── Cargo.toml             # Standalone workspace (no parent inheritance)
├── README.md              # This file
└── src/
    └── lib.rs             # Budget<MAX> + tests + compile_fail doctests
```

## Status

This crate is a **demonstration** of the technique. The main
`token-capability` library continues to ship the runtime-checked
`Budget` as its production default. Integrating this into the main
library, and extending the Coq/TLA+/Dafny mechanization to the
const-generic version, are future work.
