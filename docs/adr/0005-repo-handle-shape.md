---
status: accepted
date: 2026-08-16
updated: 2026-08-17
decision-makers: itsakeyfut
enforcement-level: none
# `none`, not `upper_bound_checked`: a lifetime is a containment mechanism, not
# an upper bound on a declared set, and it surfaces no AI Context key. The same
# correction ADR-0011 took in #34 ("privacy is not an upper bound").
ai-context-key: none
scope: the handle cannot outlive the borrow of the `Ctx` that produced it
voided-by: path 24 (closed), path 25 (Rule 4 ordering, stated)
---

# `Repo<'req, D, R, M>` — the capability handle carries the request lifetime

> **Decided in #39.** This record was `proposed` while eight places in the specs
> already used the type — a defect under
> [ADR-0000](./0000-record-architecture-decisions.md). The declaration now exists,
> in [`capability-system.md`](../specs/capability-system.md) §The capability handle
> and in `crates/verum/src/capability.rs`.

## Context and Problem Statement

`Repo<D, R, M>` is the capability handle — what `ctx.users()` returns and what a
Service is allowed to receive. It appears in at least eight places:

| Usage | Location |
|---|---|
| `fn users(&self) -> Repo<User, Self::R, Self::M>` | `capability-system.md:191`, `architecture-contract.md:53`, `rust-type-model.md:279` |
| `impl<R, M> UserRepo<M> for Repo<User, R, M>` | `rust-type-model.md:289` |
| "What a service may receive is a parameterised `Repo<D, R, M>`" | `architecture-contract.md:100`, `capability-system.md:232` |
| module placement — `capability.rs` | `docs/rules/design.md:72` |

**There is no declaration.** `grep 'struct Repo' docs/` returns nothing.

That would be a documentation gap on its own. It is worse than that, because the
undeclared shape has a load-bearing consequence that has now been measured.

## Decision Drivers

* **`Repo<D, R, M>` as written has no lifetime parameter, so it cannot borrow.**
  It must own its access to the store, which makes it `'static`.
* **Measured, at runtime**: a handle obtained from a correctly-scoped
  `Ctx<'req, E>` was moved into `tokio::spawn`, the response was sent, and the
  store was mutated 150 ms later — `spikes/ctx-lifetime-rpitit`, probe E1. **The
  `Ctx` is bound to the request; everything it hands out is not.** This is
  ledger path 24 and issue #39.
* **Two candidate fixes were measured and both work**: `RepoLt<'req, D, R, M>`
  (borrows) and `RepoPhantom` (owns, carries `PhantomData<&'req ()>`). Both
  reject the escape with `E0521` (probes E2, E4a) and both still serve an
  ordinary handler (E3, E4b).
* **The escape does not need `tokio::spawn`.** An adversarial pass parked a
  handle in a `static OnceLock` and mutated the store from ordinary synchronous
  code after the request ended. Any wording that names `spawn` as the route is
  too narrow.
* **#39 and #40 are coupled.** `ctx.spawn_owned`'s `JobCtx` holds the store
  directly, so adopting a lifetime-bound `Repo` does not close the path if
  `JobCtx` stays as measured (probe F3).

## Considered Options

* **`Repo<D, R, M>` as written** — no lifetime; owns its access
* **`Repo<'req, D, R, M>`** — borrows the request's runtime
* **`Repo<D, R, M>` with `PhantomData<&'req ()>`** — owns access, carries the lifetime as a marker
* **`'req` *and* `!Send`** — the extra restriction #39 asks about "while here" 

## Decision Outcome

**Chosen: the handle carries `'req` as its first parameter, and the field that
carries it is deferred to #40.**

```rust
pub struct Repo<'req, D, R, M> {
    _req: PhantomData<&'req ()>,
    _model: PhantomData<fn() -> (D, R, M)>,
}
```

> Shown here because a Decision Outcome that does not show what was decided is
> not one. **The canonical declaration is
> [`capability-system.md`](../specs/capability-system.md) §The capability handle**,
> and the implementation is `crates/verum/src/capability.rs`. Both this block and
> that one are **checked** doc blocks, so an arity drift between them fails
> `spikes/doc-code-blocks`.

The deferral is not a hedge, it is the shape of the evidence: **both measured
candidates carry `'req` as the first parameter**, so `RepoLt` and `RepoPhantom`
have the *same declared shape* and differ only in whether the handle borrows the
runtime or owns access plus a marker. The declared shape is what becomes a
breaking change after M2; the field is an internal detail that #40 has to pick
anyway, because the spike's `JobCtx` holds the store directly (probe F3) and this
ADR already records that #39 and #40 cannot be settled independently.

So #39 fixes the half that is expensive to change later and does not pre-empt the
half that is #40's.

The declaration goes into
[`capability-system.md`](../specs/capability-system.md) — as a **checked** doc
block, so the spec and the harness stub are compiled against each other rather
than agreeing on paper. That was this record's own complaint about the placeholder.

