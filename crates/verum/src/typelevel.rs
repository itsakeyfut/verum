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
/// 1.85.0, same impl shape, only the tail differs — **measured with the seal's
/// recursive impl removed**, which is what isolates coherence's behaviour. With the
/// seal as shipped, both rows are rejected by the seal instead, so a reader
/// reproducing this today sees E0277 and not the table below:
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

// SEAL-EXACT: mirrors `ConsList for ()`.
#[diagnostic::do_not_recommend]
impl private::SealedConsList for () {}

// SEAL-EXACT: mirrors `ConsList for (H, T)`, `T: ConsList` included.
//
// Note what closes this position, because the marker alone reads as if the bound
// does: dropping the recursion here is an **equivalent mutant** — no test fails —
// because `impl ConsList for (L1, L2)` is rejected by the **orphan rule** (E0117).
// Tuples are not `#[fundamental]`, so a tuple of local types is not a local type.
// The bound is kept for exactness, not because it is the load-bearing guard.
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

// SEAL-EXACT: mirrors `Index for Here`.
#[diagnostic::do_not_recommend]
impl private::SealedIndex for Here {}

// SEAL-EXACT: mirrors `Index for There<I>`, `I: Index` included.
//
// Same equivalent-mutant note as `SealedConsList` above: `impl Index for There<L>`
// is E0117, so the orphan rule closes this position and dropping the recursion
// fails no test. Kept for exactness.
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
// SEAL-DIFF: drops `T: ConsList`, so this seal holds for malformed tails that
// `Has` itself rejects. Justified because `Has` is a *predicate* whose head impl
// pins the fact it asserts — `H` must literally be the head of `Self` — so the
// difference set only admits impls claiming a membership that is true anyway. A
// trait with `type Out` has no such pin and gets no such licence.
// Keeping the drop buys the diagnostic: with `ConsList` here a malformed set fails
// on the seal and reads "not sealed by Verum", which is wrong and unactionable.
// fixture: has_forged_membership_on_malformed_set.rs
#[diagnostic::do_not_recommend]
impl<H, T> private::SealedHas<H, Here> for (H, T) {}

// SEAL-DIFF: drops `ConsList` / `Index` for the same reason as the head impl above
// — they are diagnostic-only, and the INVARIANT there covers this position too. The
// recursion is kept, because that is the enforcement.
//
// The fixture below is the one that exercises *this* impl's difference. It used to
// cite `has_cannot_be_forged_at_depth.rs`, whose set is well-formed — so the dropped
// bound was satisfied there and the difference went untested. Review caught that;
// worth remembering that "a fixture is cited" and "the difference is pinned" are not
// the same claim.
// fixture: has_forged_membership_at_depth_on_malformed_set.rs
#[diagnostic::do_not_recommend]
impl<H, X, T: private::SealedHas<H, I>, I> private::SealedHas<H, There<I>> for (X, T) {}

/// Concatenation: `Self` followed by `B`.
///
/// This is how a `when` scope's capabilities compose with the endpoint's
/// (`ctx.when::<C>`, M8):
///
/// ```ignore
/// type Mutates = <E::Mutates as Append<CondMutates>>::Out;
/// ```
///
/// # No index parameter here
///
/// Unlike [`Has`], this needs none. The two impls target `()` and `(H, T)`,
/// which are structurally disjoint, so they cannot overlap however the type
/// parameters are instantiated. `Has` needed an index only because *both* of its
/// impls target `(_, _)`.
///
/// # The result is always well-formed
///
/// `Out` is bounded by [`ConsList`], so a caller gets that guarantee without
/// restating a bound, and `Append` can never be the source of a malformed set.
/// [`Has`] cannot offer this — it has no output to constrain.
///
/// # It cannot deduplicate, and that is structural
///
/// Removing duplicates means branching on an element being *absent* from `B`, and
/// that needs a **total** membership decision. There is none: the catch-all impl
/// collides (E0119) and the index witness has nowhere to live (E0207). [`Has`]
/// works precisely because it is a *partial* relation — "absent" is "no impl",
/// which cannot be branched on. This is a language-level impossibility, not a
/// policy choice: `Subset` is banned for **cost** (`docs/rules/type-level.md` §3),
/// and citing that ban as the reason would suggest dedup becomes available if the
/// rule were relaxed. It does not.
///
/// So `(A, ()) ++ (A, ())` yields `(A, (A, ()))` silently, and the resulting E0283
/// appears later at a [`Has`] site, far from the cause.
///
/// **Dedup is therefore unconditionally the macro's job** (RK-011). The hazard is
/// real for a *valid* contract — `emits = [X]` at the top level together with
/// `when(C) => { emits = [X] }` — which is why M8's dedup is a completion
/// condition rather than a nicety.
#[diagnostic::on_unimplemented(
    message = "`{Self}` and `{B}` cannot be concatenated",
    label = "both sides must be well-formed effect sets",
    note = "write `(A, (B, ()))` — a flat tuple `(A, B)` appears to work at exactly two elements and then breaks at three",
    note = "the empty set is `()`, and a single element is `(A, ())`"
)]
pub trait Append<B>: private::SealedAppend<B> {
    /// The concatenation. Guaranteed to be a well-formed cons list.
    type Out: ConsList;
}

