//! The forgery route #9 actually shipped open, and the one rustc dictates.
//!
//! `append_cannot_be_forged_at_depth.rs` aims at a malformed *tail*, which the
//! review found was never the open route — the orphan rule closes that. The route
//! that was open needs no malformed types at all: a **bare local type in the `B`
//! slot**, with a perfectly well-formed `Self`.
//!
//! Why all three guards missed it: the orphan rule permits a local type in a trait
//! argument position; verum's own impl requires `B: ConsList`, so the intersection
//! obligation was unsatisfiable and coherence judged the impls disjoint (no E0119);
//! and the seal's base impl did not constrain `B`.
//!
//! **It is the seal that closes it now** — the `.stderr` says E0277 on
//! `SealedAppend`. An earlier version of this header credited the orphan rule, which
//! is wrong and would read as "the seal bound is redundant here".
//!
//! **This is the base case, so it was the floor under every position.** Because
//! every `Append` chain bottoms out at `for ()`, one such impl rewrote the result of
//! every concatenation in the program — `(D, ()) ++ Sneaky` became
//! `(D, (Admin, ()))`, and `Admin` then satisfied `Has` on a set that never declared
//! it.
//!
//! Worse, **rustc prints the line to write.** The honest error for a missing
//! `Append` names `()` as the `Self` to implement for, because that is where the
//! recursion terminated. RK-009's standard AI repair lands exactly here.

pub struct Declared;
pub struct Admin;
pub struct Sneaky;

impl verum::Append<Sneaky> for () {
    type Out = (Admin, ());
}

fn requires_member<Set, T, I>()
where
    Set: verum::Has<T, I>,
{
}

fn main() {
    requires_member::<<(Declared, ()) as verum::Append<Sneaky>>::Out, Admin, _>();
}
