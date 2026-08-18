---
status: accepted
date: 2026-08-18
decision-makers: itsakeyfut
enforcement-level: none
# `none`, not `upper_bound_checked`: what enforces this is rustc's borrow checker
# (`E0521`) plus the payload's `'static` bound, neither of which is an upper bound
# on a declared set, and neither surfaces an AI Context key. `Spawn<J>` itself is
# a declared effect and carries its own level in effect-system.md.
ai-context-key: none
scope: no capability-carrying value is `'static`, at the request level or the job level
voided-by: nothing yet; `JobCtx`'s per-field enforcement is 未検証 (T-M3-02)
---

# `ctx.spawn` takes a **payload**, not a context

## Context and Problem Statement

`Ctx<'req, E>` is non-`'static`, which blocks `tokio::spawn` (ledger paths 6 / 7).
`CLAUDE.md` makes providing a checked alternative in the same change a
non-negotiable, and the alternative the specs promised —
`ctx.spawn::<J>(|jctx| async move { .. })`, where the child context borrows the
parent — **does not compile**: a spawned future must be `'static` and a borrowed
child is not (`E0521`, probe F1). So for the whole of Phase 1 the specs described a
blocked route with nothing beside it, which is exactly what ARK-002 warns produces.

This needs deciding now because #39 closed ledger path 24 (a handle cannot outlive
its request), which removed the last *unchecked* way to do work that outlives the
response. Closing the last workaround while the sanctioned route does not compile
is worse than either alone.

## Decision Drivers

* **The specified shape does not compile** — F1, `E0521`. Measured, and re-measured:
  the first version of that probe stubbed out the `tokio::spawn` call and so never
  required `'static`, reporting a false pass.
* **The obvious fix costs the guarantee.** An **owned** `JobCtx<J>` compiles (F2),
  but it is `'static`, so it can be spawned onward without bound (F3) — a
  capability-carrying value with no request bound then exists, which is the thing
  `'req` was introduced to rule out.
* **`'req` is already manufactured this way, one level up.** `serve.rs` clones the
  `Runtime` **into each request's own future**, and `'req` is the borrow of that
  clone — recorded in `capability-system.md` §The erasure layer builds the `Ctx`.
  Nothing prevents a spawned task from doing the same.
* **A higher-ranked closure over a context is the shape that keeps failing.** The
  `when` scope produced `not general enough` repeatedly (probes D1, D5a–D5d). The
  trait-method shape (`Handler::handle`) is the one measured to work.
* **#39 removed the smuggling route for free.** Every capability handle is now
  non-`'static`, and a payload crossing into a spawned task must be `'static`.

## Considered Options

* **A child context that borrows the parent** — the shape the specs promised
* **An owned `JobCtx<J>`** handed to a closure
* **A payload in, and the framework builds the job's context inside the task**
* Both the payload form and the owned form, the latter as a recorded escape hatch

## Decision Outcome

Chosen: **the handler hands over a payload; the framework constructs the job's
context inside the spawned task, borrowed from a `Runtime` clone that task owns.**

```rust,ignore   // fragment, not a complete item
// the handler — no capability crosses the boundary
ctx.spawn::<SendEmailJob>((user.id(), req.email))?;

