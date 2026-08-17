---
status: proposed
date: 2026-08-16
decision-makers: itsakeyfut
enforcement-level: metadata_only
---

# Whether capability-checked getters enforce `reads`, or `reads` stays metadata only

> **Measured by #15 (T-M1-03), and still `proposed`.** The measurement is in
> `spikes/reads-getter-enforcement/`. The mechanism works, in exactly one shape,
> under two preconditions that are **not yet designed** — so the holding position
> below stands and the status does not move. `Projection` turned out **not** to be
> redundant. An earlier revision of this ADR moved to `accepted` on the strength
> of a spike whose verdict was subsequently withdrawn; that is recorded in the
> spike's README rather than edited away.
>
> The original note, kept because it records why the ADR existed:
>
> > **`proposed`, and the documentation already depends on the answer.** Per
> [ADR-0000](./0000-record-architecture-decisions.md) that is a defect, not a
> normal state. It is recorded here rather than silently settled because the
> spike that decides it — #15 / T-M1-03 — has not run.

## Context and Problem Statement

Two statements in the specs are each defensible and cannot both be acted on:

* **The derive emits capability-requiring getters.**
  `docs/specs/mutation-contract.md` lists "2. Capability-checked getters" among
  what `#[domain]` generates, and `docs/specs/read-contract.md` says
  Domain opacity plus those getters already restricts reading undeclared fields.
* **`reads` is metadata only.** `docs/specs/read-contract.md` and
  `docs/specs/ai-context.md` both emit `"level": "metadata_only"` for it
  (`"enforcement": "metadata_only"` before ADR-0008 made `enforcement` an
  object).

If the getters really require a capability, `reads` is enforced as an upper
bound and `metadata_only` **understates** what the type system does — the AI
Context is then lying in the safe direction, but lying. If they do not, the
getter description reads as a guarantee that is not there.

Nobody has measured which. That is #15 / T-M1-03: *can capability-checked getters
enforce `reads` without a `Projection` type?*

The cost of guessing is not symmetric. The versioning policy treats a change
of enforcement level as **breaking**: promoting `reads` from `metadata_only` to
`upper_bound_checked` makes previously-compiling code fail. Writing the
optimistic answer into the specs now would either bake in a promise the
implementation cannot keep, or force that breaking change later.

## Decision Drivers

* An enforcement level that overstates is the exact failure this project exists
  to prevent — metadata that lies.
* An enforcement level that understates is safe but hides work already done.
* #15's outcome also decides whether `Projection` — and with it most of Phase 9 —
  is needed at all.
* Changing the level after publication is a breaking change.

## Considered Options

* **`metadata_only` until #15 measures otherwise** — report the weaker claim, and
  say why it is the weaker one.
* **`upper_bound_checked` on the strength of the getters** — act on the reading
  that the getters enforce.
* **Leave both statements standing** — the state that produced this ADR.

## Decision Outcome

Chosen option: **`metadata_only` until #15 measures otherwise**, and say
explicitly that the getters' effect on `reads` is unmeasured.

This is a holding position, not an answer. The point is that the two statements
stop contradicting each other while the real question stays visibly open.

`docs/specs/mutation-contract.md` and `docs/specs/read-contract.md` now say that
the getters exist and that **whether they amount to enforcement of `reads` is
#15**, rather than implying it either way.

### Confirmation

**Measured in #15 / T-M1-03** (`spikes/reads-getter-enforcement/`, `bash run.sh`,
11 rows on rustc 1.85.0, two crates, against `verum`'s real `Has`). The fixture
that would fail if this decision were violated is `check_json.py` in
`spikes/doc-code-blocks/`, which asserts every guarantee-bearing key carries an
`enforcement` object — promoting `reads` without an implementation would not
change what that guard sees, which is precisely why the level is held here.

