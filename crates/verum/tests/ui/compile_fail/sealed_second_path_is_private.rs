//! The seal is reachable by two names; both must stay closed.
//!
//! `lib.rs` re-exports the module as `pub(crate) use sealed::private`, so
//! `verum::private::Sealed` is a second path to the same trait. Changing that
//! one line to `pub use` would open it — and the other compile_fail case would
//! not notice, because it names `verum::sealed::private` instead.

use verum::private::SealedIncludes;

fn main() {}
