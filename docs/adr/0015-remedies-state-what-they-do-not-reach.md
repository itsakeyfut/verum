---
status: "accepted"
date: 2026-08-20
decision-makers: itsakeyfut
enforcement-level: "none"
---

# Every ledger cause states what its remedy does not reach, and a macro's authority is bounded by attribute position

## Context and Problem Statement

`docs/specs/unverified-boundaries.md` is what the fallback criterion rests on:
*bypasses are acceptable if the metadata records them.* An adversarial audit (#44)
found four paths it did not record, and all four sat in the blind spot of a
**remedy** — not of a boundary. That is a property of how the taxonomy is cut, so
fixing the four rows without changing the cut leaves the next four unfindable.

Re-measuring #44's premise for this change turned up something narrower and worse.
`read-contract.md` records "forbid `Deserialize` on a domain", #44 established that
an attribute macro can see its sibling derives where a derive cannot, and concluded
that the attribute form **can enforce** it. Measured: it can, for the derives
written *below* it. A derive written **above** `#[domain]` is expanded independently
— dropping it from the re-emitted item does not suppress it — so the check never
runs, and what rejects that position is the **placement** ADR-0010 chose for a
different path.

So there are two decisions to take, and they are the same shape: a stated remedy
was doing less than the text around it claimed.

## Decision Drivers

* **Measured.** `#[domain]` sees a derive below it and not above it — probe P46,
  `spikes/domain-opacity-sqlx`. rustc expands an item's outer attributes in source
  order and the first active attribute macro consumes the rest, so a derive written
  above is expanded **first** and never enters the attribute's token stream; it then
  applies to whatever the attribute emitted. An earlier version of this record said
  the attribute is expanded first and "dropping it from the re-emitted item does not
  suppress it" — the attribute never receives it, so there is nothing to drop, and
  only the accurate mechanism predicts P42's error.
* **Measured, and it is what closes the position the check cannot see.** An
  attribute does not need to *observe* a derive; it needs to **occupy the coherence
  slot**. Emitting `impl Default` and `impl Clone` makes the user's derive `E0119`,
  spanned on their own derive, with both violations reported at once — **in every
  position and under every spelling**, because coherence does not read names.
  Probes **P42** / **P47**. This is the same mechanism the code's own comment
  already cited for `Debug`, which is why writing "no fix" was a miss rather than a
  judgement.
* **Measured.** With the field names preserved and the struct left in the user's
  module, the above-position derive forges a Domain that a foreign crate reads at
  run time (P43). With the struct in a macro-owned child module, the same source is
  rejected (P44, `E0616`; `E0451` with `Default` alone). **The variable is
  placement — for a derive whose generated code names a field.**
* **Measured, and it bounds the line above.** `#[derive(Clone, Copy)]` above the
  attribute **compiles** under the confined shape: `Clone`'s derive emits `*self`
  when `Copy` is present, so it names no field and meets neither the newtype
  mismatch nor the field privacy. Placement does not reach it. Probe **P49**, and
  the review that found it deleted the "placement is the mechanism" sentence's
  unqualified form.
* **Measured, and uncomfortable.** Emitting `Clone` to close the two derives
  **opened a third**: `#[derive(Copy)]` requires `Self: Clone`, which used to be
  unsatisfied (`E0277`), so the barrier that had been stopping `Copy` was
  incidental and this decision removed it. What still stops it by default is
  structural — the `Repr` carries no derive, so `Copy` on the newtype is `E0204`
  (**P49**) — and the one route past that runs through `repr_derive(..)`, which is
  the attribute's **own argument list** and therefore checkable in every position
  (**P48**).
* **Measured.** `impl<D, R, M> AnyService for verum::Repo<'_, D, R, M>` compiles from
  a crate whose only dependency is verum, and `&dyn` erases all three parameters
  (G1–G3, `spikes/ctx-lifetime-rpitit`). Path 11's remedy — "do not expose
  `dyn Repository`" — cannot be enforced, because the trait is the *user's*.
* **Recorded, from this repository's own history.** A blank in
  [api-surface.md](../rules/api-surface.md) §2's seal table pointed straight at
  where the hole was (#9). The same discipline had never been applied to the
  ledger's causes.
* RK-016: a guard must not depend on where code is placed or what it is called.
  Both halves fired here — the attribute's **position in a list**, and the derive's
  **spelling**. The instance number is deliberately not given: the bank's own
  numbered history stops at ⑦ (#34), #39's is recorded only in a `run.sh` comment
  and #41's only in the batting log, so a number written here would be a fourth
  place for it to disagree from — which is the defect this record is about. The
  count belongs in `docs/dev/code/review-knowledge.md` and nowhere else.

## Considered Options

* **A.** Fix the four rows and leave the taxonomy as it is.
* **B.** Add a fourth structural cause — *a macro's authority is bounded by its
  expansion form and position* — as #44 proposes.
* **C.** Keep three causes and give every one a mandatory "what this remedy does
  not reach" column; record the macro-authority observation where it is actionable
  rather than as a cause.

For the derive check, separately:

* **D.** Treat the layer-1 rejection of `Deserialize` / `Default` / `Clone` as the
  guarantee, as `read-contract.md` implies.
* **E.** Treat it as a **lint**, and name the child-module placement as the
  mechanism.
* **F.** **Emit the conflicting impl** for every forbidden trait verum can name,
  and keep the lint for the rest.

## Decision Outcome

Chosen: **C** and **F** (which contains **E** for what it cannot reach).

**C, and the reason is narrower than the first version of this record gave.** That
version said adopting B "would still leave two paths outside every cause". **That
is false, and the decision does not need it**: the ledger files path 24 under Cause
2 and path 27 under Cause 3, and a fourth cause is an *addition* — an addition
cannot remove existing coverage. What #44 actually observed is that the proposed
cause does not *explain* 24 or 27, which is a statement about cause 4 and not about
the partition. The surviving reason for C is the first clause alone: **B re-cuts the
taxonomy by mechanism**, and the causes are cut by boundary so that a remedy's edge
has somewhere to live. The column achieves that directly, and the ledger now forbids
leaving a cell blank or writing `—` in it.

> **B is also not #44's proposal.** #44 raises the fourth cause and then rejects it
> in the next sentence — *"But it does not cover 23-A or 23-C. **The minimum fix is
> a 'what this remedy does not reach' column**"* — which is C. An earlier version of
> this record attributed B to #44 and rejected it as such, i.e. rejected #44's
> rejected option as if it were its proposal.

**F, because D is false and E is true but leaves the reachable positions to
accident.** The check is worth having where it fires — it turns a confusing
downstream error into a message naming the route — but it reaches **one position out
of two** and **one spelling out of many**: a derive above the attribute is invisible
to it, and `r#Default` or `use core::clone::Clone as Dup;` are invisible in *any*
position, because a proc macro sees tokens and resolves nothing. That is the same
argument this project uses to reject path 5's name-based field whitelist, and it
applies here too.

So the guarantee is the **conflicting impl**, and the lint is what remains for the
traits it cannot cover:

| Derive | What closes it | Position-independent? | Spelling-independent? |
|---|---|---|---|
| `Default`, `Clone` | verum emits its own impl → `E0119` (P42, P47) | **yes** | **yes** |
| `Copy` | structurally `E0204` unless `repr_derive(Copy)`; that argument list is the attribute's own, so the check there is unconditional (P48, P49) | **yes** | **yes** |
| `Deserialize` | the lint only — verum has no serde dependency, so there is no impl to collide with (P50) | no | no |

**The cost of F is stated rather than hidden.** The emitted bodies are
`unimplemented!()`, so a legitimate `Default::default()` **compiles and panics at
run time** instead of failing to compile. That is worse than a compile error and
better than a silent forgery, and it is why `Deserialize`'s row above says "the lint
only" rather than pretending the coverage is uniform. `Copy` is the sharpest
consequence: emitting `Clone` removed the incidental `E0277` that had been stopping
it, so this decision *created* the need for P48.

**E survives as the framing for `Deserialize`** — the same demotion
[ADR-0011](./0011-domain-is-an-attribute-macro.md) gave the `pub`-field check, though
**not for the same reason**: ADR-0011's check is a lint because the attribute
*consumes* the `pub`, so the guarantee comes from the emitted shape and holds whether
or not the check runs. Here the check is a lint because it is blind in half the
positions. Both are lints; only one of them is redundant.

**A consequence worth stating plainly**: path 26 is closed today by a placement
chosen for path 21. If that confinement radius is ever relaxed, two paths reopen and
only one of them says so — which is why both the ledger row and this record name the
dependency rather than leaving it implicit.

### Confirmation

**The column**: nothing mechanical. It is a review rule stated in the ledger, in the
same words `api-surface.md` §2 uses for the seal table, and the fixture that would
fail is a reviewer noticing a blank cell. Written here rather than left to be
inferred.

**The conflicting impl, and the lint's boundary**: `spikes/domain-opacity-sqlx/run.sh`.

* **P42** — `#[derive(Default)]` **above** `#[domain]`: `E0119`, and the row needles
  on the **absence** of verum's layer-1 wording, so it reddens if anyone concludes
  the name-based check reaches there. Removing the emitted `impl Default` turns it
  red (planted).
* **P47** — the same position with `r#Default` and an aliased import: `E0119` twice.
  This is the row that stops the check being described as the defence.
* **P46** — `#[derive(Default)]` **below** it: verum's own wording. The one position
  the lint reaches.
* **P50** — `#[derive(serde::Deserialize)]` below it. Added because narrowing
  `FORBIDDEN_DERIVES` to `["Default"]` left the entire suite green: `Clone` and
  `Deserialize` were in the list with nothing exercising them.
* **P48 / P49** — `Copy`. P49 is the structural barrier (`E0204`, any position);
  P48 is the only route past it, rejected in the attribute's own argument list.
  Giving P49 a `repr_derive(Copy)` turns it red; dropping P48's check turns it red.
* **P43 / P44** — the pair that isolates placement. Swapping P44's confined
  attribute for the same-module form turns it red, so the row measures placement
  rather than shape. **P44's needle is `E0616`** (`Clone` reads the field); with
  `Default` alone the same source is `E0451`.
* **P45** — path 28's `&self` mutation, run-verified from a foreign crate. ⚠️ It
  does **not** measure the alias half of path 28 — replacing the alias with the type
  written out keeps the row green, and there is no whitelist in the spike to defeat.
  That half is desk analysis and the ledger now says so.

**Path 27's unenforceability**: `spikes/ctx-lifetime-rpitit/run.sh` G1–G4, against
the shipped `verum::Repo` through a crate that path-depends on `crates/verum`.
**G4** is the control and must stay red: the erasure loses the capability parameters
and does **not** launder `'req`, so path 27 does not dominate path 24.

**The enumeration**: `spikes/doc-code-blocks/check_json.py` compares the ledger's
sample, `ai-context.md`'s, the ledger's counting rule, **and the progress-metric
block** — including each name's side against the JSON `permanent` flag, the flag's
agreement *across* the two files, duplicate entries, and that each file has exactly
**one** populated `unverified_boundaries.entries` sample. Every rule was planted and
confirmed red first, including three the first version missed: a flag flipped in one
file only, a kind moved from the real sample into an illustrative fence in the same
file (the fail-open shape of the very drift this rule exists to catch), and the
progress-metric block, which is a third copy of the two numbers eleven lines above a
sentence asserting there was no third place for them to disagree from.

It does **not** check that every `kind` is named by some `voided_by`; **six** are
not, and the ledger says so rather than exempting them.

### Consequences

* Good, because a remedy's edge is now part of the remedy. Every one of #44's four
  paths would have had a place to be written before it was found.
* Good, because "the attribute can enforce it" is replaced by a measurement of
  *where* it can, which is what an implementer of `#[domain]` needs.
* Good, because paths 5 and 11 stop claiming to be closed. Both said "Closed in the
  First PoC" while their remedies were unenforceable.
* Bad, because the column is prose with no guard, and this repository's record on
  prose rules it cannot check is poor.
* Bad, because the coverage is not uniform and the record has to keep saying so.
  `Deserialize` is lint-only, and a legitimate `Default::default()` now compiles and
  panics rather than failing to compile.
* Bad, because closing `Default` and `Clone` **created** the `Copy` route by
  supplying the `Clone` bound that `#[derive(Copy)]` needs. P48 exists only because
  of this decision.
* What would reverse this: verum depending on serde (which
  [ADR-0011](./0011-domain-is-an-attribute-macro.md)'s `repr_derive` exists to
  avoid), or a stable way to write a negative impl, either of which would make the
  coverage uniform and retire the lint.

> **An earlier version of this record said the ordering hazard had "no fix" and that
> "nothing in the current language offers one".** Both were false, and the mechanism
> was already named in this repository — the spike's own comment cites `E0119` as
> what defends `Debug`. Writing an *absolute* impossibility is what SRK-009 exists
> to catch, and this is its fourth instance; the enumeration behind it was one item
> long ("observe or suppress the derive") and did not include "occupy the slot".
> Tried and rejected on the way: a conflicting impl with an unsatisfiable `where`
> clause, to keep the legitimate call a compile error — the impl itself is rejected
> (`trivial_bounds`, rust#48214), which is why the bodies are `unimplemented!()`.

## Pros and Cons of the Options

### C — three causes plus a mandatory column

* Good, because it fixes the mechanism that hid four paths without renumbering or
  re-filing anything.
* Good, because the ledger's cross-references (paths 12/13/14 are named from two
  rules and three knowledge banks) are untouched.
* Bad, because a never-blank rule is only as good as the reviewer applying it.

### B — a fourth structural cause

* Good, because the observation it names is real, and sharper than #44 stated: the
  bound is attribute *position*, not only expansion site.
* Bad, because it does not cover 24 or 27, so the causes would still not partition
  the paths — with the added cost that a reader would believe they did.

### E — the derive check is a lint

* Good, because it is what the measurement supports for the traits F cannot reach.
* Bad, because "a lint" reads as optional, and an AI facing the lint may reorder
  the attributes rather than remove the derive — the contract-relaxation bias this
  file records, in a new shape. **F removes that lever for `Default` and `Clone`**:
  reordering does not help when coherence is what rejects it.

### F — emit the conflicting impl

* Good, because it is position- and spelling-independent, and it reports both
  violations at once with the span on the user's own derive.
* Good, because it reduces the lint's job to one trait, which makes the remaining
  gap small enough to state precisely.
* Bad, because the legitimate call becomes a run-time panic rather than a compile
  error, and because it supplies the `Clone` bound `#[derive(Copy)]` wanted.

## More Information

* [`unverified-boundaries.md`](../specs/unverified-boundaries.md) — the causes
  table's fourth column, paths 26 / 27 / 28, and the corrections to 1 / 2 / 5 / 11 / 20
* [ADR-0010](./0010-domain-constructor-confined-by-module-privacy.md) — the
  placement that closes path 26 as a side effect
* [ADR-0011](./0011-domain-is-an-attribute-macro.md) — the same demotion, applied
  to the `pub`-field check
* [`read-contract.md`](../specs/read-contract.md) — "forbid `Deserialize` on a
  domain", now carrying its measured limit
* [`../rules/proc-macro.md`](../rules/proc-macro.md) — the layer-1 table
* `spikes/domain-opacity-sqlx/` (P42–P46), `spikes/ctx-lifetime-rpitit/` (G1–G4)
* Issue #44; RK-016 (a guard must not depend on placement), ARK-002 (blocking needs
  a checked alternative), SRK-009 (an impossibility claim needs a closed enumeration)
