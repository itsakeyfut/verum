//! The framework side, in the two shapes #41 compares.
//!
//! `private` holds the **structural** seals — `pub(crate)` here and permanently so
//! in the real crate, which is why ledger paths 14a–14e survive M2.
//!
//! `derive_facing` holds the seals a derive has to *name*, so M2 must expose it
//! (`docs/rules/api-surface.md` §2). Exposing it is this spike's whole subject, and
//! it is done in the exact form measured to be necessary: the module is declared
//! `pub`, because `pub use`-ing a `pub(crate)` module is **`E0365`** ("only public
//! within the crate, and cannot be re-exported outside"). #41 did not record that,
//! and it constrains how M2 can do it.

pub struct Here;
pub struct There<I>(core::marker::PhantomData<I>);

pub(crate) mod private {
    pub trait SealedHas<E, I> {}

    /// Structural under the **blanket** shape: the derive never names it, so it
    /// never leaves this module. That is the property #41 actually buys — and it is
    /// not the property #41 says it buys. See the README.
    #[cfg(feature = "blanket")]
    #[diagnostic::on_unimplemented(
        message = "`{Self}` does not declare the domain `{D}`",
        label = "reaching `{D}` requires declaring it",
        note = "either add `{D}` to this endpoint's declared domains, or use a domain it already declares — `Includes` is derived from the declaration and cannot be implemented by hand"
    )]
    pub trait SealedIncludes<D, I> {}
}

/// Exposed as M2 must expose it. `#[doc(hidden)]` is cosmetic: it hides the module
/// from rustdoc and changes nothing about reachability.
#[doc(hidden)]
pub mod derive_facing {
    /// Derive-facing under **both** shapes — the derive emits `impl Endpoint for X`,
    /// so it must name this. Nothing in #41's direction changes that, which is why
    /// `Endpoint` is the irreducible residual.
    #[diagnostic::on_unimplemented(
        message = "`{Self}` is not a Verum endpoint",
        label = "declare it with `#[endpoint(..)]`",
        note = "`Endpoint` is emitted by the attribute; do not write it by hand"
    )]
    pub trait SealedEndpoint {}

    /// Derive-facing only under the **per-domain** shape. Sealed on `(Self, D)` so
    /// one domain's impl does not unlock the others (`api-surface.md` §2) — still
    /// true, and still insufficient once the module is public.
    #[cfg(feature = "per-domain")]
    #[diagnostic::on_unimplemented(
        message = "`{Self}` cannot implement a sealed Verum trait",
        label = "this trait is sealed",
        note = "`Includes` is implemented by the derive; do not write it by hand"
    )]
    pub trait SealedIncludes<D> {}
}

pub trait Has<E, I>: private::SealedHas<E, I> {}
impl<H, T> private::SealedHas<H, Here> for (H, T) {}
impl<H, T> Has<H, Here> for (H, T) {}
#[diagnostic::do_not_recommend]
impl<H, T, E, I> private::SealedHas<E, There<I>> for (H, T) where T: Has<E, I> {}
#[diagnostic::do_not_recommend]
impl<H, T, E, I> Has<E, There<I>> for (H, T) where T: Has<E, I> {}

pub trait Endpoint: derive_facing::SealedEndpoint {
    type Domains;
}

// --- Shape 1: per-domain, what ships today ---------------------------------
#[cfg(feature = "per-domain")]
#[diagnostic::on_unimplemented(
    message = "`{Self}` does not declare the domain `{D}`",
    label = "reaching `{D}` requires declaring it",
    note = "either add `{D}` to this endpoint's declared domains, or use a domain it already declares — do not implement `Includes` by hand, it is sealed"
)]
pub trait Includes<D>: derive_facing::SealedIncludes<D> {}

// --- Shape 2: #41 / ADR-0013 -----------------------------------------------
//
// The seal AND the trait are blanket-implemented from the endpoint's declared set,
// so the derive emits nothing per domain and never names the seal.
//
// `do_not_recommend` on both is load-bearing for the *message*, not for the closure:
// without it rustc drills through to the raw `Has<D, I>` bound and the wording is
// lost. Measured both ways.
#[cfg(feature = "blanket")]
pub trait Includes<D, I>: private::SealedIncludes<D, I> {}

#[cfg(feature = "blanket")]
#[diagnostic::do_not_recommend]
impl<E, D, I> private::SealedIncludes<D, I> for E
where
    E: Endpoint,
    E::Domains: Has<D, I>,
{
}

#[cfg(feature = "blanket")]
#[diagnostic::do_not_recommend]
impl<E, D, I> Includes<D, I> for E
where
    E: Endpoint,
    E::Domains: Has<D, I>,
{
}
