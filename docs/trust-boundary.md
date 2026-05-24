# Token Budgets Trust Boundary (v0.2)

This document explains the threat model enforced by `build.rs` when the
`system-authority` Cargo feature is enabled.

## Problem we fix

`BudgetMint::take_authority()` is the only public way to construct a
`Budget` value. The function is gated behind the `system-authority`
Cargo feature, so any crate that wants to mint budgets must enable
that feature.

Per Cargo RFC 2867 (feature unification), any workspace member can
enable `system-authority` on a transitive dependency. This means
"feature flag is set in Cargo.toml" is insufficient as a trust
boundary — a malicious transitive dep can silently enable the feature
on `token-budgets` without the operator's awareness.

## What `build.rs` v0.2 does

When `system-authority` is enabled, the build script:

1. Locates the workspace root by walking up from `CARGO_MANIFEST_DIR`.
2. Reads an allowlist file from `$TOKEN_BUDGETS_AUTHORITY_ALLOWLIST`
   (defaults to `./.token_budgets_authority.toml` in the workspace root).
3. Fails the build if the allowlist file is missing or if the
   activating crate is not in `allowed_activators`.

This forces the operator to make an **explicit, version-controlled**
decision about which crates may mint budgets. The allowlist file is a
PR-reviewable artifact; changes to it appear in git diffs and CI logs.

## Threat model

### Threats this defends against

| Threat | Defense |
|--------|---------|
| Malicious transitive dep enables `system-authority` | Caught at build time: activator must be in allowlist |
| Operator forgets to allowlist a new dep | Caught at build time: explicit error message |
| Compromised crate gains minting via feature unification | Caught at build time: feature activator is recorded |
| Silent feature enablement during dependency upgrade | `cargo:rerun-if-changed` on allowlist forces rebuild on change |

### Threats this does NOT defend against

| Threat | Why not |
|--------|---------|
| Malicious crate that is already on the operator's allowlist | TCB includes allowlisted set; allowlist must be reviewed |
| Operator commits a wildcard `allowed_activators = ["*"]` | This is a process failure, not a type-system failure |
| Malicious workspace member edits the allowlist file | Mitigated by `git_tag` field; operator must verify with `cargo verify-authority` (planned) |
| `unsafe` Rust that constructs `Budget` via transmute or pointer arithmetic | Outside the type system; flagged by `#![forbid(unsafe_code)]` lint |
| Direct edits to compiled `.rlib` files | Outside any source-level type system |

## Allowlist file format

Example `.token_budgets_authority.toml`:

```toml
# Version tag pinning the allowlist to a tagged commit
git_tag = "v1.0.0"

# Crate names allowed to enable system-authority feature
allowed_activators = [
    "my-agent-runtime",
    "production-cli",
]

# Optional: SHA-256 of the workspace Cargo.lock at the pinned tag
cargo_lock_sha256 = "..."
```

## Status

- **`build.rs`**: Shipped.
- **`cargo verify-authority` subcommand**: Skeleton shipped at
  `tooling/cargo-verify-authority/`; not yet integrated.
- **`.token_budgets_authority.toml.example`**: Shipped (workspace root).

## TCB summary

After `build.rs` v0.2, the TCB for compile-time minting integrity is:

1. The operator-controlled allowlist file
2. The `token-budgets` crate source itself
3. The Rust compiler

This is strictly smaller than the pre-v0.2 TCB, which was "every
crate in the dependency graph that could enable `system-authority`".