#[diagnostic::do_not_recommend]
// `B: ConsList` here is load-bearing — this is the bound that makes `Out: ConsList`
// satisfiable and that rejects a malformed right-hand operand. Removing it stops
// verum itself from building (measured).
impl<B: ConsList> Append<B> for () {
    type Out = B;
}

// No `ConsList` bounds here, and that is measured rather than an oversight.
// `Out: ConsList` on the trait plus `B: ConsList` on the base impl above is
// already sufficient: every `Append` chain must bottom out at `()`, so both
// operands and the result are forced well-formed from there. Adding `T: ConsList`
// and `B: ConsList` to this impl changed nothing — verum built, every test passed,
// and every `.stderr` was byte-identical.
#[diagnostic::do_not_recommend]
impl<H, T: Append<B>, B> Append<B> for (H, T) {
    type Out = (H, <T as Append<B>>::Out);
}

// SEAL-EXACT: mirrors `Append`'s impls bound for bound.
//
// The earlier version dropped `B: ConsList` here, reasoning that shape bounds are
// diagnostic-only (as they are for `SealedHas`). That shipped a forgery hole, and
// the reasoning error is worth naming because it is easy to repeat:
//
// **A bound on verum's impl constrains verum, not downstream.** `B: ConsList` on
// `impl Append<B> for ()` decides who *verum* implements `Append` for; a foreign
// impl never has to satisfy it. Only three things constrain a foreign impl — the
// trait declaration, the seal, and the orphan rule. So a bound that is
// "load-bearing" in the sense that removing it breaks verum's own build can still
// be worth nothing to security, and those two properties must never be conflated.
//
// With `B` unconstrained here, `impl Append<Local> for () { type Out = whatever }`
// compiled downstream: the orphan rule allows a local type in `B`, verum's own
// impl needs `B: ConsList` so the intersection obligation was unsatisfiable and
// coherence stood aside, and the seal never looked at `B`. Because every `Append`
// chain bottoms out here, that one line rewrote the result of *every*
// concatenation. Verified, including that rustc's own error names `()` as the type
// to implement for.
//
// Putting the bound on the trait declaration (`pub trait Append<B: ConsList>`)
// also closes it, but measured: it forces every generic caller to restate the
// bound, which is what `docs/rules/type-level.md` §1 rejects. The seal has no
// use-site cost.
//
// The safety here is a *chain*, not this bound alone, and it is worth naming
// because the last link is in another type: the seal requires `B: ConsList`;
// `ConsList` is itself sealed and holds only for `()` and tuple spines; every such
// type is built-in, hence never a local type, hence orphan-unreachable. So any `B`
// satisfying the seal cannot be forged on, and any `B` that could be forged on
// fails the seal. **If `ConsList` ever gains an impl for a non-tuple shape, this
// route reopens** — measured, including the two-step attack of forging `ConsList`
// for a local type first (rejected by `SealedConsList`).
#[diagnostic::do_not_recommend]
impl<B: ConsList> private::SealedAppend<B> for () {}

// SEAL-EXACT: mirrors `Append for (H, T)` — `T: Append<B>` becomes
// `T: SealedAppend<B>`. `B: ConsList` is inherited through the base impl above,
// which every chain must reach.
#[diagnostic::do_not_recommend]
impl<H, T: private::SealedAppend<B>, B> private::SealedAppend<B> for (H, T) {}

