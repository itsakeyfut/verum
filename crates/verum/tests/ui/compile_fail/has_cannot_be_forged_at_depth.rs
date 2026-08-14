//! A forged membership impl must fail at *every* position, not just the head.
//!
//! `has_cannot_be_forged.rs` covers `Here` and a non-tuple `Self`. Both were
//! already closed by the seal's head impl, and pinning only those let a real hole
//! ship: while the seal's `There<I>` impl was unconditional it held for every
//! 2-tuple, so this file compiled and granted membership no contract declares.
//!
//! Coherence does not catch it — it admits precisely the impls where membership
//! genuinely fails. The seal's recursion is the whole defence, so this fixture is
//! what keeps it there.

pub struct Declared;
pub struct Undeclared;

/// The declared set is `(Declared, ())`. `Undeclared` is in no set at all.
impl verum::Has<Undeclared, verum::There<verum::Here>> for (Declared, ()) {}

fn requires_member<Set, T, I>()
where
    Set: verum::Has<T, I>,
{
}

fn main() {
    requires_member::<(Declared, ()), Undeclared, _>();
}
