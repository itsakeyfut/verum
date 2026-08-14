//! Forging a well-formedness proof must not compile.
//!
//! `ConsList` is what says "this set has a shape membership can be tested
//! against". An impl for a local type would assert that about anything — the
//! RK-009 move of writing the missing impl, applied to the shape check.
//!
//! The subject must be a **local struct**: `impl ConsList for (Order, Item)`
//! is stopped by E0117 before the seal is ever consulted, so a tuple would test
//! the orphan rule rather than the seal.

struct MySet;

impl verum::ConsList for MySet {}

fn main() {}
