# T-M1-07 / #37 — can token-scanning `handle` recover the Contract?

Measured on **rustc 1.85.0** (Verum's MSRV). Two crates: `mac` is the attribute
macro, `app` is `docs/specs/handler-rules.md` §The complete implementation
example plus the blindness probes. No dependency outside `syn` / `quote` /
`proc-macro2`, at the root workspace's versions.

```bash
bash run.sh          # 15 rows. Every positive probe asserts its JSON in full
```

> **This document replaces a version whose verdict was too kind.** The first pass
> measured only whether effects were *missed*. It never probed whether effects
> were *invented*, and they are. What that changed is recorded in
> *What the first version got wrong* at the end, rather than edited away.

---

## Verdict

> **The premise is false, the mechanism is partially viable, and — the part the
> first pass missed — the mechanism is not monotone.** `observed` is neither a
> subset nor a superset of what the program does. It misses effects that happen
> and reports effects that cannot.

`docs/specs/rust-type-model.md` §What a proc macro can see states the premise as
fact and calls it "what makes the approach feasible". One half holds: a macro on
the impl block really does read every token of `handle`'s body. The other half —
that effects are **syntactically confined** there — is a convention, and
ordinary Rust breaks it in six measured ways.

### Declared vs recovered, on the canonical example

| Key | Declared | Recovered | |
|---|---|---|---|
| `mutates` | `User::name` | `User::name@top` | ✅ |
| `when(EmailChanged).mutates` | `User::email` | `User::email@when:EmailChanged` | ✅ |
| `when(EmailChanged).emits` | `EmailVerificationRequested` | `…@when:EmailChanged` | ✅ |
| `creates` | `AuditLog` | `AuditLog@top` | ✅ |
| `emits` | `UserUpdated` | `UserUpdated@top` | ✅ |
| **`when(EmailChanged).calls`** | **`EmailService`** | **`Email@after_commit`** | ❌ wrong name **and** wrong scope |
| **`reads`** | **`User::id`, `User::status`** | **`User@top`** | ❌ no field granularity |
| `forbidden` | `User::password_hash` | — | a negative: violations would show, compliance cannot be confirmed |

**Count it two ways, because they say different things.** Seven *key instances*,
five recovered. Five *distinct contract keys* (`reads`, `mutates`, `creates`,
`emits`, `calls`; `forbidden` aside), **three recovered**. The second number is
the one #18 should plan against — "three of five" is not "mostly works".

---

## The scan is not monotone

This is the finding the first pass did not look for, and it is the one that
matters most.

**It invents.** An attribute macro runs **before cfg-stripping**, so probe **V2**
puts a statement that is never compiled — naming a type that does not exist —
into the output:

```
CfgGated   emits: ["ThisTypeDoesNotExist@top"]
```

**Why that direction is the dangerous one.** A missed effect makes `observed` a
subset of what is declared, and the difference reads as *over-declaration*. A
phantom effect makes it a superset, and the difference reads as
**under-declaration** — whose repair is to **widen the contract**. That is
precisely the bias `docs/specs/evaluation.md`'s Q-C experiment measured and
RK-010 records: *an AI relaxes the contract rather than fixing the
implementation.* A false under-declaration report is the exact input that
triggers it.

**And the backstop is uneven.** For a key at `upper_bound_checked`, narrowing
the contract is refused by the type system, so a false over-declaration report
is noise rather than damage. **`reads` is `metadata_only` with `scope: none`**
(`docs/specs/ai-context.md`) — nothing refuses narrowing it, and `reads` is one
of the two keys that fails hardest here. The safety argument holds for the
checked keys only.

---

## Where the wall actually is

The first pass said "three ordinary constructs, one of them structural". That
was the wrong cut. There are **two causes**, and they have different fixes.

### Cause A — the item boundary. Inherent.

A proc macro sees the tokens of **the item it is attached to**. `handle`'s body
counts as inside that item, *including nested `fn`s and closures* (**X1**:
visible). Anything in another item is not.

| Probe | Route | Reachable by more analysis? |
|---|---|---|
| P4 | a free associated function taking `&ctx` | yes, with cross-item analysis |
| P7 | a helper in a **sibling** `impl` block | yes, with cross-item analysis |
| **M1** | the effect produced by a **`macro_rules!` expansion** | **no** — a proc macro receives *unexpanded* tokens, and the macro may come from another crate |

`effect-inference.md` §Scope of generation lists avoiding cross-item analysis as
the approach's advantage, so A is a real limit for the First PoC. **M1 is the
only genuinely unfixable member**, and the first pass did not have it.

> **`E0407` is real but proves less than claimed.** A helper cannot sit beside
> `handle` in `impl Handler for X` — R2 pins that. The first pass concluded "no
> arrangement lets the macro see both", which is false: **X1** (a nested `fn`)
> and a trait default method both compile and are both seen. P7 is about *where
> the author put it*, not about what Rust permits.

### Cause B — matching by spelling. A scanner limitation, closable.

Within those tokens, effects are found by matching the identifier `ctx` and the
shape `ctx.<accessor>()`. Nothing type-checks that.

| Probe | Route | |
|---|---|---|
| **V1** | the parameter is named `cx` | **output byte-identical to `Noop`** for a handler that mutates *and* emits |
| P6 | `let repo = ctx.users(); repo.set_name(..)` | missed; every token is inside `handle` |
| U1 | `Repo::set_name(&ctx.users(), ..)` (UFCS) | missed; no indirection at all |

**V1 defeats every recovered key at once, including the conditional split.** An
impl need not reuse the trait's parameter names and nothing warns.

**None of cause B is a law.** V1 is closable at layer 1 — the macro can require
the parameter to be named `ctx` and error otherwise. P6 needs `let`-binding
tracking in the visitor. U1 needs the `ExprCall` form handled. They are listed
as measured limits of *this* scanner, not as properties of the approach — the
first pass presented P6 as author discipline, which was wrong.

---

## Probe table

| # | Construct | Result |
|---|---|---|
| **R1** | `#[observe]` on a struct | rejected, span on the item |
| **R2** | a helper beside `handle` in the observed impl block | `E0407` |
| P1 | `ctx.users().set_name(..)` | `User::name@top` |
| P2 | `ctx.when::<EmailChanged, _>(..)` | `@when:EmailChanged` — **the conditional split survives** |
| **W1** | a `when` nested in a `when` | `@when:EmailChanged+when:AlsoVerified` — **both** conditions |
| P3 | `ctx.after_commit(async \|ctx\| { .. })` | `@after_commit` |
| P4 | a free associated function that emits inside | invisible. Control P4c: the same emit inline **is** seen |
| P5 | `User::from_repr(..)` | **visible** as an escape |
| P6 · U1 · **V1** | aliasing · UFCS · **the parameter renamed** | invisible (cause B) |
| P7 · **M1** | sibling impl · **macro expansion** | invisible (cause A) |
| **X1** | the same helper as a nested `fn` | **visible** — which is why P7 is placement, not law |
| **V2** | a `#[cfg]`-gated statement, never compiled | **appears in the output** |
| NOOP | an empty `handle` | empty — the standing control |
| **D1** | an effect inside `if false { .. }` | **present**, as `@top` — identical to an unconditional one |
| **D2** | does `if false` relieve the declaration obligation? | fail `E0277` — it does not |

W1 aside, everything from V1 down was found in Tier-2 review, not in the issue's
list of five.

---

## What survives, and what it is worth

**P2 and W1 are the strongest results.** `conditional-effects.md` spends three
contract keys distinguishing "email may change" from "email changes", and the
scan preserves it — including nesting, where reporting only the innermost
condition would have claimed a *weaker* precondition than reality. That took two
bug fixes to get right; both are recorded in the mutation table.

**P5 is the one place this improves the picture.** `User::from_repr(..)` never
goes through `ctx.`, so it can never be an effect and generation does not close
ledger path 21 — but it sits in `handle` and the scan finds it. **Where the type
system cannot prevent the forgery, the metadata can stop it being silent.** That
is the minimum guarantee if #33 finds no closing mechanism.

**`calls` fails twice over, and the second is the DSL's.** `ctx.email()` recovers
as `Email` where the contract writes `EmailService` — different vocabularies, no
types to bridge them. And the declared contract puts the call inside
`when(EmailChanged)` while the implementation makes it inside `after_commit`:
**the Contract DSL has no category for "after the commit"**, though
`handler-rules.md` Rule 4 makes the scope semantically distinct. That is a
constraint on the DSL, recorded here rather than as a footnote per #37's third
requirement.

**Field-level `reads` cannot be recovered at all.** `find` yields the whole
domain; field reads are `user.name()` and never go through `ctx.`. This is a
permanent property of the mechanism, not a gap in this scanner.

---

## Two by-products

Both are in the canonical example, both found by trying to compile it.

**`handler-rules.md`'s worked example does not borrow-check.** `E0382` —
`req.name` moves, and `&req` follows two lines later. Reproduced standalone
outside this spike. The example clones `req.email` and not `req.name`.

**Rule 4's `after_commit` uses the form RK-005 already rejected.** Under
`AsyncFnOnce`, `|ctx| async move { .. }` is `E0282` and `async |ctx| { .. }`
compiles. RK-005 records this for `when`; the spec applied it to `when` and not
here. *(An earlier draft said `AsyncFnOnce` was "the only bound that lets the
future borrow `ctx`". It is not — a named lifetime tied to the receiver, and a
HRTB over a boxed future, both accept the original form. The claim is scoped to
`AsyncFnOnce` now.)*

---

## The harness

`run.sh` implements `docs/rules/test.md` §9. Proven by planting mutations —
**all planted and observed on 2026-08-16**:

| Mutation | Caught by |
|---|---|
| the macro emits a constant | five rows red (NOOP correctly still passes — it is the control, not the catcher) |
| the `when` scope is not pushed | `P1-4` red, effects surface at `@top` |
| **only the innermost scope is tagged** | **`W1` red** — the fix this probe exists to pin |
| a `probe`/`expect` line deleted | `FATAL: 14 rows ran, expected 15` |
| a feature typo on the command line | the row goes red (needles are error codes or exact JSON, never prose cargo also emits) |
| a whole probe endpoint deleted | baseline `FATAL` — `src/bin/observed.rs` names every const |
| the `TOOLCHAIN` name is not installed | `FATAL: toolchain … is not installed` |

**§9-14 was checked for both rejection probes** (previously done and not
recorded): moving `apply` out of the trait impl makes the crate compile clean,
and removing `#[observe]` from the struct clears R1. Each measures the cause it
names.

**§9-13's anchor is `src/bin/observed.rs`, not a `const _` block.** An earlier
version carried `const _` anchors; planting showed deleting one left the suite
green, because the bin already names every const. A check that cannot fail is
not a check, so they are gone and the header says what actually holds.

---

## Remaining limits

* **The types are stand-ins.** `Ctx`, the accessors and `when`'s signature exist
  so the crate compiles. The macro reads the AST before type checking, so they
  cannot flatter the result — but the `Ctx` here has **no effect-set parameter**,
  so the claim "the type system still demands the capability" was *not* measured
  through a helper call. Labelled as reasoning, not measurement.
* **`after_commit`'s real signature is not established.** The spike shows the
  spec's form fails under `AsyncFnOnce`; it does not establish that `AsyncFnOnce`
  is the right bound. That belongs with #39 / #40.
* **Nothing re-runs this.** No workflow references `spikes/`, deliberately — but
  `app/src/lib.rs` is a copy of `handler-rules.md`'s example and that file is
  edited by this very change, so the verdict can go stale silently.
* **The rule table is hand-written, and that is the finding.** `set_*`, `find`,
  `create`, `emit` and everything-else each need their own rule; each is a naming
  convention; none can be validated. `ctx.addresses()` still yields `Addresse`.
* **Four lines differ from the canonical example**, not two: `req.name.clone()`
  and `async |ctx|` are the by-product findings; `AuditLog::user_updated(&ctx, ..)`
  adds a `&ctx` so P4's free function can reach an effect at all, and
  `Ok(UserView)` replaces `Ok(UserView::from(user))` for no reason (it compiles
  either way).
* **The output rides on `effect-inference.md`'s `observed` shape so it stays
  comparable; riding on it is not endorsing it.** #42 argues that shape has three
  structural defects and this spike measures the mechanism, not the semantics.

---

## What the first version got wrong

| Claim | Status |
|---|---|
| "the failure mode is false over-declaration; the upper bound is unaffected" | **withdrawn.** V2 shows the scan invents; the direction reverses, and `reads` has no backstop |
| "`E0407` — no arrangement lets the macro see `handle` and its helper" | **withdrawn.** X1 and a trait default method both work |
| "three ordinary constructs break confinement" | **withdrawn.** Six measured, in two causes with different fixes |
| "five of seven observable keys" | **kept, and qualified.** Three of five *distinct* keys |
| "under the only bound that lets the future borrow `ctx`" | **withdrawn.** Two other bounds accept the original form |
| "two lines differ from the spec's example" | **withdrawn.** Four |
| "token access holds; confinement does not" | **stands** |
| P2 / W1, P5, the `calls` DSL gap, `reads` granularity | **stand** |

The common shape: the first pass probed one direction of each question and wrote
the conclusion as if it had probed both.

### #42's defect 1 — dead code is counted by BOTH bounds (D1 / D2)

`if false { ctx.events().emit(UserUpdated)?; }` appears in the emitted JSON as
`"emits":["UserUpdated@top"]` — **`@top`, with no condition tag**, exactly as an
unconditional effect does. And it is still *required* to be declared: the minimal
`Has` shape in `dead_code_upper_bound` gives `E0277` when the effect is not in the
declared set, `if false` notwithstanding.

So a declared-but-dead effect is in **both** sets, and
`declared \ syntactically_present` is empty for it. **The CI gate cannot separate
"declared and dead" from "declared and live"** — which is why #42 / ADR-0014 turned
it into a warning and stated what it *can* catch (a declared effect nowhere written
in the item).

**This is not V2.** V2 is code never *compiled* — the unsoundness, counted by the
scan alone. Dead code is compiled, type-checked, and never *run*, and counted by
both. Two probes because two halves, and this crate's `Ctx` carries no effect set,
so D2 uses the minimal shape rather than the real one.

