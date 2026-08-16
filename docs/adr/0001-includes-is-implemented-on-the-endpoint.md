---
status: accepted
date: 2026-08-16
decision-makers: itsakeyfut
enforcement-level: upper_bound_checked
---

# Implement `Includes<D>` on the endpoint type, not on the domain set

## Context and Problem Statement

The Architecture Contract holds that an endpoint may only reach the domains it
declared. `Includes<D>` is the trait that answers "may this endpoint touch
`Order`?".

**The implementation and the specs disagreed about what that trait is
implemented *on*.** The specs write `where E::Domain: Includes<User>` — the
domain set as the subject. `crates/verum` implements it on the endpoint. The
disagreement surfaced in #43, when the specs' code blocks were compiled for the
first time and this one did not.

Whichever side is wrong, the other is a bypass waiting to happen: a where clause
written against the wrong subject either never holds (the feature is dead) or
holds vacuously (the contract is not enforced).

## Decision Drivers

* The implemented, sealed, tested code is the ground truth; the spec prose
  predates it.
* `#[diagnostic::on_unimplemented]` on `Includes` names `{Self}` in its message —
  whatever `Self` is has to be what a reader sees in the error.
* The choice determines whether `CtxUsers` needs an associated type at all
  ([ADR-0002](./0002-ctxusers-exposes-the-endpoint-as-owner.md)).

## Considered Options

* **The endpoint type is the subject** — `where E: Includes<User>`
* **The domain set is the subject** — `where E::Domain: Includes<User>`

## Decision Outcome

Chosen option: **the endpoint type is the subject.**

```rust,ignore   // the implementation, crates/verum/src/domain.rs:45-46
impl derive_facing::SealedIncludes<Order> for GetOrder {}
impl Includes<Order> for GetOrder {}
//                     ^^^^^^^^ an endpoint, not a domain set
```

so bounds read:

```rust,ignore
where E: Includes<User>          // correct
where E::Domain: Includes<User>  // wrong
```

Four pieces of evidence in the implementation agree, and all of them are
compile-verified:

| Evidence | Location |
|---|---|
| The doc comment says "Declares that an **endpoint's** domain set contains `D`" | `crates/verum/src/domain.rs:5` |
| The diagnostic reads `` `{Self}` does not declare the domain `{D}` `` — `Self` is the endpoint | same, `:21` |
| The satisfiability test is `impl Includes<Order> for GetOrder` | same, `:45-46` |
| The UI fixture calls `reaches_order::<GetUser>()` | `crates/verum/tests/ui/compile_fail/includes_undeclared_domain.rs` |

**The specs already contain the correct form once**, in a quoted error message at
`docs/rules/api-surface.md:424`:

```text
error[E0277]: the trait bound `MyEndpoint: Includes<Order>` is not satisfied
```

So the documents contradicted themselves: the where clauses named one subject and
the error message those where clauses would produce named the other.

### Confirmation

* `crates/verum/tests/ui/compile_fail/includes_undeclared_domain.rs` — an
  unsatisfied bound at a use site, which is the shape almost every real error
  takes.
* `crates/verum/src/domain.rs::includes_should_be_satisfiable_for_a_declared_domain`
  — proves the trait can hold at all, not merely that the bound can be written.
* `spikes/doc-code-blocks/run.sh` compiles the blocks in the specs, so the wrong
  form cannot be reintroduced silently.

### Consequences

* Good, because the error a user sees now matches the bound that produced it.
* Good, because it settles [ADR-0002](./0002-ctxusers-exposes-the-endpoint-as-owner.md)
  rather than leaving `Owner` to inference.
* Bad, because five documents were wrong and had to be corrected — including
  `docs/roadmap/M3-capability-enforcement.md:52`, **the task definition an M3
  implementer reads first.**
* The domain set (`E::Domain`) is not the subject of `Includes`. What it is for
  is outside this ADR.

### Sites corrected

| Location | Kind |
|---|---|
| `docs/specs/capability-system.md:194` | where clause on the impl |
| `docs/specs/capability-system.md:210` | prose |
| `docs/rules/type-level.md:408` | ❌ example (the *placement* was the point; the subject was wrong too) |
| `docs/specs/research-questions.md:58` | summary of a settled item |
| `docs/roadmap/M3-capability-enforcement.md:52` | M3 task definition |

`docs/specs/architecture-contract.md` already used the endpoint form and needed
no change.

## More Information

* [ADR-0002](./0002-ctxusers-exposes-the-endpoint-as-owner.md) — what `Owner` denotes
* `docs/specs/architecture-contract.md` — the Architecture Contract itself
* `docs/specs/unverified-boundaries.md` path 13 — forging `Includes`, and the seal
