//! `SealedHas` is deliberately wider than `Has`. This pins the difference as harmless.
//!
//! `Has`'s seal drops `T: ConsList`, so it holds for lists whose tail is malformed —
//! shapes `Has` itself rejects. That difference is a *licence to forge*, exactly its
//! own size, so it needs a justification and the justification needs a test. This is
//! the test (`SEAL-DIFF` marker on `typelevel.rs`'s `SealedHas<H, Here>` impl).
//!
//! The claim being pinned: **the difference only admits impls asserting a membership
//! that is true anyway.** `Has` is a predicate whose head impl ties `H` to the actual
//! head of `Self`, so a forger cannot use the residual to assert something false —
//! only to restate something already true, on a shape no derive emits.
//!
//! Below, `Undeclared` is *not* the head of the malformed list, so the seal rejects
//! it. If this file ever starts compiling, the justification for the `SEAL-DIFF` has
//! evaporated and the bound must go back on the seal.
//!
//! Contrast `Append` / `Lookup`, whose seals are `SEAL-EXACT`: a trait with
//! `type Out` pins nothing, because the forger names the output, so no difference
//! there could ever be harmless. #9 shipped that difference and it was exploitable in
//! one line.

pub struct RealHead;
pub struct Undeclared;
pub struct NotAConsList;

/// Malformed: the tail is not a cons list. `Has` rejects this shape; the seal admits
/// it. The forgery still fails, because `Undeclared` is not the head.
impl verum::Has<Undeclared, verum::Here> for (RealHead, NotAConsList) {}

fn main() {}