/// Type-level map search: the value stored under `K` in `Self`, at position `Idx`.
///
/// The map is a cons list of `(Key, Value)` pairs — `((K1, V1), ((K2, V2), ()))`.
/// M8 uses it to pull the matching `When<C, ..>` out of an endpoint's
/// `Conditional` by its condition, so the derive emits
/// `((C, When<C, ..>), rest)`.
///
/// # Why pairs rather than keyed entries
///
/// The alternative — entries that declare their own key through a `Keyed` trait —
/// removes the redundancy of writing `C` twice, but it would mean this module
/// knowing about `When`, and `typelevel` is the bottom of the dependency chain
/// (`docs/rules/design.md` §2). Pairs keep it ignorant.
///
/// **The retreat exists, but only one way round it.** Measured:
///
/// | how `Keyed` is added later | result |
/// |---|---|
/// | as a **separate** trait (`LookupKeyed`) beside this one | additive, no conflict |
/// | as another impl **of this trait** | compiles today, then E0119 the moment anyone implements `Keyed` for a 2-tuple |
///
/// The second is the one a future implementer would reach for first, and it is a
/// *delayed* coherence break — fine until an unrelated impl lands. If `Keyed`
/// arrives, it gets its own trait.
///
/// # This failing *is* a guarantee
///
/// "A conditional effect that was never declared cannot fire" is enforced by this
/// lookup not resolving (`docs/specs/conditional-effects.md`). The wording stays
/// generic because this layer does not know what a condition is; M8 puts
/// condition-specific guidance on `Condition`, which fails on a different bound
/// and composes with this one.
///
/// # Duplicate keys select a *different value*, and `Has`'s argument does not carry
///
/// [`Has`]'s rustdoc says a hand-written index on a duplicate is harmless because
/// "nothing depends on which — they are the same type". **That does not transfer
/// here.** With `((K, VA), ((K, VB), ()))`, measured:
///
/// | index | `Out` |
/// |---|---|
/// | `Here` | `VA` |
/// | `There<Here>` | `VB` |
/// | inferred | E0283 |
///
/// Both explicit forms compile and give *different* types, so a written-out index
/// silently picks one of two conditional scopes. M8 keys this map by condition type,
/// so two `when(IsPaid)` blocks with different effect sets would compile and one
/// would be chosen silently. **Dedup is the macro's job for keys as well as for
/// elements** (RK-011 covers the element case; this is the key case).
///
/// # Index parameter
///
/// Required, for exactly [`Has`]'s reason — both impls target `(_, _)`.
#[diagnostic::on_unimplemented(
    message = "`{Self}` has no entry for key `{K}`",
    label = "`{K}` is not a key in this map",
    note = "add the entry to the declaration, or remove the lookup that requires it"
)]
pub trait Lookup<K, Idx>: private::SealedLookup<K, Idx> {
    /// The value found under `K`. Deliberately unbounded — the values are
    /// `When<..>` entries, not cons lists.
    type Out;
}

// `T: ConsList` here is the load-bearing one, and it is on this impl rather than
// on both for a reason worth stating once: **the shape bound belongs where the
// recursion terminates.** Every `Lookup` chain ends at this head impl, so the
// spine is checked from here. Repeating it on the recursive impl below is
// redundant — `T: Lookup<K, I>` already forces `T` to be a 2-tuple, and its own
// resolution must bottom out here. Same for `I: Index`: impls exist only for
// `Here` and `There<_>`, so a non-index cannot satisfy the recursive premise.
// Both measured — adding them back changed no behaviour and no `.stderr`.
//
// `Append` follows the same rule: its shape bound sits on the `for ()` base impl.
//
// (`Has` still carries `I: Index` on its recursive impl. It is equally redundant
// there; it survives only because `has_duplicate_element.stderr` reproduces that
// impl's signature verbatim, so removing it shows up as a text diff. Left alone —
// out of scope here.)
#[diagnostic::do_not_recommend]
impl<K, V, T: ConsList> Lookup<K, Here> for ((K, V), T) {
    type Out = V;
}

#[diagnostic::do_not_recommend]
impl<K, X, T: Lookup<K, I>, I> Lookup<K, There<I>> for (X, T) {
    type Out = <T as Lookup<K, I>>::Out;
}

