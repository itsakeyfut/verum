---
status: accepted
date: 2026-08-17
decision-makers: itsakeyfut
consulted: "#13 (T-M1-01), #33, #34"
informed: "docs/specs/persistence.md, docs/specs/mutation-contract.md, docs/rules/proc-macro.md, docs/roadmap/M2-contract-dsl.md"
# No enforcement-level: this decides a macro form, not an AI Context key.
# Opacity is a privacy property, which is not an upper bound (ai-context.md
# defines that as implementation ⊆ contract), and ADR-0008 requires any
# guarantee claim to name its key plus scope and voided_by. The `pub`-field
# check it does bear on is a lint — level `none`.
---

# The Domain macro is an attribute, `#[domain]`, not `#[derive(Domain)]`

## Context and Problem Statement

Every spec since the design phase has written `#[derive(Domain)]`, and
[ADR-0010](./0010-domain-constructor-confined-by-module-privacy.md) — `accepted`,
and the mechanism that closes ledger path 21 — specifies the shape that derive is
supposed to emit:

```rust,ignore
mod __verum_account {
    pub struct Account(AccountRepr);
    struct AccountRepr { .. }
    impl Account { fn from_repr(r: AccountRepr) -> Self { .. } }
    pub struct AccountRepository;
}
pub use __verum_account::{Account, AccountRepository};
```

**A derive cannot produce it *as written*** — the re-export collides with the
user's own item (`E0255`, probe P38). ADR-0010's own text says "derive-owned" and
"what the derive emits" throughout, and that was never compiled.

> **⚠️ An earlier revision of this ADR stopped there and concluded a derive cannot
> own the confinement at all. That is false, and review refuted it by
> construction** (probe **P40**): emit only the `impl` block into the generated
> module —
>
> ```rust,ignore
> mod __verum_account {
>     pub(super) struct AccountRepr { .. }
>     impl super::Account { fn from_repr(r: AccountRepr) -> Self { .. } }
> }
> ```
>
> A private inherent method's visibility is **the module the `impl` is written in**,
> not where the type is defined. No re-export is needed, so nothing collides, and
> `Account::from_repr(..)` from the user's crate is still `E0624` (**P40a**).
>
> **The real reason the attribute form wins is a cost, not an impossibility**, and
> it was already in this record under option A: **a derive cannot consume the
> user's item.** The transparent original survives beside the opaque one, so
> `account.email = v` compiles next to the declaration (**P40b**) and the field
> list the markers and the `pub` check are generated from is the *user's* struct,
> not one the macro controls. `E0255` is therefore not the deciding probe — P40b
> and the T-M2-04 input cost are.

#34 also asked two questions this ADR has to answer at the same time, because the
form determines both: what the `pub`-field check guarantees, and who may name
`sqlx`.

## Decision Drivers

* ADR-0010's confinement must survive — it is the only measured mechanism that
  rejects every forgery route without depending on the user's module layout.
* Verum must not require a database crate (`docs/rules/design.md`,
  `persistence.md` — "Verum は DB を知らない").
* An enforcement level must be stated for what it *is*, not for what reads best
  (ADR-0008).
* Cost is measured, not estimated: `verum-macros` is 10 lines and emits nothing,
  and no user exists.

## Considered Options

* **A. `#[derive(Domain)]`, user writes the newtype** — the user declares
  `pub struct User(UserRepr);` and the derive emits only the `Repr`.
* **B. `#[derive(Domain)]`, `Repr` as a type alias** — `pub(crate) type UserRepr = User;`.
* **C. `#[domain]` attribute** — consumes the user's item and emits ADR-0010's shape.

## Decision Outcome

Chosen option: **C**, because it is the only form that **consumes the user's
item**, and everything else follows from that: the confinement radius is the
macro's rather than the user's, the emitted type has no transparent twin, and the
field list is one the macro controls. It also loses none of the layer-1 checks.

### Confirmation

`spikes/domain-opacity-sqlx/` (`bash run.sh`, 44 rows). The rows that decide it:

| Probe | What it does | Result |
|---|---|---|
| **P38** | ADR-0010's shape **verbatim** from a derive | **`E0255`** — the *re-export* collides with the user's own item |
| **P40** | a derive emitting only the `impl` block into the module | **compiles** — a derive *can* own the confinement radius |
| **P40a** | the forgery, under P40's shape | **`E0624`** — and the wall holds there too |
| **P40b** | `account.email = v` beside the declaration, under P40 | **compiles — this is the deciding row.** A derive cannot consume the item, so the transparent original survives |
| **P39** | ADR-0010's shape from an attribute | compiles. The row references the expansion, because an earlier version passed on an *empty* one |
| **P39b** | the forgery ADR-0010 exists to reject | **`E0624`** |
| **P39d** | naming the `Repr` from outside the module | **`E0422`** — "module-private: paths 3/4 shut with it", which nothing had measured |
| **P39c** | the generated repository's legitimate route | compiles (ARK-002's checked alternative) |
| P16 | (pre-existing) a derive emitting a **sibling** named after its input | `E0428` |