// the job the user declares — shaped like `Handler`, not like a closure
impl Job for SendEmailJob {
    type Payload = (UserId, Email);
    fn run(ctx: JobCtx<'_, Self>, (id, to): Self::Payload)
        -> impl Future<Output = Result<()>> + Send { /* ... */ }
}
```

So the spawned **future** is `'static` while the job's **context** is not. What
crosses is a `Runtime` — not a capability, but the thing a context is built *from*,
whose constructor is what [ADR-0006](./0006-runtime-sealed-token.md) gates — plus an
owned payload.

#### `'job` is bounded by the construction site, not by the type

**This record's own reasoning needed narrowing, and review measured it.** The
sentence above is true of the implementation and is *not* a property of the type:

```rust,ignore   // fragment, not a complete item
let owned: Runtime = self.rt.clone();          // ← the guarantee lives here
tokio::spawn(async move { let jctx = ScopedJobCtx { rt: &owned, .. }; .. })
```

Replace that line with `Box::leak(Box::new(self.rt.clone()))` and a
`ScopedJobCtx<'static, J>` is constructible — **compiles with zero errors**, and F5's
attack then succeeds. So `'job`'s boundedness rests on `spawn` borrowing a
**task-owned** value.

This is not a new kind of dependency; it is the **same one `'req` already carries**,
and `capability-system.md` states it plainly under §The erasure layer builds the
`Ctx`: *"A `Ctx` borrowing a server-lived `Runtime` would satisfy the recorded
signature exactly, be non-`'static` exactly as required, and still span the whole
process. The lifetime carries the guarantee only because of where the borrowed value
lives — so this constraint belongs in the derive's generated code, not in a
convention."* Every word of that applies one level down, and the first version of
this record reused the mechanism without carrying the caveat forward.

**Nothing detects a regression here.** F5 measures whether the context *as
constructed* can be re-spawned; it cannot see a differently-constructed one — the
same blind spot #39's E2 / E4a had, which measured only the downstream escape and so
made `RepoLt` and `RepoPhantom` read as equivalent. The construction site carries a
load-bearing comment saying so, in the shape ADR-0010 uses for the `Repr`'s
visibility, because a comment is what is available and a probe is not.

**#60 / `T-M3-02` acceptance criterion**: the generated `spawn` borrows a task-owned
value, and either a fixture pins that or the ledger records that none does.

**The name stays `ctx.spawn::<J>`.** The specs, the rules and the roadmap already
promise that name; only the argument changes, from a closure to a payload.

### Why the owned form is rejected rather than kept as an escape hatch

It works. It is rejected because it would make the sentence "no capability-bearing
value is `'static`" false, and that sentence is what paths 6 and 7 rest on. #40 was
filed expecting exactly that trade — *"the argument for why `'req` closes those
paths has to be re-derived"* — and the payload form shows the trade is avoidable.
Per ARK-002 one checked route is enough; a second one that costs a guarantee is not
an escape hatch, it is the unchecked route wearing a proof token.

### Confirmation

`spikes/ctx-lifetime-rpitit/` (`bash run.sh` → `25 as specified, 0 unexpected`):

| Probe | What it does | Result |
|---|---|---|
| **F1** | the specified shape — the child borrows the parent | `E0521` |
| F2 | an owned `JobCtx<J>` | compiles and runs |
| F3 | that owned `JobCtx` spawned onward from inside the job | compiles — **the cost, and why F2 is rejected** |
| **F4** | this decision's shape | compiles, and `f4_scoped_job_mutates_after_the_response` observes the store change **after** the response |
| **F5** | F4's context spawned onward from inside `Job::run` | `E0521` — **the cost is not paid** |
| **F6** | a capability handle smuggled through `J::Payload` | `E0521` |

F4's live test was mutation-tested: removing the job's write turns it red rather
than leaving it green.

**No fixture in `crates/verum` asserts any of this**, and cannot yet — there is no
`Ctx` there (`crates/verum/src/` is `capability.rs`, `domain.rs`, `lib.rs`,
`sealed.rs`, `typelevel.rs`). The `compile_fail` for `tokio::spawn(ctx)` and the
`pass` for the sanctioned route are relocated to **#60** / `T-M3-02`, which is where
`Ctx` and the extension traits arrive.

### Consequences

* Good, because the alternative now exists, and **paths 6 and 7 keep their original
  argument** instead of needing a re-derivation. The ledger rows are corrected to
  say the alternative is provided. ⚠️ That argument is conditional in exactly the way
  `'req`'s already is — see §`'job` is bounded by the construction site — so it is
  "unchanged", not "unconditional".
* Good, because a named `Job` type with a declared contract is more inspectable from
  the AI Context than an anonymous closure — the ceremony buys something the
  framework wants.
* Bad, because it is more ceremony than a closure: the user declares a type and an
  impl for what could have been three lines inline.
* **未検証**: that `JobCtx`'s methods enforce `J`'s declared capability set. The
  spike's stand-in is unconditional. The mechanism is **identical to `Repo`'s** — a
  derive-emitted extension trait with `where J::Mutates: Has<..>` on the *method*
  (ADR-0002, #39) — so nothing new is assumed, but nothing is compiled either. An
  acceptance condition of `T-M3-02`.
* The transaction boundary is still undesigned, so this says nothing about
  **ordering** — an effect can still fire before a commit. That is ledger path 25
  and `handler-rules.md` Rule 4, unchanged by this decision.

## More Information

* #40 — the issue this decides; #39 / [ADR-0005](./0005-repo-handle-shape.md) — the
  handle lifetime, which removed the smuggling route
* [ADR-0006](./0006-runtime-sealed-token.md) — the constructor gate; still `proposed`
* `spikes/ctx-lifetime-rpitit/README.md` — probes F1–F6
* `docs/specs/capability-system.md` §Provide an alternative route for spawning
* `docs/specs/unverified-boundaries.md` paths 6, 7, 24, 25 · ARK-002, RK-012