### `!Send` is rejected — measured, not argued

#39 also asks whether the handle should be `!Send`. It should not, and the reason
is a pair of probes rather than one:

| Probe | Shape | Result |
|---|---|---|
| **E5** | a `!Send` handle held **across** an `.await` in a handler | `future cannot be sent between threads` |
| **E5b** | the same handle used and dropped **before** the await | **compiles**, and the mutation has already happened |

**E5b carries the rejection; E5 is weaker than first written.** E5b shows `!Send`
buys nothing: `+ Send` on the returned future reaches only what it holds **across**
an await, so a handle used and dropped before the await mutates the store and
compiles. The rule would depend on where the `.await` sits rather than on what the
handle may do.

E5's "and it costs ordinary use" is **narrower than stated**. It is true that
`Handler::handle` returns `impl Future + Send` because hyper's multi-thread runtime
requires it. But the first version of this section argued "in the real design the
setters are `async`, so an ordinary handler *must* hold the handle across an await"
— and that premise **is not the current design**: the capability-checked extension
trait is specified **synchronous** in [`persistence.md`](../specs/persistence.md),
[`mutation-contract.md`](../specs/mutation-contract.md) and
[`rust-type-model.md`](../specs/rust-type-model.md), with only the inner
`UserRepository` async. In the spike the handle's methods are synchronous too, so
E5's across-await hold is produced by an inserted `yield_now().await`. E5 therefore
measures a language fact about any `!Send` value, not a cost specific to this
handle. **未検証**: whether a real handler must hold the handle across an await
depends on a transaction design that does not exist.

Per SRK-009 this is a rejection, not an impossibility claim, and it is not an
enumeration: one case `!Send` *would* block and `'req` does not is
`std::thread::scope`, which moves a non-`'static` borrow across threads. It is not
a lifetime escape — the scope joins inside `'req` — so the rejection stands, but on
E5b alone.

**This is RK-017's shape for the third time** (#14's `+ Send`, #15's getter
bound, and now this): a bound constrains a type, not who supplies it or when.
Either probe alone argues for the opposite conclusion, which is why both are rows.

An invariant **brand** would close it, and is still not taken: the diagnostics
cost is the same objection `capability-system.md` already records, and with `'req`
blocking the attack nothing forces the expensive mechanism yet.

### The field is not "an internal detail" — it decides who is held to the shape

**This is the correction review made to this record's own reasoning, and it is the
part #40 should be read against.** The claim above — both candidates carry `'req`,
so the declared shape is settled and the field is internal — is **true of the
declaration and false of the guarantee**:

| Producer | `_req: PhantomData<&'req ()>` (chosen) | `rt: &'req Runtime` (candidate 1) |
|---|---|---|
| `fn users(&self) -> Repo<'_, ..>` — the prescribed form | compiles | compiles |
| `fn users<'any>(&self) -> Repo<'any, ..>` — unconstrained | **compiles** | `error: lifetime may not live long enough` |

With a marker field, `'req` is whatever the caller asks for, so **nothing in the
compiler holds the emitter to the prescribed signature**; the containment lives in
the derive's generated code. With a real borrow, the compiler forces it. Compile-
verified in both directions, and the spike shows the same split in its own bodies:
`RepoLt` is built from `self.rt` (a borrow, so `'req` is forced) while
`RepoPhantom` is built from `Arc::clone(&self.rt.store)` — a `'static` clone that
constrains `'req` not at all. **E4a measured only the downstream escape**, which is
why the two candidates read as equivalent.

That is RK-017's shape once more: a bound constrains a type, not who supplies it.

**Consequence, recorded rather than resolved.** Choosing the borrowing field is
ADR-0006's business (whether `Ctx` holds a `&Runtime` or an `Arc` is open there), so
#39 does not pick it. What #39 does is stop calling the choice internal:

* With the marker, **`'req` on `Repo` is a convention on the emitter**, not a
  checked property. The fixture pins the hand-written type; nothing pins the
  generated producer.
* **#40 acceptance criterion**: either adopt the borrowing field, or add a
  `compile_fail` fixture asserting that no producer can return
  `Repo<'static, ..>` from `&'a Ctx<'req, E>`.
* rustc's own `help:` on the `E0261` below suggests introducing a fresh lifetime,
  which lands exactly on the unconstrained form. That makes it the shape an AI
  reaches for first.

### Confirmation

