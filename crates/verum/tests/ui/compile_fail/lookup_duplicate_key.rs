//! A duplicate key makes the index ambiguous — E0283, saying nothing about duplicates.
//!
//! The element-side analogue is `has_duplicate_element.rs`. This is the **key** side,
//! and it is worse in a way `Has`'s reasoning does not cover: with
//! `((K, VA), ((K, VB), ()))`, writing the index explicitly compiles *and picks a
//! different value* — `Here` → `VA`, `There<Here>` → `VB` (pinned by a unit test in
//! `typelevel.rs`). For `Has`, both indices prove membership of the same type, which
//! is why its rustdoc can call a hand-written index harmless. Here the two answers
//! are different conditional scopes.
//!
//! M8 keys this map by condition type, so two `when(IsPaid)` blocks with different
//! effect sets compile, and inference then fails here with an error about type
//! annotations. Dedup is the macro's job for keys as well as elements — the reason
//! this is pinned at layer 3 rather than fixed is the same as for
//! `append_duplicate_breaks_membership.rs`: layer 3 is the backstop for hand-written
//! and future generated maps.

pub struct IsPaid;
pub struct FirstScope;
pub struct SecondScope;

type DuplicatedCondition = ((IsPaid, FirstScope), ((IsPaid, SecondScope), ()));

fn lookup<Map, K, I>()
where
    Map: verum::Lookup<K, I>,
{
}

fn main() {
    lookup::<DuplicatedCondition, IsPaid, _>();
}