// SEAL-EXACT: mirrors `Lookup`'s head impl, `T: ConsList` included. Dropping it
// let a downstream crate write `impl Lookup<K, Here> for ((K, V), NotAConsList)`
// with an `Out` of its choosing — the key is genuine, the *value* is a lie. That is
// strictly worse than `SealedHas`'s residual, which only ever admits a membership
// that is true anyway: a predicate pins the fact it asserts, whereas a trait with
// `type Out` pins nothing, because the forger names the output.
#[diagnostic::do_not_recommend]
impl<K, V, T: ConsList> private::SealedLookup<K, Here> for ((K, V), T) {}

// SEAL-EXACT: mirrors `Lookup for (X, T)` — `T: Lookup<K, I>` becomes
// `T: SealedLookup<K, I>`. `T: ConsList` is inherited through the head impl above,
// where every chain terminates.
#[diagnostic::do_not_recommend]
impl<K, X, T: private::SealedLookup<K, I>, I> private::SealedLookup<K, There<I>> for (X, T) {}

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

    /// Asserts that two types are the *same* type. `X: IsSame<Y>` only holds via
    /// the reflexive impl, so this fails to compile unless `X == Y` — which is
    /// what makes it an assertion about `Append`'s output rather than about
    /// whether some bound happens to be satisfiable.
    trait IsSame<T> {}
    impl<T> IsSame<T> for T {}
    fn assert_same<X: IsSame<Y>, Y>() {}

    #[test]
    fn append_should_concatenate_at_every_size() {
        assert_same::<<() as Append<()>>::Out, ()>();
        assert_same::<<() as Append<(A, ())>>::Out, (A, ())>();
        assert_same::<<(A, ()) as Append<()>>::Out, (A, ())>();
        assert_same::<<(A, ()) as Append<(B, ())>>::Out, (A, (B, ()))>();
        assert_same::<<(A, (B, ())) as Append<(C, ())>>::Out, (A, (B, (C, ())))>();
    }

    #[test]
    fn append_output_should_always_be_a_well_formed_set() {
        // The generic form is the one that actually exercises `Out: ConsList`.
        // Applying `ConsList` to a *concrete* projection result proves nothing —
        // measured: with `type Out: ConsList` weakened to `type Out;`, the concrete
        // version still passes, because the resulting tuple satisfies `ConsList`
        // through its own impl. Here `X::Out` is opaque, so the bound is the only
        // thing that can discharge it.
        fn requires_well_formed_generically<X: Append<Y>, Y>() {
            assert_cons_list::<<X as Append<Y>>::Out>();
        }
        requires_well_formed_generically::<(A, (B, ())), (C, ())>();
        assert_cons_list::<<(A, (B, ())) as Append<(C, ())>>::Out>();
    }

    /// A duplicate key is resolvable *and* ambiguous: each explicit index picks a
    /// different value. Pinned so that a silent change is visible, in the style of
    /// `append_should_produce_duplicates_rather_than_deduplicating`.
    #[test]
    fn lookup_duplicate_keys_should_resolve_to_different_values_per_index() {
        struct K;
        type DupMap = ((K, A), ((K, B), ()));

        fn lookup_is<M, Key, I, Expected>()
        where
            M: Lookup<Key, I>,
            <M as Lookup<Key, I>>::Out: IsSame<Expected>,
        {
        }

        lookup_is::<DupMap, K, Here, A>();
        lookup_is::<DupMap, K, There<Here>, B>();
    }

    #[test]
    fn append_should_produce_duplicates_rather_than_deduplicating() {
        // Not a wish, a constraint: dedup would need to branch on an element being
        // *absent*, which requires a total membership decision that does not exist
        // (E0119 + E0207 — see `Append`'s rustdoc). Pinned so that if this ever
        // silently changes, it changes visibly.
        assert_same::<<(A, ()) as Append<(A, ())>>::Out, (A, (A, ()))>();
    }

    #[test]
    fn lookup_should_retrieve_at_every_position() {
        struct K1;
        struct K2;
        struct K3;

        type Map = ((K1, A), ((K2, B), ((K3, C), ())));
        fn assert_lookup<M, K, I, Expected>()
        where
            M: Lookup<K, I>,
            <M as Lookup<K, I>>::Out: IsSame<Expected>,
        {
        }

        assert_lookup::<Map, K1, _, A>();
        assert_lookup::<Map, K2, _, B>();
        assert_lookup::<Map, K3, _, C>();
    }
}
