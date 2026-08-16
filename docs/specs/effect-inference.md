# Effect inference and static verification

Detecting divergence between the declared contract and the implementation, and
the "generate it from the implementation" alternative.

Related: [`capability-system.md`](./capability-system.md),
[`unverified-boundaries.md`](./unverified-boundaries.md),
[`persistence.md`](./persistence.md).

---

## Why it is needed

The moment a contract and its implementation diverge, the contract drops to the
credibility of a comment. That is the state Verum exists to reject.

```text
Declared effects  VS  the effects the implementation actually causes
```

---

## The approach taken: require capabilities in types and let rustc do the matching

```text
A repository setter requires where M: Has<Mutate<User, user::Email>, I>
        ↓
A capability the endpoint did not declare is not in E::Mutates
        ↓
Compile error
```

This avoids writing a cross-crate call-graph analysis (`rustc_private` / MIR).

### Two limits of this approach

**1. A contract is only an upper bound**

Type checking sees one direction only: implementation ⊆ contract. An effect
declared but never used — over-declaration — is not detected.

```rust,ignore   // needs a macro that arrives in M2
#[contract(mutates = [User::name, User::email])]
// the implementation only changes name → no error
```

So an AI reading `mutates = [name, email]` and concluding "this endpoint changes
name and email" is **wrong**. The correct reading is "it changes nothing but name
and email".

The `enforcement` field in the AI Context has to carry a value that makes the
difference visible.

```json
"mutates": {
  "enforcement": {
    "level": "upper_bound_checked",
    "scope": "handle_via_ctx",
    "voided_by": ["domain_repr", "repository_impl", "service_body", "..."]
  }
}
```

The spelling `type_checked` is not used, because it reads as "verified in both
directions".

**2. The correctness of the matching rests on hand-written boilerplate**

Writing `Has<Mutate<User, user::Name>, I>` by mistake in `set_email`'s where
clause is not detected. The claim "rustc does the matching for us" rests on a
weaker premise than it sounds: **that whoever wrote the repository trait did not
make a mistake.**

→ Generating the repository **trait definition** from a derive takes priority
([`persistence.md`](./persistence.md)).

---

## Decision (Q-A): keep **both** type enforcement and generation, and make the difference the detector

