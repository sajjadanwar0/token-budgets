//! `BudgetMint` / `CapAuthority`: capability-gated authority to construct
//! `Budget` values.
//!
//! # Why this exists
//!
//! In v0.x, `Budget::new(amount) -> Result<Budget, BudgetError>` was a
//! `pub fn`. Any crate in the workspace dependency tree could mint
//! arbitrary Budget values, including supply-chain-attacker crates that
//! gain workspace membership via dependency confusion. This undermined
//! the "compile-time integrity" claim: the borrow checker prevents
//! misuse of a Budget but does nothing to prevent unauthorised creation
//! of new Budget instances.
//!
//! # What this fixes
//!
//! `Budget::new` is now `pub(crate)`. The only public mint API is
//! `Budget::mint(&BudgetMint, u64)` (or its legacy alias
//! `Budget::new_sealed(&CapAuthority, u64)`). Acquiring a `BudgetMint`
//! requires the `system-authority` Cargo feature, which should be
//! enabled only in the top-level binary's `Cargo.toml`.
//!
//! Feature flips appear in the workspace `Cargo.lock` and
//! `cargo tree -f "{p} {f}"`, and are reviewable in PR diffs.
//!
//! # Threat model
//!
//! Closed:
//! - **T1: Workspace-resident malicious crate.** Cannot mint a Budget
//!   without the operator enabling `system-authority`. Enablement is
//!   visible in `Cargo.lock`.
//! - **T2: Plugin / dynamic loader.** Cannot obtain a `BudgetMint`
//!   because the FFI boundary does not expose it.
//! - **T3: Workspace member as regular dependency.** Cannot enable
//!   `system-authority` for itself; feature is workspace-scoped.
//!
//! Not closed:
//! - **Operator misconfiguration.** Enabling `system-authority` in a
//!   library crate (not a binary) re-opens the gap.
//! - **Unsafe code.** A dependency containing
//!   `unsafe { std::mem::transmute(...) }` can manufacture a Budget
//!   value. Mitigation: `#[forbid(unsafe_code)]`; recommend
//!   `cargo-geiger` audits on the dependency tree.
//! - **Compiler bugs.** rustc soundness assumed.
//!
//! # Usage
//!
//! Top-level binary's `Cargo.toml`:
//! ```toml
//! [dependencies]
//! token-budgets = { version = "0.5", features = ["system-authority"] }
//! ```
//!
//! ```rust,ignore
//! use token_budgets::{Budget, BudgetMint};
//!
//! fn main() {
//!     let mint = BudgetMint::take_authority();
//!     let budget = Budget::<1_000_000>::mint(&mint, 100_000).unwrap();
//!     run_agent(budget);
//! }
//! ```

use core::marker::PhantomData;

mod sealed {
    /// A token only constructible from within the `mint::sealed` module.
    /// Used as a phantom field of `CapAuthority` to ensure that
    /// `CapAuthority` cannot be constructed via struct literal even
    /// inside the `token_budgets` crate; only
    /// `CapAuthority::seal_at_startup` (feature-gated) can mint one.
    pub struct CapAuthorityToken {
        _private: (),
    }

    impl CapAuthorityToken {
        #[cfg(feature = "system-authority")]
        pub(super) fn new_sealed() -> Self {
            Self { _private: () }
        }
    }
}

/// Capability token authorising creation of `Budget` values.
///
/// `CapAuthority` is `!Send + !Sync + !Clone + !Copy` (the
/// `PhantomData<*const ()>` field opts the struct out of `Send`/`Sync`,
/// and the absence of `derive(Clone, Copy)` keeps the latter two off).
///
/// The capability is *reusable* (a binary may legitimately mint many
/// budgets, e.g., per-tenant): the restriction is on *who* may mint, not
/// *how many* mints are allowed.
///
/// `BudgetMint` is the paper-consistent alias for this type; both names
/// refer to the same struct.
pub struct CapAuthority {
    _seal: sealed::CapAuthorityToken,
    // `*const ()` is a raw pointer; raw pointers do not implement
    // Send or Sync by default. Wrapping in PhantomData costs zero bytes
    // at runtime.
    _not_send_sync: PhantomData<*const ()>,
}

impl CapAuthority {
    /// Acquire mint authority.
    ///
    /// Only available when the `system-authority` Cargo feature is enabled.
    /// Enable this feature only in the top-level binary's `Cargo.toml`.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Requires the `system-authority` feature.
    /// let auth = token_budgets::CapAuthority::seal_at_startup();
    /// let b = token_budgets::Budget::<1_000_000>::new_sealed(&auth, 1_000);
    /// ```
    #[cfg(feature = "system-authority")]
    #[must_use]
    pub fn seal_at_startup() -> Self {
        Self {
            _seal: sealed::CapAuthorityToken::new_sealed(),
            _not_send_sync: PhantomData,
        }
    }

    /// Paper-consistent alias for [`seal_at_startup`].
    #[cfg(feature = "system-authority")]
    #[must_use]
    pub fn take_authority() -> Self {
        Self::seal_at_startup()
    }
}

/// Paper-consistent alias for [`CapAuthority`].
pub type BudgetMint = CapAuthority;

#[cfg(all(test, feature = "system-authority"))]
mod tests {
    use super::*;

    #[test]
    fn mint_can_be_acquired() {
        let _mint = BudgetMint::take_authority();
    }

    #[test]
    fn cap_authority_alias_works() {
        let _auth = CapAuthority::seal_at_startup();
    }
}