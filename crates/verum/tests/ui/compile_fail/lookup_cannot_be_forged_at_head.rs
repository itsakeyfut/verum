//! `Lookup`'s shallowest impl position — the one the suite had no fixture for.
//!
//! `api-surface.md` §2's own table recorded `—` in the shallowest-position column
//! for `Append` and `Lookup` while its neighbouring row said the fixture was
//! mandatory. That `—` was the audit trail of the gap, and a mutation freeing the
//! key in the head seal survived a fully green suite because of it.
//!
//! Two forgeries at the head, both of which compiled before the seal was made
//! `SEAL-EXACT`:
//!
//! The value forgery is the dangerous one. The key is genuine, the map looks right,
//! and only the *answer* is a lie — `Lookup` decides which conditional scope applies
//! to an endpoint, so choosing `Out` chooses the scope. A predicate like `Has` cannot
//! be abused this way: its head impl ties the asserted fact to `Self`'s structure. A
//! trait with `type Out` ties nothing.

pub struct IsPaid;
pub struct RealScope;
pub struct ForgedScope;
pub struct NotAConsList;
pub struct Absent;

/// Case 1 — malformed spine, genuine key. `Lookup` rejects the shape; the seal used
/// to admit it, so the forger could name any value for a key that really exists.
impl verum::Lookup<IsPaid, verum::Here> for ((IsPaid, RealScope), NotAConsList) {
    type Out = ForgedScope;
}

/// Case 2 — **well-formed** map, wrong key. This one is separate on purpose: with the
/// `T: ConsList` bound restored, case 1 now fails on the *shape*, which leaves the
/// thing that actually stops a wrong key — the seal tying `K` to the pair's key —
/// unpinned. Mutation-tested: freeing `K` in the head seal
/// (`SealedLookup<X, Here> for ((K, V), T)`) passed the entire suite until this case
/// existed, and it opens a forgery where a key that is *not in the map* resolves to a
/// value of the forger's choosing.
///
/// Two cases, one purpose (the head position), so sharing a `.stderr` is fine here —
/// unlike `lookup_malformed_map.rs`, where a deliberately-pinned wording shares a file
/// with a case that will legitimately change.
impl verum::Lookup<Absent, verum::Here> for ((IsPaid, RealScope), ()) {
    type Out = ForgedScope;
}

fn main() {}
