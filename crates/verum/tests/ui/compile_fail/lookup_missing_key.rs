//! A key that is not in the map does not resolve — and this failure *is* a
//! guarantee, not an inconvenience.
//!
//! `docs/specs/conditional-effects.md` states "a conditional effect that was never
//! declared cannot fire", and the mechanism is exactly this: the effect's
//! condition is not a key in the endpoint's `Conditional` map, so `Lookup` finds
//! nothing and the code does not compile.
//!
//! The wording is generic on purpose. `typelevel` is the bottom of the dependency
//! chain and does not know what a condition is; M8 puts condition-specific
//! guidance on `Condition`, which fails on a different bound and composes with
//! this message rather than replacing it.

pub struct IsPaid;
pub struct IsShipped;
pub struct PaidEffects;

/// A map declaring one condition. `IsShipped` was never declared.
type Conditional = ((IsPaid, PaidEffects), ());

fn lookup<Map, K, I>() -> core::marker::PhantomData<<Map as verum::Lookup<K, I>>::Out>
where
    Map: verum::Lookup<K, I>,
{
    core::marker::PhantomData
}

fn main() {
    let _ = lookup::<Conditional, IsShipped, _>();
}
