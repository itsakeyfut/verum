---
status: accepted
date: 2026-08-18
decision-makers: itsakeyfut
enforcement-level: none
# `none` deliberately. This key is *generated output*, not a guarantee: it carries
# no `enforcement` object, and `check_json.py`'s CLAIMS_EXEMPT names it for exactly
# that reason. Everything it could have claimed is claimed by `mutates` instead.
ai-context-key: syntactically_present
scope: calls spelled `ctx.<accessor>()` inside the item the attribute is on
voided-by: path 22 (`unscanned_effect`) — the scan is neither complete nor sound
---

# `syntactically_present` replaces `observed`, and the lower bound is abandoned

## Context and Problem Statement

Q-A (2026-08-15) decided to generate a **lower bound** — "a declared effect
actually happens" — by scanning `handle`'s tokens, and to let
`declared \ observed` detect over-declaration.

T-M1-07 (#37) compiled the premise and
[ADR-0009](./0009-observed-is-not-a-lower-bound.md) reopened Q-A. That record
corrected the *prose* in `ai-context.md` — it now says "syntactic presence, not
execution" — and deliberately left the vocabulary alone: *"`observed` is otherwise
unchanged here, so that this file does not prejudge either."* It also handed three
judgements to **#18, which is closed**, orphaning them.

This ADR is the second half of that two-step: **#37 measured, #42 names.**

## Decision Drivers

* **A name that overstates its mechanism gets removed here, not annotated.** The
  precedent is this project's own twice over: `type_checked` is **banned** as an
  `enforcement` value because it reads as "verified in both directions", and
  `scope_of_readonly_guarantee` was **deleted** in ADR-0008 rather than qualified.
  `observed` says an effect was seen to happen. Nothing observed anything.
* **Dead code is counted by both bounds — compile-verified.** A call inside
  `if false { … }` still has to satisfy its `Has` bound
  (`error[E0277]: the trait bound (MutateName, ()): Has<MutateEmail, _> is not
  satisfied`), and `if false` is a token, so the scan reports it. So a
  declared-but-dead effect satisfies the upper bound *and* appears in the lower
  one, and the difference the CI gate reads is **empty**. Probes **D1** / **D2**.
* **The conditional split survives the scan** (#37): an effect inside
  `ctx.when::<C>` is tagged with the condition and never appears at top level,
  nesting included. So "it cannot carry a split" would have been false.
* **`reads` can never have a lower bound from this mechanism**, and the reason is
  structural rather than incidental — see below.

## Considered Options

* Keep `observed` and rely on #37's corrected prose
* **Rename to `syntactically_present`, and stop calling anything a lower bound**
* Rename, keeping `observed` as a deprecated alias

## Decision Outcome

```json
"syntactically_present": {
  "unconditional": ["User::name"],
  "conditional":   { "EmailChanged": ["User::email"] },
  "scope": "ctx_spelled_same_item",
  "deferred": "unknown"
}
```

Three names change, and each old one asserted more than its mechanism supports:

| Old | New | What the old name claimed that is not true |
|---|---|---|
| `observed` | **`syntactically_present`** | that the effects were seen to happen |
| `scope: "handle_only"` | **`"ctx_spelled_same_item"`** | that everything in `handle` was covered. It is matched by spelling and confined to one item — ADR-0009 called `handle_only` "a smaller version of the misreading this field exists to prevent" |
| `kind: "service_body"` | **`"unscanned_effect"`** | that the misses are in service bodies. Most are inside `handle`. `uncapped_read` is the precedent for a kind that names an escape rather than a place, and the new name is a **superset**, so every existing `voided_by` reference stays true |

No deprecated alias: the AI Context is unimplemented (M6), so there is nothing to
stay compatible with, and an alias would preserve the misreading it exists to
remove.

### The lower bound is abandoned as a concept, not renamed

This is the part worth stating plainly, because renaming a field can look like
bookkeeping. Q-A's shape was two goals with two mechanisms:

| Goal | Mechanism | After this ADR |
|---|---|---|
| Leave no bypass — an undeclared effect does not happen (**upper bound**) | type enforcement | unchanged, `upper_bound_checked` |
| Leave no lie — a declared effect actually happens (**lower bound**) | generation | **there is no lower bound.** There is a syntactic presence set, which is neither a subset nor a superset of what runs |

What generation still earns, and why it is kept: the conditional split, the
escape-hatch visibility that probe P5 found (`User::from_repr` shows up though it
is not an effect), and a stale-contract detector — see the gate below. "Leave no
lie" is now served by *stating* the gap in `unverified_boundaries`, which is what
this file has always done for everything types cannot reach.

### The conditional split is about syntax, and says nothing about the condition

`conditional` here means the effect is **written inside** a `ctx.when::<C>` scope.
Whether `C` ever holds is `condition_verified`'s business and is `false`
(`Condition::holds` is user code — ledger path 20, permanent). The split exists
because the flat form let the canonical example in `ai-context.md` list
`User::email` unconditionally while `handler-rules.md` performs it inside a `when`
— #42's "flattens in one line the split `mutates` spends three keys making".

### What the CI gate catches, now that dead code is measured

`declared \ syntactically_present ≠ ∅` **cannot see a declared-but-dead effect**,
because both sides count it (D1/D2). What it does catch is a declared effect that
is **nowhere written in the item** — a stale declaration, or a contract copied from
another endpoint. That is a real and common defect, and it is worth a gate.

Against it, two costs that are now recorded where the gate is argued for:

* **False positives** from the six constructs #37 measured — a renamed `ctx`
  parameter voids every key at once, so the gate would fail a correct endpoint.
* **A false under-declaration report's repair is to widen the contract**, which is
  the bias `evaluation.md`'s Q-C experiment measured and RK-010 records. Narrowing
  is refused by the compiler only for keys at `upper_bound_checked`.

So the gate is kept as a **warning** rather than a failure until cause B (matching
by spelling) is closed. That is a change from Q-A's "CI fails".

### `reads` is upper-bound-only, structurally and permanently

Token scanning works because `handler-rules.md` Rule 2 routes effects through
`ctx.`. **Reads do not go through `ctx.`** — they are `user.name()` and
`UserView::from(user)`. So no amount of fixing cause A or cause B yields a lower
bound for `reads`; the mechanism has nothing to match on. Recorded beside
[ADR-0004](./0004-reads-enforcement-level.md)'s scope note, in `read-contract.md`
and in `ai-context.md`, as a property of the mechanism rather than a gap in it.

### Defect 2 — the scan's scope and the architecture spec

`architecture-contract.md:3` constrains "handler → service → repository", and Q-A
scans `handle` only. Follow both and the set is empty while `deferred` holds
everything, at which point the CI gate pushes `@service` onto every endpoint and
the escape hatch becomes the main road — the failure `read-contract.md` describes
where it deleted `into_owned`.

**Resolved by narrowing the architecture claim, not by extending the scan.** The
First PoC's shape is the handler-only scan; `handler → service → repository` is not
its recommended path. Two things already in the specs make this the smaller change:
`architecture-contract.md` §The service layer already records its own position as
**undecided** ("no code example anywhere contains one", and `ctx.users()` "makes
bypassing the service the shortest path"), and `evaluation.md` measured the
29%-shorter result **because the service layer disappeared**. Fragment composition
stays "recorded but not adopted".

What changes is that Q-A stops depending on an answer the architecture spec says is
open. Whether Verum has a service layer at all is still undecided, and this ADR does
not decide it.

### Confirmation

`spikes/contract-from-tokens/` (`bash run.sh` → 17 rows), the two added by this
change being the ones that pin its central claim:

| Probe | What it measures | Result |
|---|---|---|
| **D1** | an effect inside `if false { … }` in the emitted JSON | present, as `@top` — identical to an unconditional one |
| **D2** | does `if false` relieve the declaration obligation? | `E0277` — it does not |

Both mutation-tested: emptying D1's `if false` block changes the JSON and the row
goes red; removing D2's `if false` makes the crate compile (`rc=0`) and its row goes
red.

`kind: "unscanned_effect"` is referenced from ~10 `voided_by` arrays across six
spec files, and ADR-0008 requires every `voided_by` name to exist as a `kind`.
**That join is asserted** at `spikes/doc-code-blocks/check_json.py`, so an
incomplete rename fails mechanically rather than being caught by eye — verified by
planting a partial rename.

**Nothing in `crates/` implements any of this.** The AI Context is M6; this ADR
decides vocabulary and semantics only.

### Consequences

* Good, because no field name in the output implies execution any more, which is
  #42's first requirement and the same standard that banned `type_checked`.
* Good, because the three judgements ADR-0009 handed to the now-closed #18 have an
  owner: the gate's value and the two name replacements are decided here.
* Bad, because "leave no lie" loses its mechanism and keeps only its statement. The
  goal is not abandoned — `unverified_boundaries` is where it now lives — but the
  honest position is weaker than Q-A claimed and should not be re-strengthened
  without a new measurement.
* Bad, because the rename touches ~44 sites. `check_json.py`'s join and the spike's
  exact-JSON needles are what make that safe.
* **ADR-0009 keeps its title.** It is the record of the reopening and `observed` was
  the name at the time; rewriting it would erase what was actually decided. It gains
  a pointer here instead — the same treatment #34 gave the published issue bodies.

## More Information

* #42 — the issue; #37 / [ADR-0009](./0009-observed-is-not-a-lower-bound.md) — the
  measurement this decides on top of
* [ADR-0008](./0008-guarantees-carry-scope-and-voiding-paths.md) — `scope` /
  `voided_by`, and the deletion of `scope_of_readonly_guarantee` as precedent
* [ADR-0004](./0004-reads-enforcement-level.md) — `reads`'s level, now with the
  structural reason it can never gain a lower bound
* `spikes/contract-from-tokens/README.md` — probes including D1 / D2
* `docs/specs/unverified-boundaries.md` path 22 · RK-010, ARK-003
