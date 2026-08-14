//! Domain-side contract traits.

use crate::private;

/// Declares that an endpoint's domain set contains `D`.
///
/// This is the Architecture Contract: an endpoint may only reach the domains it
/// declared. `Includes` is the trait that answers "may this endpoint touch
/// `Order`?", so forging an impl of it is what would erase the contract.
///
/// It is sealed on `(Self, D)` rather than on `Self` alone. `impl Includes<Order>
/// for User {}` would otherwise compile — `User` is a local type, so the orphan
/// rule permits it — and once a derive had sealed `User` for any domain at all,
/// every *other* domain would be forgeable too. See
/// `docs/specs/unverified-boundaries.md` path 13.
///
/// The annotation here covers the case the seal's own annotation cannot reach:
/// an unsatisfied `Includes` bound at a *use* site, which is the shape almost
/// every real error takes. The seal only fires when someone writes the impl.
#[diagnostic::on_unimplemented(
    message = "`{Self}` does not declare the domain `{D}`",
    label = "reaching `{D}` requires declaring it",
    note = "either add `{D}` to this endpoint's declared domains, or use a domain it already declares — do not implement `Includes` by hand, it is sealed"
)]
pub trait Includes<D>: private::Sealed<D> {}

#[cfg(test)]
mod tests {
    use super::*;

    struct Order;
    struct GetOrder;

    // Proves the trait is satisfiable, not merely nameable. Without this, an
    // implementation where `Includes` can never hold would still pass the UI
    // suite — the `pass` case only shows the bound can be *written*.
    //
    // `#[cfg(test)]` does not cross the crate boundary, so this creates no
    // god-mode constructor for downstream code (docs/rules/test.md §4).
    impl private::Sealed<Order> for GetOrder {}
    impl Includes<Order> for GetOrder {}

    fn assert_includes<E: Includes<Order>>() {}

    #[test]
    fn includes_should_be_satisfiable_for_a_declared_domain() {
        assert_includes::<GetOrder>();
    }
}
