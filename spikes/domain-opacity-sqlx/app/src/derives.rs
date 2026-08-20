//! #44 / ledger paths 26 and 28 — the derives the **user** attaches, and the
//! insides of a field's **type**.
//!
//! Cause 1's remedy is "make the Domain opaque". Neither of these routes touches
//! `Repr`, so neither is path 21, and both survive any `Repr` redesign. What the
//! probes here isolate is *what actually closes them*, which is not what #44
//! assumed and not what `read-contract.md` promised.
//!
//! THE QUESTION P42 AND P46 SPLIT
//!   #44 measured that an attribute can see its sibling derives and a derive
//!   cannot, and concluded that the attribute form "can enforce" the ban on
//!   `Deserialize`. That is true of one position only. `#[domain]` is expanded
//!   before the item's derives, but a derive listed **above** it is collected by
//!   rustc independently — dropping it from the re-emitted item does not suppress
//!   it. So the layer-1 check reaches the derives below the attribute (P46) and
//!   is blind to the ones above it (P42).

// ---------------------------------------------------------------------------
// P42 — a forbidden derive written ABOVE `#[domain]`, the position the layer-1
// check cannot see.
//
// Expected to FAIL with **`E0119`**: the expansion emits its own `impl Default`
// and `impl Clone`, so the user's derive collides with them. Coherence does not
// consult the check, the attribute's token stream, or the spelling — it is the
// one mechanism that covers this position.
//
// run.sh needles on `E0119` AND on the ABSENCE of verum's layer-1 wording. The
// second half is what keeps the row honest: it fails if someone concludes the
// name-based check reaches here. It does not, and P47 shows it cannot.
//
// The first version of this row needled on `E0560` — the derive naming fields the
// attribute had deleted — and recorded that as what rejects the position. It is
// real but *accidental*: it disappears the moment `#[domain]` preserves field
// names (probe P43 compiles and forges under exactly that shape). #44's review
// found the collision, which is neither accidental nor shape-dependent.
// ---------------------------------------------------------------------------
#[cfg(feature = "p42-derive-above-attribute")]
#[derive(Default)]
#[mac::domain_attr]
pub struct AboveAccount {
    pub email: String,
}

// ---------------------------------------------------------------------------
// P46 — the same derive written BELOW `#[domain]`, which the attribute can see.
//
// Expected to FAIL with verum's layer-1 wording. This is requirement 6 of #44
// measured: the attribute form *can* enforce `read-contract.md`'s ban. The
// trybuild fixture belongs with the real `#[domain]` (T-M2-04); this is what can
// be measured before it exists.
// ---------------------------------------------------------------------------
#[cfg(feature = "p46-derive-below-attribute")]
#[mac::domain_attr]
#[derive(Default)]
pub struct BelowAccount {
    pub email: String,
}

// ---------------------------------------------------------------------------
// P43 — the SHAPE-PRESERVING `#[domain]`, in the user's own module.
//
// `mutation-contract.md` describes a Domain with private fields, not necessarily a
// newtype. Under that shape the derive above the attribute generates its impl in
// the module where the fields are private-but-reachable, so it compiles — and the
// forgery is then callable from any crate. `separate-repo`'s `derives.rs` is the
// half that runs it.
//
// `Deserialize` is in the list and not only `Default`, for P41's reason: `Default`
// invents a value the attacker cannot choose, so a probe on it alone would print an
// empty string and could not tell a real forgery from an empty one. From a JSON
// string the attacker picks the values, which is what #44 records and what makes
// the run probe's needle unforgeable by an empty test body.
//
// Expected to COMPILE. That is the defect.
// ---------------------------------------------------------------------------
#[cfg(feature = "p43-keep-shape-same-module")]
#[derive(Default, Clone, serde::Deserialize)]
#[mac::domain_keep_shape]
pub struct SameModuleAccount {
    pub email: String,
}

// ---------------------------------------------------------------------------
// P44 — the same shape, the same derive, in the same position. The ONLY change is
// that the attribute emits into a macro-owned child module (ADR-0010's radius).
//
// Expected to FAIL with **`E0616`** — `Clone`'s generated code *reads* the field.
// With `Default` alone the same source is `E0451` (a struct literal *constructs*
// it); both were measured in place, and the harness pins `E0616` because that is
// what this derive list produces. An earlier version of this comment said `E0451`
// while `run.sh`, the README and the ledger all said `E0616` — the wrong-error-code
// drift #34's review recorded, in the same change that claimed to have avoided it.
//
// **Placement is the mechanism, for derives whose generated code names a field.**
// P49 is the counter-case: `Copy` names none, so placement does not reach it.
// ---------------------------------------------------------------------------
#[cfg(feature = "p44-keep-shape-confined")]
#[derive(Default, Clone)]
#[mac::domain_keep_shape_confined]
pub struct ConfinedAccount {
    pub email: String,
}

// ---------------------------------------------------------------------------
// P45 — ledger path 28. The interior-mutability whitelist cannot see through an
// alias, and `&self` is all a mutation needs.
//
// `AuditTrail` is what a field-type whitelist reads: a derive sees *tokens*, and
// nothing resolves the alias for it. Path 5's remedy ("restrict domain field
// types") is a name check standing in front of a name the user chose.
//
// The mutation then goes through the `&self` getter, so it is available where
// `Mutates = ()` — a GET. Expected to COMPILE, and `separate-repo` runs it.
// ---------------------------------------------------------------------------
#[cfg(feature = "p45-alias-interior-mutability")]
pub type AuditTrail = core::cell::RefCell<Vec<String>>;

