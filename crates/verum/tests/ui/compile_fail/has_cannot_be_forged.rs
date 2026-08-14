//! Forging membership must not compile.
//!
//! This is RK-009 applied to the predicate every capability check runs through:
//! one hand-written impl would grant an endpoint an effect no contract declares,
//! `cargo build` would succeed, and nothing would report it.
//! See docs/specs/unverified-boundaries.md path 14.

struct MyList;
struct MyElem;

impl verum::Has<MyElem, verum::Here> for MyList {}

fn main() {}
