//! Stands in for the `verum` crate.
//!
//! Deliberately tiny. The question under test is Rust visibility and how sqlx's
//! macros expand, neither of which needs cons lists, `Ctx`, or capability
//! checking to reproduce. Adding them would only make the probes harder to read.

/// Marker only, so the app's `User` is a Domain in the sense the specs use.
pub trait Domain {}

/// The Repr conversion expressed as a **framework trait**.
///
/// This is NOT the shape `docs/specs/persistence.md` currently specifies — that
/// one uses inherent `pub(crate) fn from_repr` / `as_repr`. It is here because
/// "put the conversion behind a verum trait" is one of the alternatives #18 will
/// have to weigh, and P9/P10 measure what it costs before anyone picks it.
///
/// The cost is structural: **a trait method cannot be `pub(crate)`**. It is
/// exactly as public as the trait, so this makes the conversion reachable from
/// every crate rather than one, and `Self::Repr` becomes nameable by projection
/// even where the underlying type is not.
pub trait DomainRepr: Sized {
    type Repr;
    fn from_repr(r: Self::Repr) -> Self;
    fn as_repr(&self) -> &Self::Repr;
}
