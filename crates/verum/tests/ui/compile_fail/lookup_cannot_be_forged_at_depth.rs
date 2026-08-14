//! A forged `Lookup` must fail at every position, not just the head.
//!
//! Mandated by `docs/rules/api-surface.md` §2: a new sealed trait ships with a
//! forgery fixture aimed at its **deepest** impl position. In T-M0-08 the `Has`
//! suite tested only the shallowest one, and a real hole shipped behind that gap.
//!
//! This is the most valuable single fixture for `Lookup`, because `type Out` means
//! a forged impl chooses *which* conditional scope applies. Where a forged `Has`
//! asserts "this effect is declared", a forged `Lookup` answers "here is the entry
//! for that condition" with an entry of the forger's choosing.
//!
//! Verified in both directions: with the seal's recursive impl written
//! unconditionally this file compiles; with `T: private::SealedLookup<K, I>` it
//! does not.

pub struct IsPaid;
pub struct Unrelated;
pub struct ForgedScope;

impl verum::Lookup<IsPaid, verum::There<verum::Here>> for ((Unrelated, Unrelated), ()) {
    type Out = ForgedScope;
}

fn main() {}
