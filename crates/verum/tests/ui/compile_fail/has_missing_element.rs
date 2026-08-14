//! The common failure: the element was never declared.
//!
//! This is the shape almost every real capability error takes — a `Has` bound
//! that is not satisfied because the contract does not list the effect.

struct A;
struct B;
struct C;
struct Undeclared;

fn requires_member<Set, T, I>()
where
    Set: verum::Has<T, I>,
{
}

fn main() {
    requires_member::<(A, (B, (C, ()))), Undeclared, _>();
}
