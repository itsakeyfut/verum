---
status: proposed
date: 2026-08-16
decision-makers: itsakeyfut
enforcement-level: none
---

# What `Runtime<Sealed>` is, and whether a sealed token is what closes the god-mode constructor

> **`proposed`, and the design already leans on it** to close ledger path 9.
> Neither `Runtime` nor `Sealed` is declared anywhere.

## Context and Problem Statement

`Ctx` must not be constructible by user code. If it were, every capability check
would be walk-aroundable: build a `Ctx` for an `Endpoint` you declared yourself,
with whatever `Reads` and `Mutates` you like.

The specs close this with a token:

```rust,ignore   // docs/specs/capability-system.md:70 and docs/rules/api-surface.md:510
impl<'req, E: Endpoint> Ctx<'req, E> {
    pub(crate) fn new(rt: &'req Runtime<Sealed>, ...) -> Self;
}
```

> A user cannot construct a `Runtime<Sealed>`, so they cannot make a `Ctx` for an
> arbitrary endpoint type.

**Neither `Runtime` nor `Sealed` is declared.** `grep 'struct Runtime' docs/`
returns nothing, and the `...` in the signature is never expanded. This is ledger
path 9, whose remedy is stated in terms of a type that does not exist.

## Decision Drivers

* **`#[cfg(test)]` does not cross a crate boundary**, so `Ctx::for_test()` cannot
  be gated that way. If a `test-util` feature is enabled transitively it ships in
  the production binary (`capability-system.md`).
* **Visibility alone was measured to be insufficient.** The T-M1-02 spike used
  `pub(crate) fn new` with no token. An adversarial pass then found that
  `ErasedHandler::call` is a **public trait method taking `&Runtime`**, so user
  code can `Box::leak` a `Runtime`, call the erased handler directly, and have
  the framework build a `Ctx<'static, _>` for it. The framework constructed it,
  so `pub(crate)` was never violated — and the construction still happened on the
  caller's terms.
* **`Ctx`'s own field set is undeclared too.** `api-surface.md:519` writes
  `pub struct Ctx<'req, E> { /* ... */ }`. Whether `Ctx` holds a `&Runtime`, an
  `Arc`, or request data changes what the token has to protect, and whether the
  handler future stays `Send` ([ADR-0005](./0005-repo-handle-shape.md) is the
  same problem one level down).
* The test story depends on it: `verum::test::run::<UpdateUser>(req, mocks)` is
  the sanctioned path, and it needs *some* way to build a `Ctx` that user code
  does not have.

## Considered Options

* **A sealed type parameter** — `Runtime<Sealed>`, with `Sealed` unconstructible outside `verum`
* **Visibility only** — `pub(crate) fn new(rt: &'req Runtime)`, no token
* **A sealed trait bound on the constructor** rather than a marker type

## Decision Outcome

**Not decided here.** What this ADR records is that the remedy for path 9 is
written in terms of two undeclared types, and that the cheaper alternative
— visibility alone — has been **measured to leak** through a public trait method.

That measurement is the useful part: it means "make the constructor
`pub(crate)`" is not a substitute for the token, and any design that keeps a
`pub` method taking the runtime reintroduces the hole regardless of how `new` is
declared.

### Confirmation

**Measured in T-M1-02 / #14** (`spikes/ctx-lifetime-rpitit/`):

* **A0** — `app` cannot *construct* a `Ctx`: `Ctx::new` is `pub(crate)` and the
  attempt is `E0624`. That is the whole of what visibility buys.
* **It is not sufficient.** In the spike `ErasedHandler::call` is a public trait
  method, so `app` can `Box::leak` a `Runtime` and drive a handler with
  `'req = 'static` — reaching a live `Ctx` without constructing one. **The
  god-mode route is a supplier, not a constructor**, and blocking the constructor
  does not block the supplier.
* An earlier version of this record derived a coupling with ledger path 8 from
  "every supplier of a `Ctx` imposes `+ Send`". **That premise is false and the
  derivation is withdrawn**: `Handler::handle` returns a `+ Send` future but its
  body is synchronous, so path 8's leak (D5c) is reachable from an ordinary
  handler without any god-mode supplier (RK-017). The two paths may still interact,
  but nothing measured shows it, and path 8 does not depend on path 9 staying
  closed. See [`../specs/unverified-boundaries.md`](../specs/unverified-boundaries.md)
  path 8.

What is still missing is the sealed token itself: neither `Runtime` nor `Sealed`
is declared anywhere, so there is no fixture asserting that a token cannot be
forged.


**Nothing enforces this today.**

* Path 9 has no fixture. `docs/rules/test.md` §4 describes the intended test API
  but nothing asserts that user code cannot reach `Ctx::new`.
* The spike's probe A0 checks only "can `app` name `Ctx::new`" (`E0624`). It does
  **not** check that no other public path hands out a `Ctx` — and the review
  found one that does.

The confirmation this needs is a `compile_fail` fixture per public entry point
that touches the runtime, not one for the constructor alone.

### Consequences

* Good, because path 9's remedy stops being stated in terms of vapour.
* Bad, because the coupling to `Ctx`'s undeclared field set means this cannot be
  settled in isolation.
* **Whatever is chosen, the audit is "which public items accept or return
  something the runtime can build a `Ctx` from", not "is `new` private".** That
  is the shape of the measured leak.

## Pros and Cons of the Options

### A sealed type parameter (`Runtime<Sealed>`)

* Good, because it is checked by the type system rather than by visibility, so it
  survives a `pub` method that takes a `Runtime`.
* Bad, because `Runtime` then has a type parameter that exists only to be
  unconstructible, which shows up in every signature that mentions it.

### Visibility only

* Good, because there is nothing to declare.
* **Bad, because it was measured to leak.** A public trait method taking
  `&Runtime` lets the caller drive construction without ever naming `new`.

### A sealed trait bound on the constructor

* Good, because no marker type appears in `Ctx`'s signature.
* Unmeasured. Whether it composes with the erasure layer's public method is the
  same question that defeated the visibility-only option.

## More Information

* `docs/specs/unverified-boundaries.md` path 9
* `docs/specs/capability-system.md` §Seal the construction route
* `docs/rules/test.md` §4 — the sanctioned test path
* `spikes/ctx-lifetime-rpitit/README.md` — probe A0, and what it does not cover
* [ADR-0005](./0005-repo-handle-shape.md) — the same "undeclared type carrying a guarantee" one level down
