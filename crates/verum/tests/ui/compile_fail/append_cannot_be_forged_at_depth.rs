//! A forged `Append` must fail even where coherence cannot help.
//!
//! **This position is closed by the seal.** The `.stderr` beside this file says so
//! (E0277 on `SealedAppend`), and it is decisive: with `Append`'s seal supertrait
//! removed, the exact impl below *compiles* downstream (measured).
//!
//! Two earlier versions of this header were wrong in opposite directions — first
//! "coherence rejects it with E0119", then "the orphan rule closes it". The orphan
//! rule closes only the *tuple-`B`* shape (a well-formed `B` is a tuple, hence not
//! local, hence E0117). It does not close this one, and saying it did invited the
//! reader to drop the seal's recursion as redundant — which is precisely bug #8.
//!
//! Mandated by `docs/rules/api-surface.md` §2: every sealed trait ships a forgery
//! fixture at its deepest impl position — and, since #9, at its shallowest too
//! (`append_cannot_be_forged_at_base.rs`), because the shallowest is where the
//! actual hole was.
//!
//! The stakes are higher than for `Has`: `Append` carries `type Out`, so a forged
//! impl does not merely assert a membership — it *names the composed capability
//! set*.

pub struct Granted;
pub struct NotAConsList;

impl verum::Append<Granted> for (NotAConsList, NotAConsList) {
    type Out = (Granted, (Granted, ()));
}

fn main() {}
