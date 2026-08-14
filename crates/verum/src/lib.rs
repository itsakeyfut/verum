//! Verum — an AI-native semantic web framework.
//!
//! Endpoints declare their semantics as a typed contract — which fields they may
//! read and mutate, which effects they may cause, under which conditions — and
//! implementations that deviate from the declaration do not compile.
//!
//! # Status
//!
//! Skeleton. No types are defined yet; the type-level primitives arrive in
//! Phase 0 and the contract DSL in Phase 2.
//!
//! # Layout
//!
//! Users depend on this crate alone. The derive and attribute macros live in a
//! separate crate because a `proc-macro = true` crate cannot export anything
//! else, and are re-exported here.

// `verum-macros` exports nothing yet, so the glob re-export is currently empty
// and rustc reports it as unused. The wiring is kept in place so that the macros
// defined in T-M2-01 are reachable through `verum` without a further change.
// Remove this `allow` as soon as the first macro exists.
#[allow(unused_imports)]
pub use verum_macros::*;
