//! Membership resolves at head, middle, and tail — with the index inferred.
//!
//! Paired with the compile_fail cases: without this, an implementation where no
//! membership ever resolved would still show a green suite (docs/rules/test.md §2).
//!
//! Note the caller writes no bound beyond `Has<T, I>` — `ConsList` and `Index`
//! sit on the impls, not the trait, so they need no restatement.

struct A;
struct B;
struct C;

fn requires_member<Set, T, I>()
where
    Set: verum::Has<T, I>,
{
}

fn main() {
    requires_member::<(A, (B, (C, ()))), A, _>();
    requires_member::<(A, (B, (C, ()))), B, _>();
    requires_member::<(A, (B, (C, ()))), C, _>();
}
