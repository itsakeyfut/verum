//! Verum — an AI-native semantic web framework.
//!
//! Endpoints declare their semantics as a typed contract — which fields they may
//! read and mutate, which effects they may cause, under which conditions — and
//! implementations that deviate from the declaration do not compile.
//!
//! # Status
//!
//! Early Phase 0. Only the sealing foundation and the first trait built on it
//! exist; the type-level primitives follow in T-M0-07/08 and the contract DSL in
//! Phase 2.
//!
//! # Layout
//!
//! Users depend on this crate alone. The derive and attribute macros live in a
//! separate crate because a `proc-macro = true` crate cannot export anything
//! else; `verum` depends on it and will re-export the macros here once they
//! exist.

mod domain;
mod sealed;

// Re-exported at crate level so every module writes the same `private::Sealed`
// supertrait bound that docs/rules/api-surface.md §2 prescribes.
pub(crate) use sealed::private;

pub use domain::Includes;

// The `verum-macros` dependency is declared but not yet re-exported: the crate
// defines no macros, so there is nothing to name. The re-export arrives with the
// first macro in T-M2-01, in the named form required by
// docs/rules/proc-macro.md §7:
//
//     pub use verum_macros::{contract, endpoint, Domain, ...};
//
// Do not restore a glob re-export. An empty `pub use verum_macros::*;` is
// unreachable, so `unreachable_pub` rejects it and it can only be kept alive by
// stacking `#[allow]`s on a line that exports nothing.
