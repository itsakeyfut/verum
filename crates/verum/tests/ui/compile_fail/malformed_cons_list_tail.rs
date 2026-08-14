//! The malformed part is not always in position 0.
//!
//! `(A, (B, C))` is what a mis-folded `to_cons_list` emits — the outer cell is
//! fine and the tail is broken. This is also where `do_not_recommend` earns its
//! keep: without it the error names the innermost `C` and exposes the recursion.

struct A;
struct B;
struct C;

fn requires_effect_set<L: verum::ConsList>() {}

fn main() {
    requires_effect_set::<(A, (B, C))>();
}
