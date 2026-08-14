//! A duplicate element makes the index ambiguous (RK-011).
//!
//! `Has<T, Idx>` assumes each element appears exactly once. With `(A, (A, ()))`
//! both `Here` and `There<Here>` satisfy the bound, so `I` cannot be inferred and
//! rustc reports E0283 — an error that says nothing about duplicates.
//!
//! `ConsList` does not catch this: it checks shape, not set-ness. Dedup is
//! entirely the macro's job, and this fixture is what makes a change to that
//! behaviour visible.

struct A;

fn requires_member<Set, T, I>()
where
    Set: verum::Has<T, I>,
{
}

fn main() {
    requires_member::<(A, (A, ())), A, _>();
}