#[cfg(feature = "p45-alias-interior-mutability")]
#[mac::domain_keep_shape_confined]
pub struct Order {
    audit: crate::derives::AuditTrail,
}


// ---------------------------------------------------------------------------
// P47 — the two spellings that defeated the name-based check, in the position it
// cannot see anyway.
//
// `r#Default` made `to_string()` yield `"r#Default"` and matched nothing; an
// aliased import matches nothing in principle, because a proc macro resolves no
// names. Both are now `E0119`: **coherence does not read spellings.** This row is
// what stops the ledger from describing the check as the defence.
// ---------------------------------------------------------------------------
#[cfg(feature = "p47-spelling-independent")]
pub use core::clone::Clone as Duplicate;

#[cfg(feature = "p47-spelling-independent")]
#[derive(r#Default, crate::derives::Duplicate)]
#[mac::domain_attr]
pub struct SpelledAccount {
    pub email: String,
}

// ---------------------------------------------------------------------------
// P48 — `Copy`, and the one place a check on it is position-independent.
//
// Emitting `Clone` to close path 26 **removed the incidental barrier that had been
// stopping `Copy`**: `#[derive(Copy)]` requires `Self: Clone`, which used to be
// unsatisfied (`E0277`). A bit-copy duplicates the Domain without calling the
// emitted `clone`, so the `unimplemented!()` body is no defence.
//
// `repr_derive(..)` is the **attribute's own argument list**, so rejecting `Copy`
// there works in every position — unlike a sibling derive. Expected to FAIL with
// verum's wording.
// ---------------------------------------------------------------------------
#[cfg(feature = "p48-repr-copy-rejected")]
#[mac::domain_attr(repr_derive(Clone, Copy))]
pub struct CopyAccount {
    pub id: u64,
}

// ---------------------------------------------------------------------------
// P49 — the structural half, and P48's control.
//
// Without `repr_derive(Copy)` the `Repr` carries no derive, so `Copy` on the
// newtype is `E0204` regardless of where the derive is written. That is why P48's
// route is the *only* one, and why the check there is sufficient rather than
// merely better. It is also the counter-case to "placement is the mechanism":
// `Copy`'s generated code names no field, so neither the newtype mismatch nor the
// field privacy that reject `Default`/`Clone` applies to it.
// ---------------------------------------------------------------------------
#[cfg(feature = "p49-copy-blocked-structurally")]
#[derive(Copy)]
#[mac::domain_attr]
pub struct StructuralCopy {
    pub id: u64,
}

// ---------------------------------------------------------------------------
// P50 — the residue the collision cannot reach: `Deserialize`.
//
// verum cannot emit `impl serde::Deserialize` because it does not depend on serde
// (`repr_derive` exists for that reason), so `Default`/`Clone`'s mechanism is
// unavailable here and the name-based check is all there is — with both of its
// limits. This row exists because the review measured that narrowing
// `FORBIDDEN_DERIVES` to `["Default"]` left every probe green: `Clone` and
// `Deserialize` were in the list and nothing exercised them. `Clone` is now
// covered by P42/P47; this covers the one that requirement 6 of #44 names.
// ---------------------------------------------------------------------------
#[cfg(feature = "p50-deserialize-below")]
#[mac::domain_attr]
#[derive(serde::Deserialize)]
pub struct JsonAccount {
    pub email: String,
}

// ---------------------------------------------------------------------------
// P51 / P52 — path 28's remedy, measured in both forms on the SAME input.
//
// P45 measures the `&self` escalation and **not** the alias: replacing the alias
// with the type written out keeps it green, and there was no whitelist in this
// spike to defeat. These two rows are what make the ledger's remedy table a
// measurement instead of desk analysis.
//
// Path 5's remedy has two horns and both are measured here:
//   P51 — an **allow-list** of permitted field-type names rejects the user's own
//         value object (`Email`). Too narrow, and unimplementable as specified.
//   P53 — a **deny-list** of interior-mutable names accepts the alias while
//         rejecting the type written out. Too wide, in one token.
//   P52 — the same predicate emitted as a **bound**: `E0277`, because rustc
//         resolves the alias for the macro. This is the way out, and its cost is
//         that it would reject `Rc` too.
// ---------------------------------------------------------------------------
#[cfg(any(feature = "p52-bound-check-rejects-alias", feature = "p53-denylist-passes-alias"))]
pub type Audit = core::cell::RefCell<Vec<String>>;

#[cfg(feature = "p51-allowlist-too-narrow")]
pub struct Email(String);

#[cfg(feature = "p51-allowlist-too-narrow")]
#[mac::domain_name_checked]
pub struct AllowChecked {
    pub email: crate::derives::Email,
}

#[cfg(feature = "p53-denylist-passes-alias")]
#[mac::domain_deny_checked]
pub struct DenyChecked {
    pub audit: crate::derives::Audit,
}

/// P54 — P53's control. Written out, the deny-list does reject it, so P53 measures
/// the alias and not "the check does nothing".
#[cfg(feature = "p54-denylist-catches-direct")]
#[mac::domain_deny_checked]
pub struct DenyDirect {
    pub audit: core::cell::RefCell<Vec<String>>,
}

#[cfg(feature = "p52-bound-check-rejects-alias")]
#[mac::domain_bound_checked]
pub struct BoundChecked {
    pub audit: crate::derives::Audit,
}

/// P52's control: the bound accepts an ordinary value object, so the row is not
/// "everything is rejected".
#[cfg(feature = "p52-bound-check-rejects-alias")]
#[mac::domain_bound_checked]
pub struct BoundOk {
    pub email: String,
}
