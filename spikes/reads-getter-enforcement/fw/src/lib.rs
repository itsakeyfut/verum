//! The framework half. `Repo` and `ReadSet` are what `verum` would own.
//!
//! WHY THIS CRATE EXISTS
//!   The `Domain` belongs to the downstream crate; `Repo` belongs to the
//!   framework. The first version of this spike put both in one crate, which
//!   let an inherent `impl<R, M> Repo<Domain, R, M>` compile. That shape cannot
//!   exist in the real layering — it is an inherent impl on a foreign type,
//!   E0116, which `docs/dev/code/review-knowledge.md` RK-004 already records.
//!   Probe E1 in `app` pins it.

use std::marker::PhantomData;

/// The capability handle. `R` is the endpoint's `reads`, `M` its `mutates`;
/// `M` is unused here and present only so the shape matches the specs.
///
/// **`new` is deliberately public, and probe G1 is why.** A public constructor
/// lets the caller choose `R`, which is the precondition the enforcement claim
/// rests on and did not state. `Repo` must be reachable only from `Ctx` for the
/// bound to mean anything.
pub struct Repo<D, R, M>(PhantomData<fn() -> (D, R, M)>);

impl<D, R, M> Repo<D, R, M> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<D, R, M> Default for Repo<D, R, M> {
    fn default() -> Self {
        Self::new()
    }
}

/// The framework's projection from a handle to its read set.
///
/// This is what makes the downstream extension trait unforgeable: the trait
/// carries **no** `R` parameter, so `R` can only arrive through `Self::Set`, and
/// `Self::Set` is fixed here. A downstream crate cannot re-point it — `ReadSet`
/// and `Repo` are both foreign to it, so the orphan rule refuses (probe G2).
pub trait ReadSet {
    type Set;
}

impl<D, R, M> ReadSet for Repo<D, R, M> {
    type Set = R;
}
