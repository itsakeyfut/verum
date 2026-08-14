//! A flat tuple cannot be concatenated, and the error says so *at the
//! concatenation*.
//!
//! This is where `Append` differs from `Has`. `Has` deliberately does not carry
//! `ConsList` on its seal, so a malformed set reaches it and gets a misleading
//! "does not contain" message (`has_malformed_set_message_is_misleading.rs`).
//! `Append` does carry `ConsList` on its impls, because here the bound that fails
//! *is* the composition site — so the diagnosis is not misleading and the fix is
//! actionable.
//!
//! Measured: `ConsList`'s own message still does not surface through `Append`
//! (the same dead end recorded for `Has` in T-M0-08), which is why `Append`
//! repeats the flat-tuple note in its own `on_unimplemented`.
//!
//! # A second, misleading error is pinned here on purpose
//!
//! This produces **two** errors where `Has`'s equivalent produces one. The extra
//! one reads "cannot implement a sealed Verum trait", which is nonsense advice for
//! a reader who wrote no impl at all — the seal's recursion fails for the same
//! reason `Append` does.
//!
//! # Where the second error comes from — third attempt, and the measured rule
//!
//! I have twice written a confident wrong answer here, so this states only what was
//! measured. Both the seal and the projection are **necessary**; neither alone is
//! the cause:
//!
//! | variant | errors |
//! |---|---|
//! | as shipped | **2** |
//! | seal's recursive impl opened to `impl<H, T, B>` | **1** |
//! | bound-only (`fn f<X: Append<Y>, Y>()`), malformed **left** | **1** |
//! | projection, malformed **left** (this fixture) | **2** |
//! | projection, malformed **right** | **1** |
//!
//! The last row refutes the explanation I gave second ("projecting an associated
//! type reports the supertrait obligation separately") — that would predict 2 here
//! too. The rule consistent with all five rows:
//!
//! > **The second error appears when the projection's `Self` is the type that fails
//! > the supertrait obligation.**
//!
//! Malformed left: `Self` is `(A, B)`, which fails `SealedAppend` — two errors.
//! Malformed right: the obligation terminates at `for ()`, which satisfies the seal
//! — one error. So sealing *does* contribute a cost here, and my earlier "not a
//! price paid for sealing" was wrong.
//!
//! Two things limit the cost: the actionable error is reported **first**, and both
//! errors are fixed by the same edit. It also disappears for generated sets once
//! T-M2-09 asserts the shape at the declaration site — the cost is only paid by
//! hand-written ones.
//!
//! Not caused by `type Out: ConsList` — measured: dropping that bound leaves both
//! errors, so the bound is kept for free.

pub struct A;
pub struct B;
pub struct C;

/// `(A, B)` is a flat tuple: it reads as head `A`, tail `B`, and `B` is not a
/// cons list.
type Malformed = (A, B);

fn main() {
    let _: core::marker::PhantomData<<Malformed as verum::Append<(C, ())>>::Out> =
        core::marker::PhantomData;
}
