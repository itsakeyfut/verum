---
status: accepted
date: 2026-08-16
decision-makers: itsakeyfut
enforcement-level: upper_bound_checked
---

# Expose the endpoint type as an `Owner` associated type on the generated `Ctx` extension traits

## Context and Problem Statement

`Owner` appeared in two documents as `where Self::Owner: Includes<User>` —
`docs/rules/type-level.md` and `docs/specs/architecture-contract.md` — and **was
declared in neither, nor anywhere else.** #43's code-block harness found it as
`E0220: associated type not found`.

This is the case that motivated [ADR-0000](./0000-record-architecture-decisions.md).
A reader could not tell whether `Owner` was undecided or whether they had simply
missed its definition. `Repo`, `Runtime`, `Field` and `When` are in the same
state; this ADR settles only `Owner`.

## Decision Drivers

Three constraints, each independently measured:

1. **`Includes<D>` is implemented on the endpoint type**
   ([ADR-0001](./0001-includes-is-implemented-on-the-endpoint.md),
   compile-verified). So the bound has to name the endpoint.
2. **The where clause goes on the method, not the impl**
   (`docs/specs/rust-type-model.md`, verified). On the impl it yields `E0599`,
   and `#[diagnostic::on_unimplemented]` is discarded — the message Verum wrote
   never reaches the user. `docs/specs/diagnostics.md` treats error wording as a
   specification, so this is a lost feature, not a cosmetic difference.
3. **A trait method's where clause cannot name `E`.** `Self` is `Ctx<'req, E>`,
   not `E`. A method can only write `Self::…`.

## Considered Options

* **An `Owner` associated type on the extension trait**
* **Put the where clause on the impl and drop `Owner`**
* **Give `Ctx` an inherent method instead of an extension trait**

## Decision Outcome

Chosen option: **an `Owner` associated type**, because constraints 1–3 together
leave nothing else writable.

```rust,ignore
pub trait CtxUsers {
    type R;
    type M;
    type Owner;                    // = the endpoint type

    fn users(&self) -> Repo<User, Self::R, Self::M>
    where Self::Owner: Includes<User>;
}

impl<'req, E: Endpoint> CtxUsers for Ctx<'req, E> {
    type R = E::Reads;
    type M = E::Mutates;
    type Owner = E;
    fn users(&self) -> Repo<User, E::Reads, E::Mutates> { /* ... */ }
}
```

> This is closer to "nothing else can be written" than to a preference. It is
> recorded anyway because the name and its meaning were in use in two places and
> defined in zero, and the next reader should not have to re-derive it.

### Confirmation

**Nothing enforces this today.** No fixture fails if the code regresses to the
rejected impl-side form.

A `compile_fail` fixture is required in M3
(M3, task T-M3-04):
reaching an undeclared domain must produce `Includes`'s own message, not `E0599`.
Until that exists, the decision is documented but unguarded — and the impl-side
form still compiles, so nothing would notice the regression.

### Consequences

* Good, because the diagnostic Verum authored actually reaches the user.
* Good, because the constraint is derived from measurements, not taste.
* Bad, because `Owner` is repeated on every generated extension trait —
  `CtxUsers`, `CtxOrders`, … each declare their own. There is no reason to give
  them different names, so the repetition is uniform rather than confusing.
* The derive's generated template needs `type Owner = E;`. This belongs to the M2
  domain-derive task.
* `docs/specs/capability-system.md:190-196` was written in the rejected impl-side
  form and has been corrected.

## Pros and Cons of the Options

### An `Owner` associated type

* Good, because it satisfies constraints 1, 2 and 3 simultaneously.
* Bad, because it adds a name that carries no meaning of its own — it exists to
  route `E` into a method's where clause.

### Put the where clause on the impl and drop `Owner`

```rust,ignore
impl<'req, E: Endpoint> CtxUsers for Ctx<'req, E> where E: Includes<User> { /* ... */ }
```

* Good, because no extra associated type.
* **Bad, because it violates constraint 2.** The user sees
  `E0599: the method 'users' exists but its trait bounds were not satisfied`
  instead of "`Order` is not in this endpoint's domain contract".
* This is the form `docs/specs/capability-system.md` had been carrying, while
  `docs/rules/type-level.md` listed the same shape as a ❌ example. The
  documents disagreed with each other.

### An inherent method on `Ctx`

* **Bad, because `E0116`.** `Ctx` is a framework type; a user crate cannot write
  an inherent impl for it (`docs/specs/rust-type-model.md`, verified). This is
  the reason extension traits exist here at all.

## More Information

* [ADR-0001](./0001-includes-is-implemented-on-the-endpoint.md) — what `Includes` is implemented on
* `docs/specs/rust-type-model.md` §where節はメソッド側に置く — the measurement behind constraint 2
* `docs/specs/diagnostics.md` — why error wording counts as a feature
* `docs/specs/capability-system.md` §実現方法 — the corrected site
