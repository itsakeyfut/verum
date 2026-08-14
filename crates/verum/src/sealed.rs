//! The supertraits every capability-gating trait carries.
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
/// cannot write the impls every sealed trait requires.
///
/// The module is load-bearing, not stylistic. A bare `pub(crate) trait` used as
/// the supertrait of a public trait is rejected — `trait ... is more private
/// than the item ...` (`private_bounds`). Nesting `pub` traits inside a
/// `pub(crate)` module is what makes the visibilities line up.
///
/// # One seal per sealed trait
///
/// An earlier shape used a single `Sealed<Args>` keyed by a discriminator. It
/// was abandoned on measurement: **rustc's sealed-trait help enumerates every
/// impl of the seal regardless of its arguments**, so each sealed trait's error
/// listed every *other* sealed trait's implementors. Trying to write
/// `impl Includes<Order> for User` suggested `()`, `(H, T)`, `Here` and
/// `There<I>` as the fix. That list grows with each new sealed trait.
///
/// Separate seals keep each error to its own implementors. The cost — an
/// annotation per seal, which is easy to forget — is why they are declared
/// through [`seal!`] rather than by hand.
pub(crate) mod private {
    /// Declares a seal.
    ///
    /// The diagnostic is emitted here, so a seal declared through this macro
    /// always carries one. That matters because the annotation is what stops a
    /// raw trait-bound error reaching the reader, and a seal added in a hurry is
    /// exactly where it would be omitted.
    ///
    /// The macro does **not** by itself make a hand-written seal impossible — a
    /// plain `pub trait SealedX {}` in this module compiles and passes the lint
    /// table (measured). What forbids it is
    /// [`tests::seals_should_only_be_declared_through_the_macro`], which reads
    /// this file and rejects any `pub trait` outside the macro template. Only
    /// doc comments pass through `$attr`, so a caller cannot override the
    /// mandated diagnostic either.
    ///
    /// The wording is deliberately generic: it fires for hand-written impls of
    /// any sealed trait, including ones that have nothing to do with contracts.
    /// Trait-specific guidance belongs on the sealed trait itself, which fires
    /// on a different unsatisfied bound and composes with this.
    macro_rules! seal {
        ($(#[doc = $doc:expr])* $name:ident $(<$($param:ident),+ $(,)?>)?) => {
            $(#[doc = $doc])*
            #[diagnostic::on_unimplemented(
                message = "`{Self}` cannot implement a sealed Verum trait",
                label = "not sealed by Verum",
                note = "Verum's sealed traits are implemented by Verum itself and by its derive macros. Writing the impl by hand grants something no declaration authorises, and nothing would report the difference."
            )]
            pub trait $name $(<$($param),+>)? {}
        };
    }

    seal! {
        /// Seals [`crate::Includes`].
        ///
        /// Parameterised by the domain, so the seal covers the *relationship*
        /// rather than the type. Sealing on `Self` alone would let one
        /// derive-generated impl unlock every other domain — see
        /// `docs/rules/api-surface.md` §2.
        SealedIncludes<D>
    }

    seal! {
        /// Seals [`crate::ConsList`].
        SealedConsList
    }

    seal! {
        /// Seals [`crate::Index`].
        SealedIndex
    }
}

#[cfg(test)]
mod tests {
    /// Every seal must go through `seal!`, because that is what guarantees the
    /// `on_unimplemented` floor. A hand-written `pub trait SealedX {}` in
    /// `private` compiles and passes `-D warnings` while silently dropping the
    /// floor — measured, which is why the macro's doc comment no longer claims
    /// the macro alone prevents it.
    ///
    /// The rule is mechanical: the only `pub trait` in this file is the macro's
    /// own template, so anything else is a seal that skipped the macro.
    #[test]
    fn seals_should_only_be_declared_through_the_macro() {
        let strays: Vec<&str> = include_str!("sealed.rs")
            .lines()
            .map(str::trim_start)
            .filter(|line| line.starts_with("pub trait"))
            .filter(|line| !line.contains("$name"))
            .collect();

        assert!(
            strays.is_empty(),
            "seal declared outside `seal!`, so it carries no diagnostic: {strays:?}"
        );
    }
}
