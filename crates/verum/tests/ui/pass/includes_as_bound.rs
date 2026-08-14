//! The sealed trait is still usable as a bound by downstream code.
//!
//! **Narrower than the other `pass` fixtures.** There is no `impl Includes` outside
//! `#[cfg(test)]` anywhere in `src/`, so nothing downstream can satisfy this bound
//! yet — this file proves only that the bound is *nameable*, not that it is
//! satisfiable. It therefore does **not** rule out "everything fails to compile"
//! for `Includes` the way `pass/cons_list_shapes.rs` does for `ConsList`. The
//! in-crate unit test in `domain.rs` covers satisfiability until the derive exists
//! (M2), at which point this fixture should gain a real impl.
//!
//! Paired with the compile_fail cases so that "everything fails to compile"
//! cannot masquerade as a passing suite (docs/rules/test.md §2).

struct Order;

fn requires_domain_access<E>()
where
    E: verum::Includes<Order>,
{
}

fn main() {}