**Decided 2026-08-15. ⚠️ REOPENED 2026-08-16 by T-M1-07 (#37)** — the premise
this decision rests on was measured and **does not hold as written**. The
rejected options are at the end.

> ### What #37 measured
>
> Reproduction: `spikes/contract-from-tokens/` (`bash run.sh`, 9 rows on rustc
> 1.85.0, the worked example from [`handler-rules.md`](./handler-rules.md)).
>
> **The mechanism half works.** Three of the five distinct contract keys come
> back exactly — five of seven if `when`-scoped instances are counted separately,
> and the first number is the one to plan against. The conditional split
> survives — an effect inside `ctx.when::<EmailChanged>` is tagged with the
> condition and never appears at top level. `ctx.after_commit` is distinguishable
> too. So generation is not idle.
>
> **Two keys do not come back.**
>
> * **`calls`** fails twice: `ctx.email()` recovers as `Email` where the contract
>   writes `EmailService` (different vocabularies, and the macro has no types to
>   bridge them), and the declared contract puts the call inside
>   `when(EmailChanged)` while the implementation makes it inside
>   `after_commit`. **The DSL has no category for "after the commit"** — a
>   constraint on the Contract DSL, not a macro limitation.
> * **`reads`** has no field granularity. `find` yields the whole domain; field
>   reads are `user.name()` and do not go through `ctx.`. This is what #42's
>   third defect argued, now measured.
>
> **The confinement premise is false**, in two groups with different fixes.
> *Cause A, the item boundary:* a free associated function taking `&ctx`, a
> helper in a sibling `impl`, and **an effect produced by a `macro_rules!`
> expansion**. Only the third is unreachable in principle — a proc macro receives
> unexpanded tokens, and the macro may come from another crate. *Cause B,
> matching by spelling:* **naming the handler's parameter anything but `ctx`
> voids every key at once**, and `let repo = ctx.users()` and UFCS are missed.
> Cause B is closable; cause A's third row is not. `E0407` forbids a helper
> *beside* `handle`, but a nested `fn` and a trait default method are both
> visible, so placement is the variable, not the language.
>
> **And it is not monotone.** A proc macro runs before cfg-stripping, so a
> `#[cfg]`-gated statement naming a type that does not exist **appears in the
> output**. The generated set is neither a subset nor a superset of what runs.
>
> **Which way it fails, and where the backstop stops.** A missing effect reads as
> over-declaration; a phantom one reads as **under-declaration**, whose repair is
> to widen the contract — the bias `evaluation.md`'s Q-C measured and RK-010
> records. Narrowing is refused by the type system only for keys at
> `upper_bound_checked`; **`reads` is `metadata_only` with `scope: none`, and it
> is one of the two keys that fails hardest here.** (The upper-bound half is
> compile-verified in T-M0-08. The spike's `Ctx` has no effect-set parameter, so
> transitivity through a helper was *not* measured, and the CI gate does not
> exist — both are reasoning, not measurement.)
>
> **One thing improved.** `User::from_repr(..)` is **visible** (probe P5). It can
> never be an effect — it does not go through `ctx.` — so generation does not
> close ledger path 21, but it can stop the bypass being silent. That is the
> minimum guarantee if #33 finds no closing mechanism.
>
> **What reopening means.** The two goals and the two mechanisms still stand; what
> does not is "token scanning recovers the contract". #18 decides the shape, with
> the measurement rather than the assumption as its input.

### The shape of the decision

There are two goals, and they **require separate mechanisms.**

| Goal | Meaning | Mechanism |
|---|---|---|
| **Leave no bypass** | An undeclared effect does not happen = **upper bound** | Type enforcement (`Has` plus the extension trait's where clause). Already in place |
| **Leave no lie** | A declared effect actually happens = **lower bound** | **Generation** (scanning `handle`'s tokens). Types cannot produce this in principle |

Type checking sees only implementation ⊆ contract, so it **cannot detect
over-declaration** (§Two limits of this approach above). What `mutates = [name]`
guarantees is therefore only "nothing but name changes", not "name changes".
**Either mechanism alone satisfies only half the goal.**

```text
declared_ceiling   hand-written #[contract] + type enforcement  → "nothing else happens"
observed_effects   generated by scanning handle's tokens        → "this happens"

observed ⊄ declared     → compile error (the type half; already in place)
declared \ observed ≠ ∅ → over-declaration. CI fails (below)
```

**The difference between the two solves exactly the open problem
[`research-questions.md`](./research-questions.md) §Detecting over-declaration
raised.** Taking that difference is the main purpose of this decision, not a
by-product.

### Scope of generation — the First PoC covers `handle` only

**A proc macro sees only the tokens of the item it is attached to.** An attribute
macro on `handle` can see the whole body, but not inside the services that body
calls. A cross-crate call-graph analysis is not written — §The approach taken
above lists avoiding it as an advantage.

The First PoC's generation scope is therefore the inside of `handle`, and **that
scope is stated explicitly in the AI Context.**

```json
"observed": { "fields": ["User::name"], "scope": "handle_only", "deferred": "unknown" }
```

> **⚠️ `"handle_only"` overstates what T-M1-07 measured, and is due for
> replacement.** The scan does not cover all of `handle`: it matches receivers by
> spelling, and it cannot follow a call into another item. A value naming the real
> boundary — what is spelled `ctx.<accessor>()` inside this one item — would not
> produce the misreading this field exists to prevent. Tracked on path 22.

What is out of view — service bodies, free-function constructors, raw SQL inside
a repository implementation, **and the six constructs T-M1-07 measured** — is
recorded in [`unverified-boundaries.md`](./unverified-boundaries.md) and emitted
in the AI Context.

**A possible extension, recorded but not adopted**: annotate every item that
carries an effect, have each emit a fragment, and take the transitive closure at
build time — that reaches services without a cross-crate analysis. Services are
out of scope for the First PoC, so it is not taken now.

### When over-declaration appears — CI fails

> **⚠️ Revised by T-M1-07 (#37).** This section was argued on the assumption that
> the declared-vs-observed difference has no false positives. It has both
> directions: a missed effect produces a false **over**-declaration report, and a
> `#[cfg]`-gated statement produces a false **under**-declaration one. The sample
> error below asserts "declared but never mutated in `handle`", which the scan
> cannot establish, and neither suggested repair is correct for a
> renamed-parameter or aliased-receiver shape. Whether a CI gate is still
> proportionate is #18's to decide with the measurement in hand.

**The difference cannot be computed by a single proc macro.** The declaration is
`#[contract(...)]` on a unit struct and the implementation is `handle` in
`impl Handler for X` — **different items**. The matching happens at build time,
reading the artefacts both of them emitted.

> **So this is none of the three defence layers.** It is not macro, equality
> bound or trait bound, but **a fourth mechanism at build time.** Do not read
> [`diagnostics.md`](./diagnostics.md)'s layer table as covering it. **It does not
> become a compile error** — its strength differs from the type half (an
> undeclared effect).

```text
error: `User::email` is declared in `mutates` but never mutated in `handle`
  help: remove it from the contract, or mark it `@service` if a Service performs it
```

**Over-declaration is not a security hole; it produces a mistaken reader**, so a
CI gate is proportionate. An undeclared effect — a bypass — stays a compile
error.

**The escape route is explicit, and using it is itself recorded.** To declare an
effect a service performs, mark it `@service`; it is emitted under `deferred`.
A non-empty `deferred` also shows up in `unverified_boundaries`, so **the act of
escaping leaves a record** — the same reasoning that makes `forbidden` work as a
recorder of intent.

### The premise, measured in Phase 1 (was: unverified)

> **⚠️ RESOLVED by T-M1-07 (#37): the premise does not hold as written.** Token
> scanning recovers five of seven observable keys, including the conditional
> split, and misses `calls` and field-level `reads`. Effects are **not**
> syntactically confined to `handle` — see §Decision (Q-A) above for the
> measurement, and `spikes/contract-from-tokens/` to reproduce it.
>
> The original note, kept because it records why the spike existed:
>
> > **This decision rests on the premise that token scanning alone can
> > reconstruct the whole contract, and that premise has never been compiled.**

It is the claim §Rejected options and why below rests on.
This project has been wrong in both directions about what macros can do
(RK-003 / RK-004, T-M1-01's `E0428`), and `CLAUDE.md` requires that any claim
about compiler behaviour be checked by compiling.

**T-M1-07 ran on 2026-08-16** (#37), against the worked example in
[`handler-rules.md`](./handler-rules.md). The verdict is in §Decision (Q-A)
above; the five questions it was set were:

1. `ctx.users().set_name(..)` → can it be reconstructed as `Mutate<User, user::Name>`?
2. `ctx.when::<C>(.., async |..| { .. })` → can the closure body be placed inside `When<C, ..>`?
3. `ctx.after_commit(|ctx| ..)` → can the scope be distinguished?
4. `AuditLog::user_updated(&user)` → **confirm it is invisible** (demonstrating the reliance on convention)
5. `User::from_repr(..)` → **can it be detected as an escape?**

Item 5 bears directly on ledger path 21. **Generation does not close path 21** —
`from_repr` does not go through `ctx.`, so it never appears as an effect — but
**token scanning can find a `from_repr` inside `handle`**, so even without
closing it, it **can be made visible.** That is the minimum guarantee if #33
fails to find a way to close it.

### Rejected options and why

| Option | Reason for rejection |
|---|---|
| **Type enforcement only (status quo)** | Amounts to explicitly conceding the second half of the goal, "leave no lie". Banning the word `type_checked` to prevent misreading means **giving up readability in exchange for preventing lies**, which does not match the goal |
| **Generation only** | Loses the closed loop (an AI cannot make progress while violating, and self-corrects without a human review gate). `forbidden` — intent stated in advance — cannot be expressed by generation. Generation is **description, not prevention** |
| **Generation primary, types reduced to a minimal core** | The concept count (~40) drops and Q-B's token budget improves, but the `mutates` closed loop is lost. **T-M1-07 is now measured** (#37): generation recovers three of five keys, is not monotone, and cannot be completed for `macro_rules!` expansions. That weakens this option rather than strengthening it, but the trade is #18's to re-run against the numbers; Q-B is still unmeasured |
| **Cross-crate call-graph analysis** | Depends on MIR / `rustc_private`. This file lists avoiding that as an advantage, and a nightly dependency conflicts with the MSRV policy |
| **Report the difference only** | Does nothing about contract-relaxation bias (§What types do not solve below) |

---

## Where inference is genuinely needed (if type enforcement is kept)

1. **Anywhere an escape hatch was used**
2. **Raw SQL** (inside a repository implementation)
3. **Side effects of free-function constructors**

**Item 1 is visible after all** — T-M1-07's probe P5 (#37) found
`User::from_repr(..)` sitting in `handle` and reported it. It is still not an
*effect*, because it does not go through `ctx.`, so generation does not close
ledger path 21; what it can do is stop the bypass being **silent**. Items 2 and
3 are not visible. The scope is bounded, so they can be added later.

---

## The repository implementation as a trust boundary

```text
Endpoint / service layer  → guaranteed by types (rustc does the matching)
Repository implementation → trust boundary (subject to review and audit)
DB                        → out of scope
```

> **⚠️ The first line above does not hold today** (measured in T-M1-01 / #13).
> While ledger **path 21** is open, ordinary code in the endpoint or service layer
> can forge a domain with `User::from_repr(UserRepr { .. })` — no capability, no
> repository, no SQL, no `unsafe`. The diagram describes the state after path 21
> is closed. Detail in [`persistence.md`](./persistence.md) §Verdict.

Means of narrowing the boundary are in [`persistence.md`](./persistence.md).
Every unchecked boundary is listed in
[`unverified-boundaries.md`](./unverified-boundaries.md) and emitted in the AI
Context.

---

## What types do not solve: contract-relaxation bias

Faced with a compile error, an AI **widens the contract by one line rather than
fixing the implementation.**

```text
error: undeclared mutation `User::status`
  help: add `User::status` to the contract, or remove this call
        ↑ the AI tends to pick this one
```

Once a contract stops being "a contract constraining the implementation" and
becomes "a label that widens to follow the implementation", type checking is
meaningless. Worse, **the reassurance of "it is guaranteed by types" pulls a
reviewer's attention away**, so the inversion is possible where the same bug is
*harder* to spot than it would have been in Axum.

This is an operational problem, not a type-system problem. Countermeasures are in
[`unverified-boundaries.md`](./unverified-boundaries.md) — detecting
contract-widening diffs in CI, among others.

---

## Priority of the guarantee mechanisms

```text
Type system → AST → static analyser → code generator → compiler
```

The goal:

> Even when an AI writes wrong code, it can be caught as a contract violation
> before it runs.

But **it is not claimed that every violation is caught.** The scope of detection
is stated in [`unverified-boundaries.md`](./unverified-boundaries.md).

---

## Priority

Inference is not implemented in the First PoC. How far the capability approach
reaches is measured first, and only the gaps left over are designed for.

