# Evaluation — the AI coding benchmark

The metrics that make AI coding a first-class design constraint, and the results
of the experiment already run.

Related: [`unverified-boundaries.md`](./unverified-boundaries.md),
[`effect-inference.md`](./effect-inference.md).

---

## Principle

When designing the framework API, treat the following as first-class metrics
alongside human ergonomics.

- AI discoverability / AI context size / AI ambiguity
- AI error rate / AI exploration cost
- Specification violation rate / unexpected behaviour rate

---

# The experiment already run: Q-C premise check (2026-08-14)

Before starting on Verum's type design, an experiment was run to **check the
premises without writing a line of code.**

## Design

The same development task was given to an AI agent under three conditions,
comparing tokens, exploration cost, accuracy, and behaviour on a contract
violation.

| Condition | Contents |
|---|---|
| **1** | Plain Axum + sqlx (7 files, no comments) |
| **2** | The same code as 1, plus a **`CONTRACTS.json` generated from the implementation by static analysis** |
| **3** | A Verum-form contract + handler + `CHEATSHEET.md` (the framework's conventions, compressed) |

Condition 2 is the important comparison. It is **Verum's reader-side benefit
without the type enforcement**, so the gap between 2 and 3 is what the
enforcement adds.

### The subject

A User domain (7 fields) with 4 endpoints (GET / PUT / POST suspend / DELETE).
`PUT /users/{id}` updates name and email, and sends a confirmation mail only when
the email changed. **It does not touch status.**

In condition 1, `UserRepository::save` issues a full-row UPDATE — a realistic
implementation, but one that makes per-field immutability hard to read off the
code.

### The tasks

| Task | Contents | What it measures |
|---|---|---|
| **A (investigate)** | What does `PUT /users/{id}` change, what does it leave alone, and under what condition does it call an external service? | Accuracy of understanding / exploration cost |
| **B (add a feature)** | Add `PATCH /users/{id}/email`. **Nothing but email may change** | Whether the requirement lands in the implementation |
| **C (change the spec)** | Implement "reset status to Unverified when the email changes" | **Contract-relaxation bias** |

Task C is the point. `status` is declared `must_not_mutate` in condition 2 and
`forbidden` in condition 3, creating a situation where the requirement and the
contract contradict each other.

---

## Quantitative results

| Metric | 1. plain Axum | 2. Axum + generated JSON | 3. Verum |
|---|---|---|---|
| Information supplied | 3,007 tokens | 4,198 tokens | **3,213 tokens** |
| of which framework learning cost | 0 | 0 | 1,067 (CHEATSHEET) |
| Code alone | 3,007 | 3,007 | **2,146** |
| Subject's total tokens | 39,790 | 48,246 | 47,931 |
| **Tool calls** | 14 | 18 | **12** |
| Task A correct | **❌ missed status** | ✅ | ✅ |

### The token budget

**Verum's code is 29% shorter** (2,146 vs 3,007), because the service layer
disappears.

```text
saved across 4 endpoints  : 861 tokens
cost of the CHEATSHEET    : 1,067 tokens
────────────────────────────────────────
break-even                : about 5 endpoints
```

**Below 5 endpoints the token budget is negative.**
[`../concepts.md`](../concepts.md)'s "tens to hundreds of tokens instead of
thousands of lines" does not hold unconditionally.

---

## Qualitative results

### Task A — the strength of the evidence differed

Condition 1 **missed** `status` from "never changed".

Comparing the evidence each gave makes the difference clear.

| Condition | Evidence given |
|---|---|
| 2 | "`save` is a full-row UPDATE, but **because the service layer does not rewrite anything except name/email**, the values read are written straight back" |
| 3 | "It is in neither `mutates` nor `forbidden`, so **there is no means of writing it — it cannot be written at the compiler level**" |

Condition 2 is **an observation of the implementation**; condition 3 is **a
guarantee**. Same conclusion, different strength.

### Task B — whether the requirement landed in types

Against "no field other than email may change":

- **Conditions 1 and 2**: both call `save()`, so they **UPDATE every column**. The
  requirement is not met (a concurrent write loses an update on another field).
  Condition 2 noted that "it is hard to tell from the function alone whether the
  immutable fields are protected", but said nothing about lost updates.
- **Condition 3**: declared `mutates = [User::email]` plus
  `forbidden = [User::name, User::status, User::password_hash]`. **The requirement
  landed in types.**

### Task C — the most important result

**All three conditions implemented the status change, and none of them
questioned it.**

| Condition | Behaviour | Final state of the contract |
|---|---|---|
| 1 | 3 lines added | No contract information exists |
| 2 | 3 lines added | **CONTRACTS.json not updated → implementation and contract diverged** |
| 3 | status added to `mutates` + **status removed from `forbidden`** | The contract was updated |

Two things came out of this.

#### 1. Type enforcement does not prevent relaxation bias either

Condition 3 **deleted** an explicit "this is never changed" declaration —
`forbidden = [User::status]` — to fit the requirement. The "problem types do not
solve" recorded in [`unverified-boundaries.md`](./unverified-boundaries.md) was
demonstrated.

#### 2. But type enforcement makes relaxation visible

The subject in condition 2 did not update CONTRACTS.json, and said:

> `CONTRACTS.json` is a file "generated by static analysis at build time" […] I
> have not updated it this time […] **leaving an inconsistency between the
> implementation and the contract is an operational weakness**

**The divergence does not appear in the diff.** Condition 3, by contrast, leaves
an explicit deletion from `forbidden` in the diff.

---

## What this implies for Q-A (whether to adopt generation)

[`effect-inference.md`](./effect-inference.md) said "a generated artefact cannot
drift by construction". More precisely: it cannot drift **if the generation is
run.** If it is not run, the divergence stays invisible.

**What type enforcement adds is not "preventing relaxation" but "surfacing the
divergence on the spot, without waiting for CI".**

The generation approach gets an equivalent effect if CI enforces regeneration
plus a diff check. The difference narrows to **when it is detected** (compile
time vs CI).

---

## What this implies for Q-B (the token budget)

| Claim | Measured |
|---|---|
| Tokens to survey an endpoint | Condition 3 lowest (29% shorter code, fewest tool calls at 12) |
| Tokens to edit an endpoint | Little difference; condition 2 highest (it reads the JSON as well) |
| Framework learning cost | Condition 3 only, 1,067 tokens. **Negative below 5 endpoints** |

---

## A by-product: gaps in the specification the experiment found

Every point the subject hesitated over turned out to be a real gap.

| Observation | Response |
|---|---|
| **Where a conditional mutation is declared was undefined** | Inside the `when` block, or at the top level? The documents contradicted each other → specified in [`conditional-effects.md`](./conditional-effects.md) |
| **`forbidden`'s semantics were unwritten** | "There is no confirmation of what the macro actually checks" → specified in [`mutation-contract.md`](./mutation-contract.md) |
| **The value set of `operation` was unknown** | "To avoid the risk of inventing an enum variant that does not exist, I reused the existing `Update`" → the concern raised in [`research-questions.md`](./research-questions.md) was demonstrated |
| **The list of supported HTTP methods was unwritten** | Whether PATCH was available stayed unknown; "assumed a standard framework supports it" |

**"Where the AI hesitated" works as an indicator of gaps in the specification.**
Collect it in every future experiment.

---

## Limits of the experiment

The constraints needed to interpret the results.

1. **One run per condition** — no statistical confidence; this is an observation
   of a tendency.
2. **Three tasks run in sequence by one agent** — condition 1's wrong answer on
   task A may be memory contamination from writing the report after implementing
   task C (the subject itself added "after task C, status changes too").
3. **Nothing could be compiled** — condition 3's "the types enforce it" rests on
   the subject's **belief**; no error was ever produced.
4. **The designer wrote all three conditions** — condition 3 in particular may be
   unrealistically ideal.
5. **Condition 1's `save()` is a deliberate trap** — realistic, but it worked in
   Verum's favour.

---

## Conclusion

| Claim | Result |
|---|---|
| Per-endpoint immutability is readable | ✅ Condition 1 wrong, condition 3 right and its evidence is a "guarantee" |
| Exploration cost falls | ✅ 12 vs 14 vs 18 tool calls |
| The token budget is positive | ⚠️ **Positive at 5+ endpoints.** Negative below that |
| Relaxation bias is prevented | ❌ **It is not** |
| Relaxation is made visible | ✅ **This is the substantive value of type enforcement** |

**Restating the objective is warranted.** "Reduce the tokens an AI reads" holds
only conditionally, and condition 2 (generated metadata) gets nearly the same
effect.

Where type enforcement beat condition 2 narrows to **forcing the contract to be
updated, and leaving the relaxation in the diff.** Putting that at the centre of
the claim matches the measurements.

---

# Continuing metrics

## What is measured and how

> A review pointed out that a metric name alone does not let anyone reproduce the
> experiment, so the method is defined here.

| Metric | Method | Automatable |
|---|---|---|
| Information supplied (tokens) | Bytes embedded in the prompt ÷ 3.5 | Yes |
| Exploration cost | The agent's tool-call count (Read / Grep / Glob) | Yes |
| Total tokens | The agent harness's usage report | Yes |
| Accuracy of understanding | Scoring task A's four questions against a golden answer | Manual |
| Requirement satisfaction | Whether task B's implementation changes exactly the fields the golden implementation does | Manual |
| **Contract relaxation occurred** | In task C: did it widen the contract / question it / leave it alone? | Manual |
| **Visibility of the relaxation** | Does the relaxation appear in the diff? | Manual |
| Number of points hesitated over | Self-reported by the subject | Manual |
| Iterations / compile-error count | **Measurable after the First PoC** | Yes |

## Protocol

```text
1. Prepare the same subject (User domain + 4 endpoints) under three conditions
2. Give an agent with a fresh context the identical task text under each condition
3. Run tasks A → B → C in order
4. Have it self-report "files read" and "points hesitated over"
5. Do not state the experiment's intent (avoiding the Hawthorne effect)
```

### To improve next time

- **Use a separate agent per task** (avoiding memory contamination across tasks)
- **Repeat each condition three times** (one run shows only a tendency)
- **Have a third party write each condition's code** (avoiding designer bias)
- **Make it compilable after the First PoC** (so iterations can be counted)

---

## Kill criteria — when to change direction

The criteria for judging that a premise was false, fixed in advance.

| Condition | Judgement |
|---|---|
| No measured advantage of condition 3 over condition 2 (generated metadata) **other than making relaxation visible** | **Shrink type enforcement to three items** — GET read-only, `forbidden`, and domain access restriction — and move the rest to generation |
| The token budget stays negative at 5 endpoints | Repoint the objective entirely from "fewer tokens" to "enforcement", and take `ai-context.md` off the critical path |
| Compile time exceeds 2× Axum's | Shrink the scope of type-level computation (narrow `Has`'s subjects by category) |
| "Domain opacity × sqlx" does not hold in the First PoC | Redesign how a domain is exposed. If it still does not hold, reconsider field-level mutation enforcement itself |

These exist to fix the state where **there are three measured criteria for
dropping Axum but none for stopping the project.**

---

## What to compare against

Implement the same problem in the established Rust web frameworks (Axum /
Actix Web / Loco) **and in Axum with generated metadata**, and compare the
metrics above.

**Including Axum-with-generated-metadata matters.** That is Verum's real
competitor; a comparison against plain Axum cannot measure what the type
enforcement is worth.
