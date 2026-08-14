//! A known limitation, pinned deliberately.
//!
//! `(A, B)` is a flat tuple, so it reads as the one-element list `[A]` with tail
//! `B` — and `B` is not a cons list, so no `Has` impl applies. The message says
//! "does not contain `A`" even though `A` is written right there.
//!
//! Measured in T-M0-08: no arrangement of `on_unimplemented` or
//! `do_not_recommend` surfaces `ConsList`'s flat-tuple message through `Has`,
//! and the conditional `on(...)` form is not available on stable. The fix is a
//! declaration-site assertion emitted by the derive (T-M2-09), which will change
//! this file's `.stderr` — that diff is the point of pinning it now.

struct A;
struct B;

fn requires_member<Set, T, I>()
where
    Set: verum::Has<T, I>,
{
}

fn main() {
    requires_member::<(A, B), A, _>();
}
