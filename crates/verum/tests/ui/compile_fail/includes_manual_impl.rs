//! Forging the Architecture Contract must not compile.
//!
//! `User` is a local type, so the orphan rule permits this impl. Without
//! sealing it would build cleanly and silently grant `User` access to `Order`.
//! See docs/specs/unverified-boundaries.md path 13.

struct Order;
struct User;

impl verum::Includes<Order> for User {}

fn main() {}
