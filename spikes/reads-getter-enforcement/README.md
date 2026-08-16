# T-M1-03 / #15 — can capability-checked getters enforce `reads` without `Projection`?

Measured on **rustc 1.85.0** (Verum's MSRV), against **`crates/verum`'s real
`Has`** by path dependency — not a stand-in. Two crates: `fw` is the framework,
`app` is the downstream crate that owns the `Domain`.

```bash
bash run.sh          # 11 rows, each with its expected outcome and error code
```

> **This document replaces an earlier version whose verdict was withdrawn.**
> That version measured a single-crate arrangement, and three of its conclusions
> did not survive review. What changed, and why, is in *What the first version
> got wrong* at the end. The withdrawal is recorded rather than edited away
> because the failure mode — prose generalising past its probe table — is the one
> this project keeps hitting.

---

## Verdict

> **Getters enforce `reads`, but only in one specific shape and only under a
> precondition the design must supply separately. `Projection` is *not*
> redundant: it narrows `Debug` in a way getters cannot.**

| Question | Answer |
|---|---|
| Where can a capability-checked getter live? | **Only in a derive-emitted extension trait.** An inherent `impl Repo<Domain, ..>` is `E0116` downstream (E1) |
| Does the bound reject an undeclared read? | **Yes** — E3, `E0277`, with a good message |
| Is that trait forgeable? | **Only if it takes `R` as a type parameter** (F1 compiles). Drop the parameter and take `R` from `Self::Set` and the forge is `E0119` (F2) |
| Can the caller pick its own read set? | **Yes, today** — G1 compiles. The bound constrains `R`, not who supplies it |
| Can the getter live on the `Domain`, so `user.email()` works? | **No** — D1, `E0283`. Naming `R` at the call site works (D2) but that is not `user.email()` |
| Does `Projection` buy anything getters do not? | **Yes** — P4. Its `Debug` prints declared fields only; the `Domain`'s cannot |

**What this costs:** every field read becomes `ctx.users().email(&user)`. That is
symmetric with the setters, and consistent with `handler-rules.md` Rule 2 — reads
*are* effects in the effect system. But `handler-rules.md`'s own canonical
example writes `user.email()`, and that cannot be kept.

**What this does not buy:** `reads` is enforced with the same scope as `mutates`
— `handle_via_ctx`. Free functions taking `&Domain` read every field with no
capability in sight (P2), and a `Domain`'s own `Debug` prints all of them (P1).
Those are not getters, and no getter design reaches them.

### For #18

The planning gate asked whether `Projection` can be dropped and Phase 9 shrunk.
**On the evidence here, no — not on the `Debug`/`Serialize` axis.** Getters and
`Projection` enforce equally at the *getter*; they differ at the *value*, and
only `Projection` can narrow what a derived `Debug` prints (P1 vs P4). Whether
that one capability is worth the five costs `read-contract.md` lists is a
judgement for #18, not a fact this spike can settle. What it can settle is that
"`Projection` is unnecessary" is false.

---

## Probe table

`fail` means "rejected, carrying the error **code** named here". Needles are
error codes, not prose — see the `NEEDLES` note in `run.sh` for the measured
reason. Rows that must **compile** live in the default build, and the baseline
controls are additionally pinned by name in a `const _` block (§9-13), so
deleting one is `E0425` rather than a green row that compiled nothing.

### (a) Where can the getter live?

| # | Probe | Expected |
|---|---|---|
| **E1** | inherent `impl Repo<Domain, ..>` from the downstream crate | fail `E0116` |
| E2 | reading a **declared** field through the extension trait | pass — baseline |
| E2b | the same, one element deeper — covers `There<_>`, not only `Here` | pass — baseline |
| **E3** | reading an **undeclared** field | fail `E0277` |

### (b) Is the extension trait forgeable?

| # | Probe | Expected |
|---|---|---|
| **F1** | `trait UserReadParam<R>` + a downstream impl handing a narrow repo a wide set | **pass — and that is the finding** |
| **F2** | the same forge against `trait UserRead: ReadSet` (no type parameter) | fail `E0119` |
| **G2** | a downstream crate re-pointing `ReadSet::Set` | fail `E0117` |

### (c) Who supplies `R`?

| # | Probe | Expected |
|---|---|---|
| **G1** | the caller constructs `Repo<Domain, WiderSet, ()>` itself | **pass — and that is the finding** |

### (d) The Domain-side shape

| # | Probe | Expected |
|---|---|---|
| **D1** | `user.email()` — nothing determines `R` | fail `E0283` |
| D2 | the same naming **only `R`**, index inferred | pass — baseline |
| D2b | the same for a field at position 1, still with `_` | pass — baseline |
| **D2c** | naming `R`, reading an **undeclared** field | fail `E0277` |
| D3 | the repository passed as a witness | pass — baseline |

### (e) The view conversion, uncapped reads, and `Projection`

| # | Probe | Expected |
|---|---|---|
| **V1** | `impl From<&Domain> for View` calling a **checked** getter | fail `E0283` |
| V2 | the same through a plain getter | pass — baseline |
| P1 | `Domain`'s `Debug` printing every field, including undeclared ones | pass — baseline |
| P2 | a free function taking `&Domain` | pass — baseline |
| P3 | a `Projection<D, F>` getter reading a declared field | pass — baseline |
| **P3b** | the same reading an undeclared field | fail `E0277` |
| **P4** | `Projection`'s `Debug` — **which fields print** | pass, output asserted |

---

## What the results mean

### The getter has exactly one available shape

E1 is the correction that forced this document to be rewritten. In the real
layering the framework owns `Repo` and the downstream crate owns `Domain`, so

```
error[E0116]: cannot define inherent `impl` for a type outside of the crate where the type is defined
70 | impl<R, M> Repo<Domain, R, M> {
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ impl for type defined outside of crate
   = note: define and implement a trait or new type instead
```

This is RK-004, and it means the surface the first version measured **could not
exist**. What is available is the trait rustc's own note recommends: an extension
trait, emitted into the user's crate by `#[derive(Domain)]`.

### That trait must not take `R` as a parameter

F1 and F2 differ in one thing. Written as `trait UserReadParam<R>`, a downstream
crate closes the gap between a narrow repo and a wide set in one line:

```rust
impl UserReadParam<DeclaredEmailAndName> for Repo<Domain, DeclaredEmailOnly, ()> { … }
```

It compiles. It does not collide with the blanket impl, because the blanket at
`R = DeclaredEmailAndName` yields `Repo<Domain, DeclaredEmailAndName, M>`, which
is never `Repo<Domain, DeclaredEmailOnly, ()>`. **Coherence is behaving
correctly and the guarantee is gone anyway** — the same shape as #6/#8/#9, where
a seal's solution set did not equal the trait's.

Dropping the parameter closes it. With `trait UserRead: ReadSet` and `R` reached
only through `Self::Set`, every impl target is `Repo<Domain, _, _>`, so a
concrete one overlaps the blanket:

```
error[E0119]: conflicting implementations of trait `UserRead` for type `Repo<Domain, (ReadEmail, ()), ()>`
```

G2 is the other half: a downstream crate cannot re-point `Self::Set` either,
because `ReadSet` and `Repo` are both foreign to it (`E0117`). **The seal here is
the orphan rule plus coherence, not a `Sealed` supertrait** — and that is worth
stating plainly, because a `Sealed` supertrait would *not* have worked. `verum`
would have to implement `Sealed` for `Repo<D, R, M>` generically, which admits
the forge just as readily.

This trait is generated into the user's crate and gates a capability, so it
belongs in `docs/rules/api-surface.md`'s list of things that must be
unforgeable. It is not there today. Filed as a separate issue — the design
question of where the trait lives collides with #34 and #39.

### The bound constrains `R`, not who supplies it

G1 compiles:

```rust
let r: Repo<Domain, DeclaredEmailAndName, ()> = Repo::new();
r.name(d)      // reads a field no endpoint declared
```

Nothing above is a bypass of the *bound*. The bound did its job — the set really
does contain `ReadName`. The problem is that the caller chose the set. **Every
enforcement claim about these getters is conditional on `Repo` being
unreachable except through `Ctx`**, and that precondition has to be stated
wherever the claim is.

`docs/dev/code/review-knowledge.md` RK-017 records the identical shape one PR
earlier, for `+ Send` on a returned future: a bound that constrains a type
without constraining who instantiates it. Two occurrences in two PRs is what the
Tier-2 trigger calls a rule rather than an entry.

### The error is good, and better than the first version's

E3's text, verbatim on 1.85.0 — this is requirement 4, and it seeds M4:

```
error[E0277]: `(ReadEmail, ())` does not contain `ReadName`
   --> app/src/lib.rs:147:5
    |
147 |     r.name(d).to_owned()
    |     ^ `ReadName` is not a member of this set
    |
    = help: the trait `Has<ReadName, There<_>>` is not implemented for `(ReadEmail, ())`
    = note: declare `ReadName` in the contract, or remove the call that requires it
note: required by a bound in `UserRead::name`
   --> app/src/lib.rs:92:20
    |
90  |     fn name<'a, I>(&self, d: &'a Domain) -> &'a str
    |        ---- required by a bound in this associated function
92  |         Self::Set: Has<ReadName, I>;
    |                    ^^^^^^^^^^^^^^^^ required by this bound in `UserRead::name`
```

`on_unimplemented` supplies the message, the label and the two-directional
note; `do_not_recommend` keeps the cons-list recursion out. The extension-trait
form names `UserRead::name`, which is closer to what the user wrote than the
inherent form's `Repo::<Domain, R, M>::name`.

Three things about it are **not** good, and M4 owns them:

* **The two directions are in a `note:`, not a `help:`.** RK-010 and
  `diagnostics.md:327` both say `help`. That requirement is **unachievable from
  layer 3**: `#[diagnostic::on_unimplemented]` rejects `help` on 1.85 —
  `only 'message', 'note' and 'label' are allowed as options`. Recorded in
  `diagnostics.md`; T-M4-05's completion criterion is corrected in the same
  change.
* **`There<_>` leaks the index machinery** into the `help` line. Twelve committed
  `.stderr` files under `crates/verum/tests/ui/compile_fail/` already carry it.
  T-M4-04's per-endpoint generated trait removes it; no task said so.
* **The span is a single `^` on the receiver.** `diagnostics.md:205-213` wants
  the method name.

### `Projection` narrows `Debug`; the getters cannot

P4, run and asserted rather than reasoned about:

```
1 field : Projection { email: "e@x" }
2 fields: Projection { email: "e@x", name: "nm" }
```

`secret` is populated in both values and prints in neither. The mechanism is a
derive **on the Domain** emitting one `DebugOneField<Elem>` impl per field — a
set it can see — plus one fixed recursive walk over the cons list. `F` is never
enumerated at the impl site; monomorphisation resolves it.

Compare P1, where the `Domain`'s own `Debug` prints `secret` for an endpoint that
declared no reads at all. **This is the one axis on which `Projection` is
strictly stronger**, and it is exactly the axis the ledger's `uncapped_read`
entry describes.

### `reads` is enforced with the same scope as `mutates`, not more

P1 and P2 compile. Neither is a getter, so no getter shape reaches them. This is
not a reason to reject the getters; it is the boundary of what "enforced" means,
and it is the same boundary `mutates` already has: **`handle_via_ctx`**. The
ledger gets an entry for it rather than leaving `reads` looking narrower or
broader than it is.

---

## What this changes

* **ADR-0004 stays `proposed`.** The mechanism works in one specific shape, but
  two of its preconditions — `Repo` unreachable outside `Ctx` (G1), and the
  extension trait's shape fixed (F1/F2) — are undesigned. Accepting it now would
  record a Confirmation the code does not provide.
* **`reads` stays `metadata_only` in the AI Context.** The mechanism is verified;
  the implementation does not exist — `crates/verum` has no derive, no `Repo`, no
  getters. Emitting `upper_bound_checked` would claim enforcement no code
  provides, which is what ADR-0008 was written against. It promotes when M2's
  derive lands, and that promotion is a breaking change by the versioning rules.
* **Phase 9 does not shrink on this evidence.** P4 refutes the reason that was
  given for shrinking it. #18 decides on the cost/benefit.
* **`read-contract.md`'s "declared fields only" claim is restored**, with P4 as
  its evidence.
* **`diagnostics.md` records that `help` is unreachable from layer 3.**

---

## The harness

`run.sh` implements `docs/rules/test.md` §9. Proven by planting mutations, not by
reading — **all six planted and observed on 2026-08-16**:

| Mutation | Caught by |
|---|---|
| **a one-character typo in a `--features` argument** | E3 `UNEXPECTED — MISSING("E0277")`, exit 1 |
| **delete a baseline control** (`d2b`, §9-13) | baseline `FATAL` via `E0425` at the `const _` anchor |
| delete one `probe` line | `FATAL: 10 rows ran, expected 11` |
| the `TOOLCHAIN` name is not installed | `FATAL: toolchain 1.86.0 is not installed` |
| **`Projection`'s `Debug` leaks an undeclared field** | P4 `UNEXPECTED — output mismatch`, exit 1 |
| `UserRead` weakened back to a parameterised form | baseline `FATAL` |

The first two are the ones this spike got wrong on its first pass and are worth
naming:

* The original needle was `does not contain`, which is a **substring of cargo's
  own** `error: the package 'app' does not contain this feature: <typo>`. A typo
  therefore gave `rc != 0` *and* a needle match — the row printed `as specified`
  having compiled nothing, and the suite exited 0. Error codes cannot collide
  that way, which is what the three sibling spikes already did.
* The original had no §9-13 anchors, so *deleting* a control left its row green.
  `README.md` claimed "breaking one is `FATAL` rather than a silently-green row",
  which was true for breaking and false for deleting — #48's lesson, verbatim,
  one PR later.

The last mutation is honest about what happened: weakening `UserRead` was
*detected*, but as a baseline `FATAL` rather than as F2 flipping to `pass`. The
crude form of the mutation broke inference elsewhere first. F2 is still the
probe that owns that property; this mutation does not prove it in isolation.

### Rule 14 — every rejection has a standing control

```text
E1  → E2/E2b        E3  → E2b        F2 → F1        G2 → E2
D1  → D2/D2b        D2c → D2/D2b     V1 → V2        P3b → P3
```

Every control except F1 is in the default build **and** pinned by name in the
`const _` block. F1 is feature-gated because it is itself a rejection-shaped
probe expected to pass; deleting it is caught by `EXPECTED_ROWS`.

### Remaining limits

* **The spike cannot be copied elsewhere without editing `Cargo.toml`.** The path
  dependency on `../../crates/verum` does not survive a move. `run.sh` asserts
  `cargo metadata` resolves before any probe runs, so the failure names itself.
* **Nothing re-runs this when `verum` changes.** The wording `does not contain`
  is also pinned by `crates/verum/tests/ui/compile_fail/has_missing_element.stderr`,
  which *is* in CI, so it cannot change silently — but no back-link records that
  this verdict depends on it.
* **`Read<D, F>` and `Field` are marker types here**, not the real ones —
  ADR-0007 is `proposed` and declaring them is #34's. `Has` is generic over its
  element, so the measurement does not depend on the choice; the *error text*
  does, and the real elements will print differently.
* **`Serialize` is not measured.** The earlier version named it alongside `Debug`;
  there is no serde in this spike. `Debug` is the measured case, and the
  `DebugOneField` mechanism is not `Debug`-specific — but that is desk analysis,
  not a result.
* **`Debug` is hand-written**, because `verum-macros` emits nothing yet. The
  derive's output would be this; the point is what it can see.

---

## What the first version got wrong

Kept deliberately. Three of four headline conclusions were refuted, all by the
same mechanism: a probe table that was correct, and prose around it that
generalised past what the probes measured.

| Claim | Status | What refuted it |
|---|---|---|
| "Shape A — the getter on the repository — is forced and works" | **withdrawn** | E1. It is `E0116` once `Repo` and `Domain` are in different crates, which is the real layering |
| "`read-contract.md`'s 'declared fields only' claim about a projection's derive is wrong" | **withdrawn; the original claim restored** | P4. A derive on the *Domain* emits one impl per field and a fixed walk resolves `F` at monomorphisation |
| "Adding one field to `reads` renumbers every call site" | **withdrawn** | D2/D2b. `_` suffices for the index; only `R` has to be named, and it works at every position |
| "The bound rejects an undeclared read" | **stands, with a precondition now stated** | G1. True of `R`; silent about who supplies `R` |

The single-crate arrangement is what made the first three possible: it let an
inherent impl compile, and it left the layering question — where does the derive
emit, and what can a downstream crate write — entirely unasked.
