//! The `SEAL-DIFF` on `SealedHas`'s **recursive** impl, pinned at the position it
//! actually governs.
//!
//! `has_forged_membership_on_malformed_set.rs` pins the head impl's difference. The
//! recursive impl carries its own `SEAL-DIFF` and used to cite
//! `has_cannot_be_forged_at_depth.rs` — but that fixture's set is `(Declared, ())`,
//! **well-formed**, so the dropped `ConsList` bound is satisfied there. It pins the
//! recursion, which is not what the marker drops. The difference itself was untested.
//!
//! What the difference admits (measured, all ACCEPTED downstream): a membership that
//! is **true** on a list whose tail is malformed, at any depth —
//! `impl Has<Other, There<There<Here>>> for (Decl, (Elem, (Other, Junk)))`.
//!
//! What it must not admit, and this is the file: a **false** membership at depth.
//! Below, index 2 of the set is `Other`, not `Undeclared`, so the seal's recursion
//! walks to the head impl and finds `H` tied to the wrong element. Measured at three
//! depths and for a claim on the malformed tail itself — all rejected.
//!
//! If this starts compiling, the justification for both `SealedHas` `SEAL-DIFF`
//! markers has evaporated and `T: ConsList` goes back on the seal, accepting the
//! diagnostic cost that `type-level.md` §2 records.

pub struct Decl;
pub struct Elem;
pub struct Other;
pub struct Undeclared;
pub struct Junk;

/// Index 2 holds `Other`. Claiming `Undeclared` there is false, malformed tail or not.
impl verum::Has<Undeclared, verum::There<verum::There<verum::Here>>>
    for (Decl, (Elem, (Other, Junk)))
{
}

fn main() {}