P16 was already on record and is *not* sufficient: ADR-0010 puts the struct inside
a module, where the name does not collide. P38 is the probe that had to be added,
and its error code is different for that reason.

**A and B were not rejected for being unbuildable — they are buildable.** They are
rejected for what they cost:

| Option | Cost, measured in #34 |
|---|---|
| A | the derive then sees **one unnamed field**, so field-marker generation (T-M2-04) and `pub`-field rejection (T-M2-05) lose their input entirely |
| B | `from_repr` becomes the identity; `query_as!` is lost (`E0451`) |

### What this decides about the `pub`-field check

**It becomes a lint, and the specs must say so.** Under `#[domain]` the macro
consumes the user's `pub` and emits a private inner field, so `u.email = v` fails
**whether or not the check runs**. The check is kept — a user who writes `pub`
holds a false belief about their own type, and silently rewriting their code is
worse than telling them — but it is not what provides opacity.

This retires the form of RK-008 that says "one `pub` field voids the entire
contract". Under option A that was true. Under C it is not, and
`mutation-contract.md` is corrected with it.

> **The guarantee moved, it did not weaken.** Opacity is now structural: it comes
> from the emitted shape, which the user cannot alter, rather than from a check
> that had to run. That is a strictly stronger position, and it is the reason the
> level below is `upper_bound_checked` rather than `intent_only`.

### What this decides about `sqlx::FromRow`

**Pass-through**: `#[domain(repr_derive(sqlx::FromRow))]` forwards the listed
derives to the generated `Repr`. Only the macro can attach a derive to a generated
item, so the alternatives were "Verum hard-codes it" or "no `FromRow`, and the
`query_as!` function form is unavailable".

**It does not violate Dependency Hiding**, and the argument is written here so it
is not re-litigated: the emitted `#[derive(sqlx::FromRow)]` resolves in the
**user's** crate. `verum-macros` gains no sqlx dependency and never names sqlx.
What would violate the rule is Verum writing `sqlx::` into a `quote!` itself —
that makes every user require the crate, invisibly, since it appears in no
manifest.

`.github/scripts/check-api-boundary.sh` did not catch that. Its `imports` mode
checked the **axum family only**; `sqlx` was in `FORBIDDEN_ROOTS`, which only
`public-api` mode consults — and generated tokens never appear in `verum`'s
rendered public API. #34 described this as "the guard scans source, not generated
output"; measured, the cause was the narrower vocabulary. The mode now checks the
whole forbidden list, with the `runtime/` exemption still axum-only, and the
hazard was planted and confirmed red.

### Consequences

* Good, because ADR-0010's confinement becomes implementable rather than
  aspirational, and every layer-1 check survives.
* Good, because the opacity guarantee stops depending on a check running.
* Bad, because `#[derive(Domain)]` appears widely. Measured at the time of this
  change: **15 occurrences across 11 files** in tracked `docs/` + `crates/`, or 22
  across 18 including the gitignored `docs/roadmap/`, `docs/dev/` and
  `.claude/`. **Two are inside already-published issue bodies**
  (`create_m0_issues.sh`, `create_m1_issues.sh`) that a document edit cannot reach
  — those get a comment rather than a rewrite. (An earlier revision said "15 files
  / 23 occurrences", which reproduces at no scope; the labels were swapped.)
* **"Breaking" is vacuous here and the reasoning should not be reused.**
  `verum-macros` is 10 lines and emits nothing, nothing is published, no user
  exists. A later form change would not be cheap; this one is.
* T-M2-04 / T-M2-05 / T-M2-06 are rewritten around the attribute form.

## More Information

* [ADR-0010](./0010-domain-constructor-confined-by-module-privacy.md) — the shape
  this form exists to emit. Its "derive-owned" wording is corrected by this ADR.
* `spikes/domain-opacity-sqlx/` — P38 / P39 / P39b, and P16 / P17 before them.
* #34 (the decision), #13 (which surfaced `E0428`), #33 (which chose the shape).
* RK-008 — corrected, not retired: the lesson survives for option A and for any
  future derive-shaped domain macro.
