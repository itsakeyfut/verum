//! Verum — an AI-native semantic web framework.
//!
//! Endpoints declare their semantics as a typed contract — which fields they may
//! read and mutate, which effects they may cause, under which conditions — and
//! implementations that deviate from the declaration do not compile.
//!
//! # Status
//!
//! Early Phase 0. The sealing foundation, the cons list representation, the index
//! markers, and membership ([`Has`]) exist; the contract DSL follows in Phase 2.
//!
//! Nothing here is wired to a runtime yet, so these types verify shapes and
//! membership but do not yet gate a real request path.
//!
//! # Layout
//!
//! Users depend on this crate alone. The derive and attribute macros live in a
//! separate crate because a `proc-macro = true` crate cannot export anything
//! else; `verum` depends on it and will re-export the macros here once they
//! exist.

mod domain;
mod sealed;
mod typelevel;

// Re-exported at crate level so each module can name its own seal —
// `private::SealedIncludes`, `private::SealedConsList`, … — in the supertrait
// position that docs/rules/api-surface.md §2 prescribes. One seal per sealed
// trait, deliberately: sharing one made rustc list every other sealed trait's
// implementors in each error.
pub(crate) use sealed::{derive_facing, private};

pub use domain::Includes;
pub use typelevel::{Append, ConsList, Has, Here, Index, Lookup, There};

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
