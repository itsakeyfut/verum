# Effect system

An effect system with semantics extended for web and API work. The
specification of declaration granularity and the split by category.

Related: [`capability-system.md`](./capability-system.md),
[`conditional-effects.md`](./conditional-effects.md),
[`rust-type-model.md`](./rust-type-model.md).

---

## The principle

> Do not collapse "side effect" into one giant `IO`.
>
> **Decompose effects down to a granularity an AI can act on.**

---

## Classification

Effects fall into three families, and **where they are declared and what
vocabulary they use differs per family.**

### State effects — `reads` / `mutates` / `creates` / `deletes`

```text
Read<User, user::Name>
Mutate<User, user::Email>
Create<AuditLog>
Delete<Session>
```

### External effects — `emits` / `calls`

```text
Emit<UserUpdated>
Call<EmailService>
Call<PaymentService>
```

### Infrastructure effects — `effects` (deltas only)

```text
DatabaseRead
DatabaseMutation
CacheRead
CacheWrite
Metrics
Logging
Tracing
FileRead
FileWrite
Network
Time
Spawn<Job>
```

> **The vocabulary is closed.** No infrastructure effect name outside that list
> may be used.
>
> `SendEmail` / `MessagePublish` / `ExternalMutation` in particular are **not
> infrastructure effects.** They are external effects, expressed as
> `calls = [EmailService]` / `emits = [X]`. Two declaration routes for the same
> side effect leave an AI unable to decide which to write, so it writes both or
> drops one.

---

## Decision 1: declare effects split by category

Rather than a unified `effects = [...]`, there is an attribute key per category.

```rust,ignore   // needs a macro that arrives in M2
#[contract(
    reads   = [User::id, User::status],
    mutates = [User::name, User::email],
    creates = [AuditLog],
    emits   = [UserUpdated],
)]
```

The derive expands them into per-category associated types (in cons-list form).

```rust
// A field in `mutates` is automatically included in `reads`
// ([`read-contract.md`](./read-contract.md)). The declaration names two —
// reads=[id, status] — but after expansion name and email join them, making four.
type Reads   = (Read<User, user::Id>,
               (Read<User, user::Status>,
               (Read<User, user::Name>,
               (Read<User, user::Email>, ()))));
type Mutates = (Mutate<User, user::Name>, (Mutate<User, user::Email>, ()));
type Creates = (Create<AuditLog>, ());
type Deletes = ();
type Emits   = (Emit<UserUpdated>, ());
type Calls   = ();
```

> Why a cons list (`(A, (B, ()))`): with flat tuples `(A, B)` the membership impls
> violate coherence. See [`rust-type-model.md`](./rust-type-model.md).

### Why

For deciding a single effect's membership, the unified and per-category designs
have **exactly the same enforcement power.** The differences are these.

#### (a) Stating an absence — the decisive difference

A GET's read-only guarantee is the statement "it **declares** no mutation at all" (path 28: the *contents* of an interior-mutable field type are outside it).

```rust
trait ReadOnly: Endpoint<Mutates = (), Creates = (), Deletes = ()> {}
```

An associated-**type** equality bound is stable, and the error is clear.

```text
expected unit type `()` found tuple `(Mutate<User, user::Email>, ())`
```

The unified design would have to require "contains no Mutate", and Rust has **no
negative trait bounds** (`!Trait` is unstable, and a wildcard over a type
parameter cannot be written). A fold over type-level booleans is an alternative,
but it degrades the error into a form that does not say **which element caused
it.**

#### (b) Handing effects to a repository

Split by category, the already-classified type can be passed straight through.
The unified design needs a type-level `Filter` to extract only the mutations from
`Effects`, and **the catch-all impl always collides**
([`rust-type-model.md`](./rust-type-model.md)).

#### (c) Trait-resolution cost

`Has<Set, Elem, Idx>` is linear in the element count. Split by category it scans
a short cons list (3–4 elements); unified it scans every effect (10–15).

#### (d) Ease of writing for an AI

Per-category key names make it **a structured fill-in-the-blanks.** The unified
form is closer to free text, and things get left out.

### How GET ⇒ ReadOnly is enforced

`impl<E: Endpoint<Method = Get>> ReadOnly for E {}` **cannot be written.**
`ReadOnly` has `Mutates = ()` as a supertrait, so the blanket impl must require
it too, at which point it enforces nothing about `Method` (confirmed by
compiling).

It is enforced by a compile-time assertion the derive generates.

```rust
const _: () = {
    fn assert_readonly<E: Endpoint<Method = Get> + ReadOnly>() {}
    fn check() { assert_readonly::<GetUser>(); }
};
```

On top of that, the proc macro rejects "a GET with mutates/creates/deletes" at
expansion time, which gives the most precise error. Detail in
[`rust-type-model.md`](./rust-type-model.md).

### A unified view (later)

For cross-cutting uses, the derive can also generate the concatenation of every
category. Users do not write it. **Not needed in the First PoC.**

---

## Decision 2: infrastructure effects are "a per-method default plus a delta"

### The framework's defaults

