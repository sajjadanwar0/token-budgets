# Trust boundary and trusted computing base (TCB)

This document states exactly what the `token-budgets` compile-time integrity
claim does and does not assume. It is the artifact-level companion to the
paper's Table 1 (claims/scope) and Table 2 (guarantee map). Nothing here is a
new claim; it consolidates the trust assumptions already stated in the paper
(§1.1, §3.4, §3.5, §4.4, §6.x) so a reviewer can see the boundary in one place.

## What the type system enforces (inside the boundary)

On well-typed Rust source compiled under `forbid(unsafe_code)`, the borrow
checker rejects, at compile time:

- **Aliasing** of a `Budget` (`Budget` is neither `Clone` nor `Copy`) — E0599.
- **Double-spend / use-after-move** (`spend` consumes `self`) — E0382.
- **Use-after-split** (`split` consumes the parent) — E0382.
- **Capability-gated construction**: `Budget::new` / `BudgetMint::take_authority`
  is reachable only with the `system-authority` Cargo feature — E0624.

These are the integrity properties. They are NOT a dollar bound; the cap itself
is runtime arithmetic (`checked_sub`) under estimator assumption A1 (§3.4).

## The constructor surface is the actual trust boundary

The affine discipline prevents in-program forgery of an *existing* `Budget`; it
does not bound the trusted `Budget::new` constructor. Any code path that can
reach the constructor can mint a fresh budget. This is the standard ocap threat
model: authority flows from the constructor's caller.

- `Budget::new` is a single named function; its callers are statically
  discoverable via Rust's module system.
- `BudgetMint::take_authority()` is gated behind the `system-authority` Cargo
  feature, which must be enabled in the top-level binary's `Cargo.toml`. Feature
  enablement appears in `Cargo.lock` and is reviewable in PR diffs — this is the
  operative defense against a dependency introducing rogue minting.
- The operator's TCB is therefore the set of files that invoke `Budget::new`
  plus the `system-authority` allowlist (`.token_budgets_authority.toml`). Keep
  it small by policy; this makes the assumption *auditable*, not *eliminated*.

## `forbid(unsafe_code)` scope

`forbid(unsafe_code)` is enforced at the crate root (`[lints.rust]` in
`Cargo.toml`). It governs THIS crate's own code, which contains zero `unsafe`
blocks. It does **not** apply to transitive dependencies: a typical Rust agent
project has 100+ transitive crates, any of which could forge a `Budget` via
`mem::transmute` inside its own `unsafe`. Before relying on the integrity claim,
production deployments should run `cargo geiger` and an SBOM audit, and treat
the `BudgetMint` allowlist — not `forbid(unsafe_code)` at the root — as the real
trust boundary.

## Runtime / operator-supplied trust assumptions (outside the boundary)

The cap-respecting *outcome* depends on assumptions the type system cannot
enforce. These are shared with every client-side cost-accounting mechanism
(LangSmith, LiteLLM proxy budgets, AgentGuard, Helicone):

| Assumption | Statement | Failure mode | Mitigation |
|-----------|-----------|--------------|------------|
| **A1** (estimator soundness) | reserved ≥ billable tokens for every prompt | under-reservation → overshoot | per-provider/model calibration; conservative margin (2.0× default) |
| **A6** (output cap honored) | billed output ≤ `max_output_tokens` | reasoning models bill hidden thinking tokens | provider-side `reasoning_effort` / `thinking.budget_tokens` |
| **A7** (charge truthfulness) | provider-reported `actual_charge` ≥ true bill | undetectable overshoot (quantified in Table 5) | periodic out-of-band reconciliation against billing |
| **A8** (rate stability) | per-token rates match provider's, mid-session | silent miscalibration | pin tokenizer + pricing version in build metadata |

## Not claimed

Binary-level cap-soundness on the compiled Tokio binary (Conjecture 1, §3.5) is
**open**: `rustc` codegen, LLVM optimization, and scheduler behavior are trusted,
not proved. The specification cross-checks (TLA+/TLAPS/TLC, Coq, Dafny, Verus)
establish internal consistency of the abstract specification, not a
source-to-binary refinement.