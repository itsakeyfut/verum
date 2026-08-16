---
status: proposed
date: 2026-08-16
decision-makers: itsakeyfut
enforcement-level: upper_bound_checked
---

# What `Repo<D, R, M>` is, and whether it carries the request lifetime

> **`proposed`, and eight places in the specs already use it.** Per
> [ADR-0000](./0000-record-architecture-decisions.md) that is a defect. The type
> is written down nowhere; every usage assumes a shape nobody wrote.

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
  ledger path 25 and issue #39.
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

## Decision Outcome

**Not decided here.** This ADR records that the type is undeclared, what the
usages constrain it to, and that the lifetime question is #39's to settle.

What this ADR *does* decide is that **the specs must not keep using a type they
never declare.** Whichever shape #39 picks, a declaration goes into
`docs/specs/capability-system.md` beside the extension-trait definition, and the
other sites link to it.

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

So the decision is no longer between two unmeasured options: **both block the
attack and neither breaks ordinary use.** What is still missing is the decision
itself, and one coupling — the spike's `JobCtx` holds the store directly, so
**#39 and #40 cannot be decided independently** (ADR-0006, probes F2 / F3).

* No `compile_fail` fixture in `crates/verum` asserts that a capability handle
  cannot outlive its request. E1 demonstrates the escape in a throwaway
  workspace; the fixture that would keep it closed does not exist yet.
* `spikes/doc-code-blocks` cannot check the usages either — with no declaration
  to transcribe, the stub carries a placeholder
  (`pub struct Repo<D, R, M>(PhantomData<fn() -> (D, R, M)>)`) that is marked
  `UNDECLARED` and proves nothing about the real design.

When #39 decides, a `compile_fail` fixture for "handle moved past the request
boundary" is the confirmation this ADR is missing.

### Consequences

* Good, because the eight usages now have one place that admits the type is not
  defined.
* Bad, because the placeholder in the doc-block harness will keep those usages
  compiling, which reads as verification and is not. The stub says so; nothing
  mechanical does.
* Whatever #39 chooses must be decided **together with #40** — see the coupling
  above.
* Ledger path 25 stays open until then, and the AI Context emits
  `capability_handle` as an unverified boundary.

## More Information

* #39 — the issue that decides the lifetime question
* #40 — `ctx.spawn`; coupled, see above
* `spikes/ctx-lifetime-rpitit/README.md` — probes E1–E4b, F1–F3
* `docs/specs/unverified-boundaries.md` path 25
* [ADR-0002](./0002-ctxusers-exposes-the-endpoint-as-owner.md) — the extension trait that returns it