```text
GET / HEAD  → DatabaseRead, CacheRead, Logging, Metrics, Tracing
POST        → the above + DatabaseMutation
PUT / PATCH → the above + DatabaseMutation
DELETE      → the above + DatabaseMutation
```

### An endpoint declares only its deviations

```rust,ignore   // needs a macro that arrives in M2
#[contract(
    mutates = [User::email],
    effects = [+CacheWrite],     // added
)]

#[contract(
    reads   = [User::id],
    effects = [-CacheRead],      // explicitly forbidden
)]
```

### Why

| Option | Assessment |
|---|---|
| Declare every effect | 8–10 lines of boilerplate per endpoint. Token efficiency suffers, and **forgetting to declare is indistinguishable from deliberately not declaring** |
| No infrastructure declaration | The contract reads well, but "does this endpoint write to the cache" disappears, and an endpoint that deliberately does not log cannot be expressed |
| **Default plus delta** | **Short to write, complete to read** |

### An important limit: infrastructure effects are not enforced by types

The per-method default table is **a documentation table with zero type
checking.** Calling `ctx.cache()` without writing `effects = [+CacheWrite]` is not
stopped by the current design.

So the AI Context states `level: "none"` explicitly, and does **not** call the
expanded list `effective` — nothing observed it, and `effective` means "the fully
expanded declaration" everywhere else in the output
([`ai-context.md`](./ai-context.md) §2).

```json
"effects": {
  "declared_delta": ["+CacheWrite"],
  "assumed_from_method": ["DatabaseRead", "DatabaseMutation", "CacheRead", "CacheWrite", "Logging", "Metrics", "Tracing"],
  "derived_from": "method_default_table",
  "enforcement": { "level": "none", "scope": "none", "voided_by": "not_applicable" }
}
```

**Not hiding the difference in enforcement level is the condition on which this
design is adopted.** See [`ai-context.md`](./ai-context.md).

> This axis has the lowest enforcement per concept of any contract item. A
> decision is needed later: enforce it in `ctx.cache()`'s where clause, or drop the
> axis. Recorded in [`research-questions.md`](./research-questions.md).

### On separating the writing side from the reading side

The contract in the source is a delta and the AI Context is the complete form,
which reconciles token efficiency with explicit effects. But this is a decision
to **accept that the source alone does not carry the complete meaning.**

In `../concepts.md`'s trust ordering (type/contract → … → generated
documentation), **the complete form intended for an AI sits on the generated
side.** That tension has to be acknowledged, and freshness of the generated output
— regeneration in CI plus a zero-diff check — has to be part of the
specification.

---

## GET and the immutability guarantee

For a GET endpoint,

> a GET is always immutable

is guaranteed at the type level. But "immutable" is defined carefully: a GET can
still cause Logging, Metrics, Tracing, CacheRead and CacheWrite.

```text
GET User

Allowed:
    DatabaseRead / CacheRead / Metrics / Logging / Tracing

Forbidden:
    DatabaseMutation / MessagePublish (= emits) / ExternalMutation (= calls) / FileWrite
```

### An important restatement

Not

```text
a GET has no side effects
```

but

```text
a GET endpoint has no mutation capability
```

### The guarantee's scope is the handler

**Unless middleware carries a contract, this guarantee does not hold at request
scope.**

```text
If auth middleware updates last_login_at:
  handler scope : Mutates = () → read-only (true)
  request scope : User.last_login_at is updated (false)
```

Stated in the AI Context per key rather than once globally: `mutates`, `creates`
and `deletes` each carry `enforcement.scope: "handle_via_ctx"` and list
`middleware` under `voided_by`. When middleware contracts arrive, `middleware`
leaves `voided_by` and the scope widens. See
[`unverified-boundaries.md`](./unverified-boundaries.md).

> **And it is narrower than the handler scope too** (ledger **path 28**, #44).
> `Mutates = ()` is enforced through the *setter's* where clause, so a mutation that
> never calls a setter is outside it: an interior-mutable field type is written
> through `&self`, which a GET has, and a foreign crate does it to a correctly
> loaded Domain with no `&mut` and no capability (run-verified). `read-only (true)`
> in the block above is therefore true of the **declared effect set** and not of the
> Domain's contents. This file owns the guarantee
> ([`README.md`](./README.md)), so it is stated here and not only in the ledger —
> #44's first version narrowed the ledger's twin section and left this one
> unqualified.

> **There is no `scope_of_readonly_guarantee` key.** It said what those three keys
> now say, and it overstated in exactly the way this section warns against — a GET
> still causes Logging, Metrics, Tracing, CacheRead and CacheWrite, so the
> guarantee was never "read-only" but "no state mutation".
> [ADR-0008](../adr/0008-guarantees-carry-scope-and-voiding-paths.md).

---

## Type-level constraints

| Operation | Allowed? |
|---|---|
| `Has<Set, Elem, Idx>` — membership of a single element | Safe (linear) |
| `Append<A, B>` — concatenating cons lists | Safe |
| `Lookup<Set, Key, Idx>` — a type-level map lookup | Safe |
| `Subset<A, B>` / `Filter<Set, Pred>` | **Avoid** |
| Negative reasoning | **Impossible** |

Detail in [`rust-type-model.md`](./rust-type-model.md).
