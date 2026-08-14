//! Type-level set representation.
//!
//! This module is private and its items are re-exported flat, so **rustdoc does
//! not render this comment**. The reader-facing documentation lives on
//! [`ConsList`]; keep it there rather than here.

use core::marker::PhantomData;

use crate::private;

/// The element is at the head of the list.
///
/// Index types exist because a naive recursive membership impl violates
/// coherence — the check ignores where clauses, so `H == X` makes the head and
/// tail impls overlap (E0119). Carrying the position as a type parameter keeps
/// them disjoint. See `docs/rules/type-level.md` §2.
///
/// You should never need to name this: the index is always inferred.
///
/// The private field is deliberate. `There<I>` is unconstructible downstream
/// because its `PhantomData` field is private, and an index marker that *can* be
/// built as a value while its sibling cannot reads as permission.
pub struct Here(PhantomData<()>);

/// The element is one position further down than `I`.
///
/// See [`Here`]. `There<There<Here>>` is the third position.
///
/// `fn() -> I` rather than `I`: with a bare `PhantomData<I>` this type inherits
/// `I`'s auto traits, so an index that ever reaches a field of `Ctx<'req, E>`
/// (which must stay `Send`) could silently stop being `Send` because of what it
/// indexes. `fn() -> I` is `Send + Sync` regardless of `I`.
pub struct There<I>(PhantomData<fn() -> I>);

/// A well-formed cons list: `()`, or `(Head, Tail)` where `Tail` is one too.
///
/// # Effect sets are cons lists
///
/// | set | type |
/// |---|---|
/// | empty | `()` |
/// | one element | `(A, ())` |
/// | two elements | `(A, (B, ()))` |
/// | three elements | `(A, (B, (C, ())))` |
///
/// **Never a flat tuple.** `(A, B, C)` cannot support membership testing:
/// positional impls collide at *definition* time with E0119, whether or not a
/// user ever writes a duplicate. And a flat `(A, B)` reads as head `A`, tail
/// `B`, so it **appears to work at exactly two elements** and breaks at three.
/// This trait makes that shape a compile error rather than something a reviewer
/// has to notice.
///
/// # If you are writing a derive
///
/// Emit the nested form. `to_cons_list` folds a declaration list right, ending
/// at `()` — `[A, B]` becomes `(A, (B, ()))`, and an empty declaration becomes
/// `()`, not a unit struct. See `docs/rules/proc-macro.md` §6.
///
/// # What this does *not* check
///
/// Shape only, never set-ness. `(A, (A, ()))` is a well-formed cons list, and a
/// duplicate element still produces E0283 at the membership site (RK-011) — so
/// dedup remains entirely the macro's job. Nesting instead of using `Append`
/// (`((A, ()), (B, ()))`) is also well-formed; it silently *under*-approximates
/// membership, which fails closed but is not caught here.
///
/// # Bind this on impls, not on trait signatures
///
/// Putting it on a trait forces every caller to restate it, which would spread
/// through every generated method — the index parameter already does enough of
/// that.
///
/// The cost of the impl-side placement is diagnostic, not enforcement: when a
/// malformed set reaches `Has`, the bound that fails is `Has`, so **this trait's
/// message never reaches the reader**. `Has` must repeat the flat-tuple note in
/// its own `on_unimplemented`. See `docs/rules/type-level.md` §1.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a well-formed effect set",
    label = "effect sets are cons lists, not flat tuples",
    note = "write `(A, (B, ()))` — a flat tuple `(A, B)` appears to work at exactly two elements and then breaks at three",
    note = "the empty set is `()`, and a single element is `(A, ())`"
)]
pub trait ConsList: private::SealedConsList {}

// Without `do_not_recommend` the only repair rustc suggests is `()` — narrowing
// the effect set to empty, which is RK-010's contract-loosening move arriving via
// the compiler rather than via our own note. The `on_unimplemented` notes above
// already state both `()` and `(A, ())`, so nothing is lost by hiding it.
#[diagnostic::do_not_recommend]
impl ConsList for () {}

// `do_not_recommend` keeps the failing type reported as the whole malformed
// tuple rather than recursing to its tail, and trims the "other impls" list.
// Without it the reader is pointed at `()` (RK-006, `docs/rules/type-level.md` §5).
#[diagnostic::do_not_recommend]
impl<H, T: ConsList> ConsList for (H, T) {}

#[diagnostic::do_not_recommend]
impl private::SealedConsList for () {}

#[diagnostic::do_not_recommend]
impl<H, T: ConsList> private::SealedConsList for (H, T) {}

/// A position within a cons list: [`Here`] or [`There<I>`](There).
///
/// Sealed, so no other type can occupy an index position. This is defence in
/// depth rather than the load-bearing guard — membership itself is sealed — but
/// it costs nothing at use sites and turns a forged index into a direct error
/// instead of a confusing membership failure.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a list index",
    label = "only `Here` and `There<I>` index a cons list",
    note = "index positions are filled by inference — you should not need to name one"
)]
pub trait Index: private::SealedIndex {}

#[diagnostic::do_not_recommend]
impl Index for Here {}

#[diagnostic::do_not_recommend]
impl<I: Index> Index for There<I> {}

#[diagnostic::do_not_recommend]
impl private::SealedIndex for Here {}

#[diagnostic::do_not_recommend]
impl<I: Index> private::SealedIndex for There<I> {}

#[cfg(test)]
mod tests {
    use super::*;

    struct A;
    struct B;
    struct C;

    fn assert_cons_list<L: ConsList>() {}
    fn assert_index<I: Index>() {}

    #[test]
    fn every_set_size_should_be_constructible_as_a_type() {
        assert_cons_list::<()>();
        assert_cons_list::<(A, ())>();
        assert_cons_list::<(A, (B, ()))>();
        assert_cons_list::<(A, (B, (C, ())))>();
    }

    #[test]
    fn index_markers_should_nest_to_any_depth() {
        assert_index::<Here>();
        assert_index::<There<Here>>();
        assert_index::<There<There<Here>>>();
    }
}
