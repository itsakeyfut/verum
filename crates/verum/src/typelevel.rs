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
/// coherence: at `H == X` the head and tail impls overlap and the tail's
/// `T: Has<H>` obligation *is* satisfiable there, so it does not separate them
/// (E0119). Carrying the position as a type parameter keeps them disjoint
/// structurally. See `docs/rules/type-level.md` §2.
///
/// Note the shape of that reasoning, because it is narrower than "coherence
/// ignores where clauses" and the difference is load-bearing. Compile-verified on
/// 1.85.0, same impl shape, only the tail differs:
///
/// | downstream `impl Has<Undeclared, There<Here>> for (Decl, TAIL)` | obligation on `TAIL` | rustc |
/// |---|---|---|
/// | `TAIL = ()` | unsatisfiable | **accepted** |
/// | `TAIL = (Undeclared, ())` | satisfiable | E0119 |
///
/// So the overlap check *does* consult where clauses, and it admits exactly the
/// impls where membership genuinely fails — the harmful ones. Coherence is
/// therefore no part of the defence against a forged membership impl; the seal is
/// the whole of it, which is why [`Has`]'s seal must carry the recursion.
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

/// Membership: `Self` contains `T` at position `Idx`.
///
/// This is the predicate every capability check runs through — "is this effect
/// in the set the endpoint declared?".
///
/// # Why there is an index parameter
///
/// The obvious formulation does not compile: these two are judged overlapping
/// when `H == X`, because the tail's `T: Has<H>` obligation is satisfiable at the
/// intersection and so does not separate them. (**Not** because where clauses are
/// ignored — see [`Here`]; that distinction is what the seal depends on.)
///
/// ```compile_fail
/// # trait Has<T> {}
/// impl<H, T> Has<H> for (H, T) {}
/// impl<H, X, T> Has<H> for (X, T) where T: Has<H> {}
/// // error[E0119]: conflicting implementations of trait `Has<_>` for type `(_, _)`
/// ```
///
/// Carrying the position as a type parameter separates them structurally. The
/// price is that **every signature using `Has` gains an inference-only `I`** —
/// derive writes those, but hand-written framework signatures must carry it:
///
/// ```ignore
/// fn set_email<I>(&self, ..) where M: Has<Mutate<User, user::Email>, I>;
/// ```
///
/// # Duplicates break it
///
/// The index assumes each element appears **exactly once**. A duplicate leaves
/// `I` ambiguous and produces E0283, which reads as an unrelated error. Dedup is
/// the macro's job (RK-011); [`ConsList`] does not help — it checks shape, not
/// set-ness.
///
/// Note that a *written-out* index disambiguates rather than erroring: for
/// `(A, (A, ()))` both `Here` and `There<Here>` compile. So a hand-written index
/// silently picks one of the two duplicates. Nothing depends on which — they are
/// the same type — but it means E0283 is not a reliable signal that duplicates
/// exist, only that inference had to choose.
///
/// # What a failure here does *not* tell you
///
/// A malformed set also fails this bound, and the message will say "does not
/// contain" even when the element is written in the set — because a flat
/// `(A, B)` is read as a one-element list. Measured: no arrangement of
/// `on_unimplemented` or `do_not_recommend` surfaces [`ConsList`]'s message
/// through `Has`, and the conditional `on(...)` form is not available on stable.
/// The fix belongs at the declaration site, where the derive asserts the shape
/// once (T-M2-09), rather than in every membership error.
#[diagnostic::on_unimplemented(
    message = "`{Self}` does not contain `{T}`",
    label = "`{T}` is not a member of this set",
    note = "declare `{T}` in the contract, or remove the call that requires it"
)]
pub trait Has<T, Idx>: private::SealedHas<T, Idx> {}

#[diagnostic::do_not_recommend]
impl<H, T: ConsList> Has<H, Here> for (H, T) {}

#[diagnostic::do_not_recommend]
impl<H, X, T: ConsList, I: Index> Has<H, There<I>> for (X, T) where T: Has<H, I> {}

// The seal impls mirror the two above, and the distinction between what they may
// drop and what they may not is load-bearing:
//
// - `ConsList` / `Index` are **dropped on purpose**. They are diagnostic-only
//   here: with them, a malformed set fails on the *seal* and reads "not sealed by
//   Verum", which is wrong and unactionable; without them it fails on `Has` and
//   gets the membership message. Measured both ways.
// - **The recursion is kept**, because it is the enforcement. An unconditional
//   `SealedHas<H, There<I>>` holds for *every* 2-tuple, which makes membership
//   forgeable from downstream at any position but the head: `H` is unconstrained,
//   so `impl Has<Undeclared, There<Here>> for (Declared, ())` satisfies the seal.
//   Coherence does **not** cover this — see the note on `Here`.
//
// The rule this instance produced: a seal's impls must be no more permissive than
// the sealed trait's, recursion included. `docs/rules/api-surface.md` §2.
//
// INVARIANT: `SealedHas<H, I>` holds iff `H` genuinely sits at index `I` in
// `Self` — even when `Self`'s tail is ill-formed, since the seal drops
// `ConsList`. That is exactly why no *false* membership can be written by hand,
// and why dropping `ConsList` is safe: the residual permissiveness only admits
// impls asserting a membership that is true anyway, on a list shape no derive
// emits, and it cannot affect a well-formed set (all measured).
#[diagnostic::do_not_recommend]
impl<H, T> private::SealedHas<H, Here> for (H, T) {}

#[diagnostic::do_not_recommend]
impl<H, X, T: private::SealedHas<H, I>, I> private::SealedHas<H, There<I>> for (X, T) {}

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
    fn membership_should_resolve_at_every_position() {
        fn assert_has<Set, T, I>()
        where
            Set: Has<T, I>,
        {
        }

        type Three = (A, (B, (C, ())));
        assert_has::<Three, A, _>();
        assert_has::<Three, B, _>();
        assert_has::<Three, C, _>();
    }

    #[test]
    fn index_markers_should_nest_to_any_depth() {
        assert_index::<Here>();
        assert_index::<There<Here>>();
        assert_index::<There<There<Here>>>();
    }
}
