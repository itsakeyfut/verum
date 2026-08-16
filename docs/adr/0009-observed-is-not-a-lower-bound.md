---
status: proposed
date: 2026-08-16
decision-makers: itsakeyfut
enforcement-level: none
---

# `observed` is not a lower bound, and Q-A is reopened on the measurement

## Context and Problem Statement

Q-A was decided on 2026-08-15 (`docs/specs/effect-inference.md` §Decision (Q-A)):
keep type enforcement for the upper bound, **generate `observed` from `handle`'s
tokens** for the lower bound, and let the difference between them detect
over-declaration. `ADR-0008` and `docs/specs/research-questions.md` both depend on
that decision.

It rested on one assertion that had never been compiled — that effects are
syntactically confined inside `handle`, so token scanning recovers the contract.
`docs/specs/rust-type-model.md` stated it as fact and called it "what makes the
approach feasible".

T-M1-07 (#37) compiled it. This ADR exists because CLAUDE.md requires that a
change altering a decision add or supersede an ADR in the same change, and
because the reopening is otherwise recorded only inside the file whose decision
it reopens.

## Decision Drivers

* The strict goal is **no bypasses, no lies**. `observed` was the mechanism for
  the second half.
* `docs/specs/evaluation.md`'s Q-C experiment measured that an AI relaxes the
  contract rather than fixing the implementation (RK-010). Any signal that tells
  it to widen a contract is dangerous in a way that a signal telling it to narrow
  one is not.
* Enforcement levels are not uniform: `mutates` is `upper_bound_checked`,
  `reads` is `metadata_only` with `scope: none`.

## Considered Options

* Keep Q-A as decided and treat the misses as a known limitation
* **Reopen Q-A, with the measurement attached, and let #18 choose the shape**
* Abandon generation and keep type enforcement only

## Decision Outcome

**Chosen: reopen.** Not soften — the premise as written is false, and three
documents asserted it.

### What was measured

Reproduction: `spikes/contract-from-tokens/` (`bash run.sh`, 15 rows, rustc
1.85.0).

**Three of the five distinct contract keys recover exactly** (`mutates`,
`creates`, `emits`), five of seven counting `when`-scoped instances separately.
The conditional split survives, including nesting. `calls` and field-level
`reads` do not recover.

**`observed` is not a lower bound, for two independent reasons.**

1. **It is incomplete.** Six ordinary constructs defeat it, in two groups. The
   scan cannot leave the item it is attached to (a free associated function, a
   sibling `impl`, and a **`macro_rules!` expansion** — the last unreachable even
   with cross-item analysis). And within that item it matches by **spelling** —
   naming the handler's parameter anything but `ctx` voids every key at once.
2. **It is unsound.** A proc macro runs before cfg-stripping, so a `#[cfg]`-gated
   statement naming a type that does not exist is reported as an effect.

### Consequences

* Good, because the failure is now measured rather than assumed, and #18 plans
  against numbers.
* Good, because probe P5 found one improvement: `User::from_repr(..)` is
  **visible**. Generation does not close ledger path 21 — `from_repr` is not an
  effect — but it can stop the bypass being silent.
* **Bad, and this is the sharp edge:** an incomplete scan reports false
  *over*-declaration, an unsound one reports false *under*-declaration. The
  second's repair is to **widen the contract**, which is the Q-C bias. The type
  system refuses narrowing only for keys at `upper_bound_checked`, so a false
  report on **`reads` has no compiler backstop at all**.
* `scope: "handle_only"` and `kind: "service_body"` now overstate what they name.
  Replacements are tracked on ledger path 22, not decided here.
* The CI gate argued for in §When over-declaration appears was justified on the
  assumption of no false positives. That assumption is gone; whether the gate is
  still proportionate is #18's.

### Confirmation

`bash spikes/contract-from-tokens/run.sh` — 15 rows, every positive probe
asserting its emitted JSON in full. The fixture that would fail if this decision
were violated is **V2** (a never-compiled statement appearing in the output) and
**V1** (a renamed parameter emptying it): both are `expect` rows, and a scanner
that became sound or complete would turn them red, which is the correct signal to
revisit this ADR.

Nothing re-runs the spike automatically — deliberately, as with its three
siblings.

## Pros and Cons of the Options

### Keep Q-A as decided, treat the misses as a known limitation

* Good, because no downstream document moves.
* Bad, because `ADR-0008`'s `scope` / `voided_by` machinery would keep emitting
  `handle_only` as though it meant "everything in `handle`", which is the exact
  misreading that field was added to prevent.
* Bad, because the unsound direction is not a limitation — it is a wrong answer,
  and it points at the project's measured failure mode.

### Reopen, with the measurement attached

* Good, because it separates what holds (token access, the conditional split,
  escape-hatch visibility) from what does not, so #18 can keep the parts that
  earn their place.
* Good, because the two mechanisms and two goals are untouched; only "token
  scanning recovers the contract" falls.
* Bad, because Q-A stays open through #18, and `research-questions.md` regains an
  entry it had closed.

### Abandon generation, keep type enforcement only

* Good, because it removes an unsound signal outright.
* Bad, because it concedes the "leave no lie" half of the goal, which
  §Rejected options already judged unacceptable — and that judgement is not
  changed by this measurement.
* Bad, because P5 shows generation has a use the type system cannot cover.

## More Information

* `spikes/contract-from-tokens/README.md` — the full probe table and the record
  of what the first pass got wrong.
* [ADR-0008](./0008-guarantees-carry-scope-and-voiding-paths.md) — depends on
  this; its `scope` value is what needs replacing.
* [ADR-0004](./0004-reads-enforcement-level.md) — `reads` is the key with no
  backstop; the symmetric fact that generation can never give it a lower bound
  belongs beside its scope note.
* #42 (the semantics of `observed`), #18 (the re-plan that consumes this).