| Probe | Result |
|---|---|
| **E1** | an inherent `impl Repo<Domain, ..>` in the downstream crate is `E0116` — the getter can only live in a derive-emitted **extension trait** |
| E2 / E2b | a **declared** read compiles, at the head of the set and at depth |
| **E3** | an **undeclared** read is rejected, `E0277`, with a two-directional note |
| **F1** / **F2** | with the trait written `UserRead<R>` a downstream crate forges a wider set in one line; with `R` reached through `Self::Set` instead, the forge is `E0119` |
| **G2** | a downstream crate cannot re-point `ReadSet::Set` — `E0117` |
| **G1** | the caller can construct `Repo<Domain, AnySet, ()>` itself and read anything — the bound constrains `R`, not who supplies it |
| **D1** / D2 / D2b / **D2c** | `user.email()` is `E0283`; naming `R` alone compiles at every position and still rejects an undeclared read |
| **V1** / V2 | `impl From<&Domain> for View` cannot call a checked getter; through a plain getter it can |
| P1 / P2 | a `Domain`'s `Debug` and a free function read every field with no capability |
| P3 / **P3b** / **P4** | a `Projection`'s getter enforces too — **and its `Debug` prints declared fields only**, which the `Domain`'s cannot |

**So the getters enforce, in one shape, conditionally.** The mechanism is a trait
bound — defence layer 3 — and the recorded error shows `on_unimplemented` and
`do_not_recommend` doing what `diagnostics.md` designs them for. Two preconditions
are undesigned and both are load-bearing:

1. **`Repo` must be unreachable except through `Ctx`** (G1). A public constructor
   voids the guarantee without touching the bound.
2. **The extension trait must not take `R` as a type parameter** (F1/F2). Nothing
   in `docs/rules/api-surface.md` says so today.

**`Projection` is not redundant** (P4). It narrows a derived `Debug` to the
declared set; a `Domain`-side derive cannot, because the `Domain` has no read set.
That is one capability, not a verdict on the five costs
[`read-contract.md`](../specs/read-contract.md) lists — #18 weighs those.

**The level still does not move.** `metadata_only` stays, and the reason is now
different from the one this ADR was opened with: not "unmeasured" but
"unimplemented, and two preconditions undesigned". `crates/verum` has no derive,
no `Repo` and no getters, so emitting `upper_bound_checked` would claim
enforcement no code provides — the failure
[ADR-0008](./0008-guarantees-carry-scope-and-voiding-paths.md) exists to prevent.
It promotes when M2's derive lands *and* the two preconditions are closed; that
promotion is breaking.

**When it does promote, its scope is `handle_via_ctx`, not more.** P1/P2 measured
that a `Domain`'s `Debug` and free functions taking `&Domain` read every field
with no capability, and no getter shape reaches them. Recorded in the ledger
rather than left to make `reads` look broader than it is.

### Consequences

* Good, because the AI Context does not overstate. #15 came back "yes, but", and
  the correction moves in the safe direction: a guarantee appears where none was
  promised.
* Good, because the open question stays visible instead of being resolved by
  whoever reads the specs next.
* Bad, because `metadata_only` understates the mechanism, which #15 showed
  works. The gap is stated in the Confirmation rather than in the level.
* Promoting the level later is a **breaking change** and must be handled as one.
* `Projection` is **not** unnecessary — P4 shows it narrows `Debug` where getters
  cannot. Whether that one capability justifies Phase 9 belongs to #18, not here.

## Pros and Cons of the Options

### `metadata_only` until #15 measures otherwise

* Good, because it never claims more than has been measured.
* Bad, because it is provisional by construction, and provisional states have a
  habit of outliving their reason — a rejection recorded once tends to survive
  the circumstances that produced it, which is why this record names #15 as the
  thing that ends it.

### `upper_bound_checked` on the strength of the getters

* Good, because it would credit work that may already be done.
* **Bad, because it is unmeasured.** The same reasoning — "the signature implies
  it, so it must hold" — is what produced T-M1-02's wrong verdict.

### Leave both statements standing

* **Bad, because a reader acts on whichever one they read first.** This is the
  state #43 found and the reason this ADR exists.

## More Information

* #15 / T-M1-03 — the spike that settles this
* `docs/specs/read-contract.md` §Treatment in the PoC — where the level is emitted
* `docs/specs/ai-context.md` — the sample the level appears in
