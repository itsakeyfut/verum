//! The supertrait every capability-gating trait carries.
//!
//! Sealing exists because of how an AI responds to a trait-bound error. Verum
//! shows a lot of them on purpose — that is the product — and the first repair
//! reached for is to write the missing impl. `impl Includes<Order> for User {}`
//! compiles: `User` is a local type, so the orphan rule allows it. One line
//! removes the guarantee, `cargo build` succeeds, and nothing reports it.
//!
//! See `docs/rules/api-surface.md` §2 and `docs/specs/unverified-boundaries.md`
//! paths 12–14.

/// Private by construction: a downstream crate cannot name this module, so it
/// cannot write the impl that every sealed trait requires.
///
/// The module is load-bearing, not stylistic. A bare `pub(crate) trait Sealed`
/// used as the supertrait of a public trait is rejected — `trait Sealed is more
/// private than the item ...` (`private_bounds`). Nesting a `pub` trait inside a
/// `pub(crate)` module is what makes the visibilities line up.
pub(crate) mod private {
    /// Implemented only by Verum's own derives.
    ///
    /// # `Args` carries the sealed trait's own parameters
    ///
    /// Sealing on `Self` alone is not enough, and the difference is the whole
    /// guarantee. A supertrait bound of plain `Sealed` gates only *which types*
    /// may implement the trait — never *with which arguments*. So the moment a
    /// derive emits one `Sealed` impl for an endpoint, every hand-written
    /// `impl Includes<AnyDomain> for ThatEndpoint` compiles, including domains
    /// no contract declares. Verified by compiling, not reasoned about.
    ///
    /// Keying the seal on the arguments as well makes the *relationship*
    /// sealed: the derive emits `Sealed<Order>` only for declared domains, and
    /// forging `Includes<Secrets>` fails because `Sealed<Secrets>` is missing.
    ///
    /// **Rule for future sealed traits**: the seal must carry the trait's own
    /// type parameters — `Has<T, Idx>` needs `Sealed<(T, Idx)>`, not `Sealed`.
    /// And because `Sealed<X>` unlocks *every* trait keyed on `Sealed<X>`, two
    /// sealed traits that would share an `Args` shape need distinct seals.
    ///
    /// # Diagnostics
    ///
    /// This annotation is the floor: it applies to every trait that adopts the
    /// seal, including ones a future change forgets to annotate. rustc also
    /// recognises the sealed pattern and adds its own explanation, which this
    /// composes with rather than replacing. Traits add their own
    /// `on_unimplemented` on top — the two fire on different unsatisfied
    /// bounds, so they do not conflict.
    #[diagnostic::on_unimplemented(
        message = "`{Self}` cannot implement a sealed Verum trait",
        label = "this relationship was not produced by a Verum derive",
        note = "Verum generates this impl from the endpoint's contract declaration. Writing it by hand grants access that no contract declares, and nothing would report the difference."
    )]
    pub trait Sealed<Args> {}
}
