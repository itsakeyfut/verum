//! Why `Has` carries an index parameter at all — pinned as a compiler fact.
//!
//! This fixture deliberately uses **no Verum types**. It reproduces the naive
//! index-free membership formulation locally so that rustc's rejection is what the
//! test asserts. `Has`'s own rustdoc has a `compile_fail` doctest for the same
//! shape, but bare `compile_fail` is satisfied by *any* error, and the
//! `compile_fail,E0119` form is **silently ignored on 1.85.0** (measured: a
//! doctest whose only error is E0308 still passes under it). A UI fixture is the
//! only place the error *code* can be pinned.
//!
//! If a future rustc ever accepts this, the entire justification for the index
//! parameter — and the inference-only `I` it puts on every signature — is gone.
//! This test failing is therefore good news that must not pass unnoticed.

trait NaiveHas<T> {}

impl<H, T> NaiveHas<H> for (H, T) {}

// Overlaps the impl above when `H == X`. The where clause does not separate them,
// because at that intersection `T: NaiveHas<H>` is satisfiable.
impl<H, X, T> NaiveHas<H> for (X, T) where T: NaiveHas<H> {}

fn main() {}
