//! The supertrait must be unreachable from outside the crate.
//!
//! If `private` were nameable, a downstream crate could implement `Sealed`
//! directly and then implement every sealed trait.

use verum::sealed::private::SealedConsList;

fn main() {}
