# Diagnostics

The design of compile error messages. A first-class specification, because it
feeds directly into an AI's iteration count.

Related: [`rust-type-model.md`](./rust-type-model.md),
[`evaluation.md`](./evaluation.md),
[`unverified-boundaries.md`](./unverified-boundaries.md).

> The contents of this file reflect results obtained by actually compiling on
> `rustc 1.99.0-nightly`.

---

## Why this is treated as a specification

Verum is designed so that wrong code an AI writes is rejected at compile time. It
follows that **the thing an AI reads most often is the error message.**

```text
The contract violation was detected
        ↓
but the AI cannot understand the cause
        ↓
iteration count rises
        ↓
AI coding performance ends up worse than with a conventional framework
```

**The quality of the error messages is a design subject as important as the
strength of the type system.**

---

## Three defence layers — where the error is raised

The precision of an error is determined by **which layer detects it.** The higher
the layer, the more precise.

| Layer | What it catches | Span precision |
|---|---|---|
| **1. proc macro (at expansion)** | `pub` fields, duplicate elements, a GET with mutates. **A nonexistent field or domain cannot be caught at layer 1** — see below | **Highest** (it can point at tokens inside the attribute) |
| **2. associated-type equality bound** | A `Mutates = ()` violation (a GET's read-only guarantee) | High (points at where `type Mutates` is defined) |
| **3. trait bound (`Has` / `Includes`)** | An undeclared mutation, a domain access violation | **Low** (it cannot carry a span — see below) |

### Design rule: catch it at the highest layer that can

```text
catchable in the proc macro → catch it there (precise span, a single error)
        ↓ what cannot be
expressible as an equality bound → do that (a note with a span appears)
        ↓ nor that
trait bound + on_unimplemented + do_not_recommend
```

---

## Layer 1: proc-macro errors

The most precise. **It can point at the span of the contract declaration.**

### A nonexistent field

> ### ⚠️ The `note:` below cannot be emitted (corrected in #43)
>
> `#[contract(...)]` is attached to the endpoint's unit struct. **A proc macro
> sees only the tokens of a single item** ([`rust-type-model.md`](./rust-type-model.md),
> measured), so it has no way of knowing the field list of `struct User`, a
> different item. "`User` has fields: …" is therefore **not emittable from any
> layer.**
>
> **The half that is emittable**: if the macro expands a reference to a marker
> type such as `user::Statuss`, rustc's own name resolution emits
> `help: did you mean`. So typo correction is not a layer-1 feature but **a rustc
> feature**, and the quality of `did you mean` is not under the macro's control.

```text
error[E0412]: cannot find type `Statuss` in module `user`
  --> src/endpoints/user.rs:18:32
   |
18 |     mutates   = [User::name, User::statuss],
   |                              ^^^^^^^^^^^^^ help: a struct with a similar name exists: `Status`
```

### Rejecting a `pub` field

```text
error: Domain fields must be private
  --> src/domain/user.rs:4:5
   |
 4 |     pub email: Email,
   |     ^^^ remove `pub` — access is granted through the contract
   |
   = note: `#[derive(Domain)]` generates capability-checked accessors
   = help: if this field must be public, it does not belong in a Domain
```

A `pub` field is the one route that voids the entire contract, so the macro
always rejects it ([`mutation-contract.md`](./mutation-contract.md)).

### A GET that declares mutations

```text
error: GET endpoint `GetUser` cannot declare mutations
  --> src/endpoints/user.rs:16:5
   |
16 |     mutates = [User::name],
   |     ^^^^^^^^^^^^^^^^^^^^^^ GET endpoints are read-only by construction
   |
   = help: use PUT / PATCH / POST / DELETE, or remove this declaration
```

Type checking (layer 2) catches it too, but the macro's error is more precise, so
**both are implemented.**

A `mutates` inside a `when` block is **catchable only by the macro** (to avoid a
recursive fold over `Conditional` — [`rust-type-model.md`](./rust-type-model.md)).

```text
error: GET endpoint `GetUser` cannot declare mutations
  --> src/endpoints/user.rs:18:9
   |
18 |         mutates = [User::status],
   |         ^^^^^^^^^^^^^^^^^^^^^^^^ inside `when(...)` on a GET endpoint
   |
   = note: read-only methods are GET and HEAD
```

### The same field declared both unconditionally and conditionally

```text
error: `User::email` is declared both unconditionally and under `when(EmailChanged)`
  --> src/endpoints/user.rs:12:28
   |
12 |     mutates = [User::name, User::email],
   |                            ^^^^^^^^^^^^ declared unconditionally here
...
17 |         mutates = [User::email],
   |                    ^^^^^^^^^^^^ and conditionally here
   |
   = help: remove one of them — a field is either unconditional or conditional
```

Without the macro rejecting it, the duplicate survives `Append`, `Has`'s index
inference breaks, and the result is an unrelated E0283 (type annotations needed).

### A contradiction between `mutates` and `forbidden`

```text
error: `User::status` is declared both in `mutates` and `forbidden`
  --> src/endpoints/user.rs:18:18
   |
17 |     mutates   = [User::name, User::status],
   |                              ^^^^^^^^^^^^ declared mutable here
18 |     forbidden = [User::status],
   |                  ^^^^^^^^^^^^ and forbidden here
   |
   = help: remove one of them
```

This is the only thing `forbidden` checks — it is a recorder of intent, not type
enforcement ([`mutation-contract.md`](./mutation-contract.md)).

### A duplicate element

The index-parameter approach assumes an element appears exactly once; a duplicate
produces an unrelated E0283 (type annotations needed). The macro rejects it.

```text
error: duplicate mutation `User::email`
  --> src/endpoints/user.rs:18:30
   |
18 |     mutates = [User::email, User::email],
   |                             ^^^^^^^^^^^^ already declared here
```

---

## Layer 2: the associated-type equality bound

A `Mutates = ()` violation **produces a note with a span** (verified).

```text
error[E0271]: type mismatch resolving `<GetUser as Endpoint>::Mutates == ()`
   |
note: expected this to be `()`
   |     type Mutates = (Mutate<User, user::Email>, ());
   |                    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   = note: expected unit type `()` found tuple `(Mutate<User, user::Email>, ())`
```

The `note:` points at the span of the derive-generated `type Mutates`. **If the
derive generates that type with the token spans from the contract attribute, the
note points at the contract declaration instead.**

This is the only route layer 3 cannot provide, so anything expressible as an
equality bound is deliberately put in that form.

---

## Layer 3: trait bounds — the reachable shape

### The target

```text
error[E0277]: undeclared mutation `User::status`
  --> src/endpoints/user.rs:42:21
   |
42 |         ctx.users().set_status(&mut user, UserStatus::Suspended)?;
   |                     ^^^^^^^^^^ not declared in this endpoint's contract
   |
   = note: `UpdateUser` declares mutates = [User::name, User::email]
   = help: add `User::status` to the contract, or remove this call
```

### How it is achieved

**(a) `#[diagnostic::on_unimplemented]` (1.78+)**

```rust,ignore   // needs a macro that arrives in M2
#[diagnostic::on_unimplemented(
    message = "undeclared mutation `{Domain}::{Field}`",
    label = "not declared in this endpoint's contract",
    note = "add it to #[contract(mutates = [...])] or remove this call"
)]
pub trait CanMutate<Domain, Field> {}
```

The `{Domain}` / `{Field}` placeholders expand correctly from the type parameter
names (verified).

> But **the path qualification is dropped**. `{Field}` becomes `Email`, not
> `user::Email`. That is an intermediate form matching neither the contract's
> spelling (`User::email`, lowercase) nor the marker path, so the message needs a
> string derived from `Field::NAME` embedded in it.

**(b) `#[diagnostic::do_not_recommend]` (1.85+) — required**

`on_unimplemented` controls only the message, label and note; **the chain of help
and note lines below it remains.** A naive implementation runs to about 20 lines
and exposes the cons list and the `There<There<...>>` index types wholesale.

```text
help: the following other types implement trait `Has<T, I>`
   | impl<H, T> Has<H, Here> for (H, T) {}
   | impl<H, X, T, I> Has<H, There<I>> for (X, T) where T: Has<H, I> {}
note: required for `(Mutate<User, Email>, ())` to implement `Has<Mutate<User, Status>, There<_>>`
   = note: 1 redundant requirement hidden
   = note: required for `(Mutate<User, Name>, (Mutate<User, Email>, ()))` to implement `Has<..., There<There<_>>>`
```

Attaching `#[diagnostic::do_not_recommend]` to the recursive impls takes 20 lines
down to 10, and **the failing type is shown as the actual contract tuple rather
than as `()` (the tail)** (verified).

```rust,ignore   // verum-internal: legal only inside the crate that owns the trait or type
#[diagnostic::do_not_recommend]
impl<H, T> Has<H, Here> for (H, T) {}

#[diagnostic::do_not_recommend]
impl<H, X, T, I> Has<H, There<I>> for (X, T) where T: Has<H, I> {}
```

**(c) The where clause goes on the method**

On the impl it becomes E0599 and `on_unimplemented` is **ignored** (verified).

```text
// ❌ where on the impl
error[E0599]: the method `orders` exists for struct `Ctx<UpdateUser>`,
              but its trait bounds were not satisfied

// ✅ where on the method
error[E0277]: `Order` is not in this endpoint's domain contract
```

Fixed in the derive's generated template.

---

## What cannot be reached

### A `note:` pointing at the contract declaration is not emittable through a trait bound

```text
note: `UpdateUser` declares mutates = [User::name, User::email]
  --> src/endpoints/user.rs:18:5      ← this line number does not appear
18 |     mutates   = [User::name, User::email],
```

An `on_unimplemented` note is **plain text only and carries no span.** The span
rustc emits is the location of `Has`'s impl definition, not the contract
attribute.

> **Correction**: [`semantic-endpoint.md`](./semantic-endpoint.md) originally gave
> this as the main reason for choosing the attribute approach. **It does not hold
> for trait-bound violations.** The attribute approach's advantage is the precision
> of the errors catchable at layer 1 (the macro).

### Rustc-native diagnostics Verum cannot reword

Each was parked in its own domain spec; they belong in one list, because the
shared property is what matters — **`on_unimplemented` never runs**, so no wording
Verum writes reaches the user. (This opened "Three so far" while the table held
five; the count is dropped rather than maintained.)

| Error | Where it fires | Recorded in |
|---|---|---|
| `E0615` / `E0609` | a field access on an opaque domain (T-M1-01 / #13) | [`persistence.md`](./persistence.md) |
| `E0521` | a capability borrowed across `tokio::spawn` (T-M1-02 / #14) | [`capability-system.md`](./capability-system.md) |
| `implementation of AsyncFnOnce is not general enough` | a higher-ranked `Ctx` in an `Fn`-trait position (T-M1-02 / #14) | [`conditional-effects.md`](./conditional-effects.md) |
| `E0282` — type annotations needed | `\|ctx\| async move { .. }` where the returned future borrows the argument; `async \|ctx\| { .. }` compiles (T-M1-07 / #37) | [`handler-rules.md`](./handler-rules.md) Rule 4 |
| `E0407` — method is not a member of trait | a helper placed beside `handle` in the observed `impl` block (T-M1-07 / #37) | [`unverified-boundaries.md`](./unverified-boundaries.md) path 22 |
| `E0624` / `E0603` — associated function / struct is private | building a domain from its `Repr` outside the derive-owned module (#33 / [ADR-0010](../adr/0010-domain-constructor-confined-by-module-privacy.md)) | [`persistence.md`](./persistence.md), [`unverified-boundaries.md`](./unverified-boundaries.md) path 21 |

**Two of these have no trait-bound alternative.** The pattern above — "name the
bound that *would* let Verum own the wording" — does not apply to `E0407`: "put
the helper somewhere else" is not expressible as a bound. Worse, the move
`E0407` correctly recommends is the one that makes the effect invisible to the
scan, and no wording Verum controls can say so.

**`E0624` is a third category, and the sharpest one: the trait-bound alternative
exists, produces good wording, and does not hold.** #33 compiled it — a bound on
the constructor yields `E0277` carrying Verum's `message` and `note` verbatim
(probe P37). It is still rejected, because the guarding trait is implementable by
the user from any crate (P24 / P25), so the wording is *reachable and worthless*.
Naming the bound is therefore **not** always the escape this section's pattern
suggests; check that the bound can actually be closed before proposing it.

**Two further properties of `E0624` are worse than the rows above, and are open
risks rather than recorded costs:**

1. `rustc --explain E0624` names both bypasses by number — *"1. Only use the item
   in the scope it has been defined"* and *"2. Make the item public"*. The first is
   the residue ADR-0010's option D left open; under the chosen option the module is
   derive-owned, so it is unavailable to the user, but the text still points that way.
2. The error emits **no pointer to the generated repository**, and its only
   navigational span is into the generated module. So the checked alternative
   ARK-002 requires exists but is **not discoverable from the failure**.

This is also the one row that violates the "both directions" rule below. It is
recorded as a knowing exception, not an oversight: there is no attribute that
attaches to a path-resolution diagnostic.

The third is the worst of them: it names no type the user wrote, and #14
promoted it to a first-class footgun. `persistence.md` handles its case best —
it names the trait-bound alternative that *would* let Verum own the wording, and
that is the pattern to follow when adding a row — with the caveat `E0624` adds
above, that the alternative must be checked for enforceability, not just for
existence.

### The spawn boundary is none of the three defence layers

`tokio::spawn` carrying a capability out of the request is caught by **`E0521`,
a rustc lifetime error** — not by the macro, not by an equality bound, not by a
Verum trait bound. The layer table does not cover it, and reading the table as if
it did is how the boundary gets described as "designed" when what holds it is
`Ctx<'req, E>` not being `'static`. T-M1-02 measured the limit of that: **`+ Send`
on a returned future is not a containment bound**, so a synchronous body can
build and leak whatever it likes outside any `.await`.

### The alternative

**The derive generates a dedicated trait per endpoint and embeds the declaration
in the note as a string literal.**

```rust
// derive-generated
#[diagnostic::on_unimplemented(
    note = "`UpdateUser` declares mutates = [User::name, User::email]"
)]
trait UpdateUserCanMutate<F> {}
```

The line number does not appear, but the content does. **For an AI that is likely
enough** — an AI needs "what is declared right now" more than it needs a line
number.

---

## Design rules

| Rule | Why |
|---|---|
| Catch at layer 1 (the macro) whatever layer 1 can catch | The span is precise and the error stays a single one |
| Put anything expressible as an equality bound in that form | The only route that produces a note with a span |
| Always attach `on_unimplemented` to a trait with type parameters | Do not expose raw trait-resolution errors |
| Always attach `do_not_recommend` to a recursive impl | Suppresses exposure of the cons list and the index types |
| A **help or note** always shows both directions (widen the contract / fix the implementation) | With only one, an AI relaxes the contract mechanically. **`help` is unreachable from layer 3** — see below — so the rule is satisfied by whichever of the two rustc will emit. **Exception, recorded rather than silent**: rustc-native diagnostics carry neither direction, because no attribute attaches to them. `E0624` (path 21) is the current instance |
| The where clause goes on the method | On the impl, `on_unimplemented` is ignored |
| One error, one cause | Avoids the tuple-type expansion cascading into several errors |

### `help` cannot be emitted from layer 3 at all

Measured on 1.85.0 (#15). `#[diagnostic::on_unimplemented]` rejects a `help` key:

```text
warning: malformed `on_unimplemented` attribute
  |     help = "widen the contract, or remove the call"
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ invalid option found here
  = help: only `message`, `note` and `label` are allowed as options
```

So for **every trait-bound error**, the `help:` line belongs to rustc — usually
`the trait ... is not implemented for ...` — and anything Verum writes lands in a
`note:`. The rule above is worded for that. An earlier draft of this document,
and T-M4-05's completion criterion, both required a two-directional *`help`*,
which no layer-3 diagnostic can satisfy; both are corrected rather than left as
a criterion nothing can meet.

Layers 1 and 2 are unaffected — a macro emits whatever it likes, and an equality
bound produces a note with a span.

### The limit of "a help shows both directions"

This is a wording-level countermeasure and **cannot constrain the AI's choice
itself.** There is always a third option available to it — do it in the service
layer, use raw SQL, route it through an event.

Types do not solve it, so operational measures are needed, such as detecting
contract-widening diffs in CI. See
[`unverified-boundaries.md`](./unverified-boundaries.md).

---

## How it is verified

Error messages are a specification, so they are pinned by tests.

```text
tests/ui/undeclared_mutation.rs
tests/ui/undeclared_mutation.stderr   ← the full expected error
```

UI tests with `trybuild` are the standard, and are **introduced from the First
PoC.** Verifying the type design and the error design at the same time is what
keeps the later cost of tidying wording down.

> **An operational risk**: error text containing `There<There<...>>` or a cons
> list shifts easily between rustc versions. On top of suppressing exposure with
> `do_not_recommend`, a mechanism for excluding the volatile parts from the tests
> (normalisation) may become necessary.

---

## Open problems

- Whether the derive can generate type aliases to shorten the type names in
  errors
- The error design for firing a conditional effect outside the `when` scope
  (nested projections get exposed)
- The error design for field access on a projection type
- Dealing with the unrelated "no method named `users`" error caused by forgetting
  to `use` an extension trait (auto-generating the prelude / `pub use`)

See [`research-questions.md`](./research-questions.md).
