---
status: proposed
date: 2026-08-16
decision-makers: itsakeyfut
enforcement-level: metadata_only
---

# Whether capability-checked getters enforce `reads`, or `reads` stays metadata only

> **`proposed`, and the documentation already depends on the answer.** Per
> [ADR-0000](./0000-record-architecture-decisions.md) that is a defect, not a
> normal state. It is recorded here rather than silently settled because the
> spike that decides it — #15 / T-M1-03 — has not run.

## Context and Problem Statement

Two statements in the specs are each defensible and cannot both be acted on:

* **The derive emits capability-requiring getters.**
  `docs/specs/mutation-contract.md` lists "2. Capability-checked getters" among
  what `#[derive(Domain)]` generates, and `docs/specs/read-contract.md` says
  Domain opacity plus those getters already restricts reading undeclared fields.
* **`reads` is metadata only.** `docs/specs/read-contract.md:141` and
  `docs/specs/ai-context.md:44` both emit `"enforcement": "metadata_only"`.

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

**Nothing enforces this today** — it is a claim about a claim.

What would confirm it is #15 itself: a spike that puts a `Has<Read<D, F>, I>`
bound on a generated getter and measures whether reading an undeclared field
fails to compile. Until that runs, `"enforcement": "metadata_only"` in the AI
Context is the only mechanically checkable part, and
`spikes/doc-code-blocks/run.sh` keeps the surrounding code blocks compiling.

### Consequences

* Good, because the AI Context does not overstate. If #15 comes back positive,
  the correction moves in the safe direction: a guarantee appears where none was
  promised.
* Good, because the open question stays visible instead of being resolved by
  whoever reads the specs next.
* Bad, because `metadata_only` may be understating what already works, and
  nothing in the docs will show that until #15 runs.
* Promoting the level later is a **breaking change** and must be handled as one.
* If #15 comes back positive, `Projection` may be unnecessary and Phase 9 shrinks
  or disappears — that consequence belongs to #18, not here.

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
