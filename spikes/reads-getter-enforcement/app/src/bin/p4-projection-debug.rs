//! P4 — does a `Projection<D, F>`'s `Debug` print **declared fields only**?
//!
//! `read-contract.md` says a projection's derive emits one that does. The first
//! version of this spike deleted that claim, arguing that a derive sees tokens
//! and `F` is a type parameter. This binary is the counter-example: the output
//! is asserted by `run.sh`, not eyeballed.
//!
//! `secret` is populated in both values and must appear in neither.

use app::{DeclaredEmailAndName, DeclaredEmailOnly, Domain, Projection};

fn main() {
    let one: Projection<Domain, DeclaredEmailOnly> =
        Projection::new(Domain::new("e@x", "nm", "SHOULD-NOT-APPEAR"));
    let two: Projection<Domain, DeclaredEmailAndName> =
        Projection::new(Domain::new("e@x", "nm", "SHOULD-NOT-APPEAR"));

    println!("1 field : {one:?}");
    println!("2 fields: {two:?}");
}
