//! The flagship case: an endpoint declares `Mutates = ()` and then mutates.
//!
//! This is the most common way a contract will be violated in practice, so its
//! message is pinned separately from `has_missing_element.rs` (which starts from a
//! non-empty set).
//!
//! It also pins a limitation. The `help:` line reads `Has<A, _>` — the index is
//! unresolved, because no impl applies at all. That is the *same* signature a
//! malformed set produces, so the index shape cannot be used to tell "element not
//! declared" from "set folded wrongly". `docs/rules/type-level.md` §2 claimed it
//! could; corrected in T-M0-08.

pub struct Mutation;

fn requires_member<Set, T, I>()
where
    Set: verum::Has<T, I>,
{
}

fn main() {
    requires_member::<(), Mutation, _>();
}
