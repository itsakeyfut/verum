---
status: "proposed | rejected | accepted | deprecated | superseded by ADR-0000"
date: YYYY-MM-DD
decision-makers: who decided
enforcement-level: "upper_bound_checked | intent_only | metadata_only | none"
---

# Short title, in the form "do X because Y" or "X is Z"

## Context and Problem Statement

What is the problem, and why does it need deciding now? Two or three sentences.
If something in the repository **already depends on the answer** while this ADR
is still `proposed`, say so here — that is a defect, not a normal state.

## Decision Drivers

* the constraints that actually narrow the choice
* prefer measured ones; mark the rest as desk analysis

## Considered Options

* option 1
* option 2

## Decision Outcome

Chosen option: "option 1", because …

### Confirmation

**Which fixture, guard or harness fails if this decision is violated?**

If the answer is "nothing", write that. An unenforced decision is a normal
state — an unenforced decision presented as enforced is the failure this
repository keeps repeating.

### Consequences

* Good, because …
* Bad, because …
* what would reverse this: …

## Pros and Cons of the Options

### option 1

* Good, because …
* Bad, because …

### option 2

* Good, because …
* Bad, because …

## More Information

Links to the specs, fixtures, issues and measurements this rests on.
