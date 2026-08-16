---
status: proposed
date: 2026-08-16
decision-makers: itsakeyfut
enforcement-level: upper_bound_checked
---

# What `Field<D>` declares, and what forging it would buy

> **`proposed`, and `Field` is already on the sealed-trait list.** A trait that
> must be sealed, whose signature nobody wrote, cannot have its seal checked for
> exactness — and this project has shipped a seal that did not match its trait
> three times.

## Context and Problem Statement

`Field<D>` is the marker trait the derive implements for each declared field. Two
documents show implementations:

```rust,ignore   // docs/specs/mutation-contract.md:179, and rust-type-model.md:251 in the same shape
impl Field<User> for Name {
    const NAME: &'static str = "name";
    type Ty = String;
}
```

**There is no declaration.** `grep 'trait Field' docs/` returns nothing. What
exists instead is a set of statements *about* it:

| Statement | Location |
|---|---|
| `Field` must be sealed — forging `Field::NAME` fakes the column name in generated SQL | `capability-system.md:115`, `api-surface.md:136`, ledger path 14 |
| Diagnostics lose the path qualification: `{Field}` renders as `Email`, not `user::Email` | `diagnostics.md:208`, `type-level.md:400` |
| The derive generates the marker types | `semantic-endpoint.md:215` |

So the trait carries a security property (path 14), a diagnostic property, and a
persistence property, and has no signature.

## Decision Drivers

* **The seal's solution set must equal the trait's**, or the difference is
  forgeable. `docs/rules/api-surface.md` §2 is the canon, and RK-015 records that
  this shipped wrong three times (#6, #8, #9). **The comparison cannot be made
  against a trait that has no declaration.**
* `const NAME` is what reaches generated SQL, which is why path 14 exists at all.
  Whether a `&'static str` is the right carrier — as opposed to something the
  compiler can check against the schema — is undecided.
* `type Ty` appears in the usages and is never mentioned in prose. What it is for
  (setter signatures? projection?) is not written down.
* The diagnostic limitation is already recorded and is a consequence of the shape:
  `{Field}` in `#[diagnostic::on_unimplemented]` renders the type's own name, so
  whatever `Field` is implemented *on* determines what the user reads.

## Considered Options

Not enumerable yet — the trait's obligations have not been written down, so there
are no options to weigh. That is the finding.

## Decision Outcome

**Not decided here.** This ADR records that:

1. `Field<D>` is used with `const NAME: &'static str` and `type Ty`, in two
   documents that agree.
2. It is on the sealed-trait list, with a named forgery consequence (path 14).
3. **Its declaration exists nowhere**, so the seal-exactness rule that
   `api-surface.md` §2 makes mandatory has nothing to compare against.

The declaration belongs in `docs/specs/mutation-contract.md` beside the marker
types, with the seal written next to it and marked `SEAL-EXACT` or `SEAL-DIFF`
per that rule.

### Confirmation

**Nothing enforces this today.**

* Ledger path 14 is marked *First PoCで塞ぐ* with the note `Field` 未実装 — the
  remedy is "seal it", and there is nothing to seal.
* `crates/verum/src/sealed.rs`'s parity guard scans declared seals. `Field` is not
  among them, so the guard is silent about it rather than failing — the same
  shape as the blind spot that guard was written to remove.

The confirmation is the pair `api-surface.md` §2 requires: a fixture at every
impl position, and a `SEAL-EXACT` / `SEAL-DIFF` marker on each seal impl.

### Consequences

* Good, because "seal `Field`" stops being an instruction with no referent.
* Bad, because path 14 stays open until the declaration exists, and the ledger
  currently reads as though the remedy is known.
* The diagnostic limitation (`{Field}` losing its path) is a property of whatever
  is chosen and should be re-checked against the declaration, not inherited.
* `type Ty`'s purpose has to be stated. Right now it is carried by two examples
  and explained by none.

## More Information

* `docs/specs/unverified-boundaries.md` path 14
* `docs/rules/api-surface.md` §2 — the seal-exactness rule this cannot yet satisfy
* `docs/rules/api-surface.md` §2 — the three occasions (#6, #8, #9) on which a seal did not match its trait, and the principle re-derived from them
* `docs/specs/mutation-contract.md` — where the declaration belongs