**The escape is now measured, and both candidates are measured against it**
(T-M1-02 / #14, `spikes/ctx-lifetime-rpitit/`, `bash run.sh`):

| Probe | What it does | Result |
|---|---|---|
| **E1** | `Repo<D, R, M>` as specified — no lifetime, so it owns its access and is `'static` — spawned out of the request | **passes = the defect.** Asserted at run time: the response returns, and 150 ms later the store reads `escaped@example.com` |
| E2 | candidate 1, `RepoLt<'req, ..>`, under the same attack | `E0521` |
| E3 | candidate 1 still serves an ordinary handler | passes |
| E4a | candidate 2, `RepoPhantom<'req, ..>`, under the same attack | `E0521` |
| E4b | candidate 2 still serves an ordinary handler | passes |

So the decision was not between two unmeasured options: **both block the attack
and neither breaks ordinary use.** What E2 / E4a did **not** probe is the producer
side — see the asymmetry below, which is what actually separates the two.

**The fixture now exists**, which is what this record was missing:

| Fixture | Asserts |
|---|---|
| `crates/verum/tests/ui/compile_fail/repo_handle_cannot_outlive_its_request.rs` | `E0521` — the handle cannot satisfy a `'static` bound |
| `crates/verum/tests/ui/pass/repo_handle_within_request_scope.rs` | ordinary use, passing and returning the handle, still compiles |

It is a bare `'static` bound rather than a `thread::spawn`: `spawn` requires
`Send` **and** `'static`, so a fixture built on it would fail identically whichever
property broke.

**It was mutation-tested, and the mutation has to be stated to be reproducible.**
Reverting `Repo` to its pre-#39 three-parameter shape **and adjusting both
fixtures' type arguments to match** turns the `compile_fail` case green and the
suite red — `1 of 36 tests failed` on the pinned 1.85.0 (`36` = 31 `compile_fail`
+ 5 `pass`). Mutating **only** `capability.rs` gives a different result: both
fixtures go red with `E0107` (wrong lifetime arity), which is still a red suite but
not the same measurement. Review reproduced both, and the first version of this
paragraph named only the outcome, so two reviewers reproducing it got a third
answer.

What the fixture pins, precisely: that a `Repo<'req, ..>` **written by hand** cannot
satisfy a `'static` bound. It does not pin the construction site — see the
asymmetry below.

> ### The two things the doc-block harness cannot do (both were claimed here)
>
> An earlier version of this record claimed the harness verified the declaration
> two ways. Review disproved both by mutation, and the true reasons are worth more
> than the claims were.
>
> **1. Two checked blocks declaring the same type do not check each other.** The
> block in [`capability-system.md`](../specs/capability-system.md) and the one
> above each *declare* their own `Repo`, which **shadows the stub's glob import** —
> `check.py`'s own docstring says local definitions shadow the prelude. Drifting one
> to three or five parameters leaves the harness fully green (`checked:ok 35`,
> exit 0). Nothing in the pipeline compares either block to
> `crates/verum/src/capability.rs`.
>
> **2. A stub cannot enumerate a lifetime-arity change — for any type.** Giving the
> stub `'req` was expected to break every doc block still writing the
> three-parameter form. It broke none, and the reason is **not** that the usages sit
> in `ignore` blocks (they do not: `docs/rules/type-level.md:568` is a *checked*
> block using `Repo`). The reason is that **a lifetime argument may be elided in
> path position**: `Repo<User, R, M>` compiles against `Repo<'req, D, R, M>` with
> **no diagnostic at all** on 1.85.0, the toolchain `check.py` pins. Dropping a
> *type* argument is `E0107`; dropping the lifetime is legal Rust. Measured in both
> directions.
>
> So the sweep was grep-driven, and it had to be. The stub's `'req` still earns its
> place — it rejects a block that *supplies* a lifetime to a lifetime-less `Repo` —
> but that is the opposite of the direction a sweep runs in.

### Consequences

* Good, because the type is declared — in
  [`capability-system.md`](../specs/capability-system.md) and in
  `crates/verum/src/capability.rs` — instead of being assumed by eight sites.
* Bad, because **no mechanism compares the spec's declaration to the code's.** The
  two checked blocks shadow the stub rather than checking against it (above), so
  agreement between spec and implementation is still maintained by hand.
* **The #39/#40 coupling is narrower than this record first stated.** It said the two
  "cannot be decided independently". What is actually coupled is the *field*, not
  the *parameter list*: `'req`'s presence is settled here, and which field carries
  it stays with #40 — together with the producer-side asymmetry above, which is the
  input #40 needs. `docs/rules/api-surface.md` §5 is corrected to match.
* Ledger **path 24** records the route; it is **Closed** by `'req` as of #39.
  The number was written here as "path 25" three times while the ledger held
  only 1–23 — the path did not exist. Appended as 24, per the ledger's own
  "numbers are only ever appended" rule.

## More Information

* #39 — the issue that decided the lifetime question
* #40 — `ctx.spawn`; coupled, see above
* `spikes/ctx-lifetime-rpitit/README.md` — probes E1–E4b, F1–F3
* `docs/specs/unverified-boundaries.md` path 24
* [ADR-0002](./0002-ctxusers-exposes-the-endpoint-as-owner.md) — the extension trait that returns it
