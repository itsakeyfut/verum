# Read contract

Enforcing the `reads` declaration in types. The mirror image of
[`mutation-contract.md`](./mutation-contract.md).

Related: [`mutation-contract.md`](./mutation-contract.md),
[`unverified-boundaries.md`](./unverified-boundaries.md),
[`ai-context.md`](./ai-context.md).

---

## The problem

Declaring `reads` achieves **nothing in practice** if the repository hands back
the whole domain model.

```rust,ignore   // needs a macro that arrives in M2
#[contract(reads = [User::id, User::name, User::email, User::status])]
pub struct GetUser;

let user = ctx.users().find(req.id).await?;
user.password()   // ← undeclared, and readable anyway
```

In that state `reads` is only metadata — the "specification that is no better
than a comment" [`../concepts.md`](../concepts.md) rejects.

---

## Decision: enforce it with a projection type

`find()` returns not the whole domain model but **a projection that can only read
the declared fields.**

```rust,ignore   // fragment, not a complete item
let user = ctx.users().find(req.id).await?;
// type: Projection<User, (user::Id, (user::Name, (user::Email, (user::Status, ()))))>

user.name()       // ✅ fine
```

```rust,compile_fail
user.password()   // ❌ type error
```

A projection's getters are implemented as per-field methods with `where` clauses
inside an extension trait (confirmed by compiling).

```rust,ignore   // fragment, not a complete item
pub trait UserProjection<F> {
    fn name<I>(&self) -> &String where F: Has<user::Name, I>;
    fn password<I>(&self) -> &PasswordHash where F: Has<user::Password, I>;
}

impl<F> UserProjection<F> for Projection<User, F> { ... }
```

> `Projection` is a framework type, so an inherent impl cannot be written
> (E0116). The extension trait is required. See
> [`rust-type-model.md`](./rust-type-model.md).

---

## What this buys

### 1. The whole contract becomes trustworthy

A state where `mutates` is enforced by types and `reads` is not damages the
contract's credibility in part, and an AI cannot tell **which declarations are
real**.

### 2. Reading of personal data can be limited in types

```rust,ignore   // needs a macro that arrives in M2
#[contract(reads = [User::id, User::name])]   // password and email are out of reach
pub struct GetUserPublicProfile;
```

> **This is not a data-minimisation guarantee.** A projection is **a mask at
> compile time, not a mask on the data.** Until SELECT-clause generation is
> implemented, `find()` is equivalent to `SELECT *` and the password hash is in
> memory.
>
> Therefore:
> - Do not derive `Debug` or `Serialize` on a `Projection`. **The line that used
>   to stand here — "the derive emits an implementation printing declared fields
>   only" — is wrong.** A derive sees tokens, and `F` is a type parameter: at the
>   point the impl is written nothing can enumerate the set. Measured as #15's P3.
>   A `Projection`'s `Debug` can print the type name and no more.
> - Forbid `Deserialize` on a domain, to prevent constructing one from arbitrary
>   values.
> - **Do not claim** mechanical backing for GDPR-style data minimisation until
>   SELECT-clause generation exists.

### 3. It can drive SELECT-clause optimisation

Because the declared fields are known, the generated repository implementation
can emit `SELECT id, name FROM users`.

---

## The complexity trade-off

| Cost | Detail | Mitigation |
|---|---|---|
| Field access becomes a method | `user.name()` rather than `user.name` | Consistent with the domain, which is opaque anyway ([`mutation-contract.md`](./mutation-contract.md)) |
| Response conversion gets fiddly | `UserView::from(user)` receives a projection | `#[derive(View)]` generates the conversion |
| Types get long | The cons list is spelled out | The derive generates type aliases |
| It changes setter signatures | `set_email` comes to take `&mut Projection<User, F>` | **Recorded explicitly as Full PoC work** (below) |

### `into_owned()` is not provided

It was originally listed as a mitigation for interoperating with existing code —
extracting a bare `User` out of a projection — and has been **removed.**

The reason: the read constraint disappears the moment it is extracted. And while
the text said "it is recorded in the contract", **there is no mechanism by which
a method call records itself into an attribute macro.**

**Putting an escape hatch exactly where the designer admits the design hurts most
turns that hatch into the main route rather than the exception.** If it becomes
genuinely necessary, require a ZST proof produced by an attribute macro as an
argument, so the recording cannot be skipped.

```rust,ignore   // fragment, not a complete item
fn into_owned(self, proof: EscapeHatchProof) -> User;   // uncallable without the attribute
```

### Interaction with mutation

A field declared in `mutates` needs its previous value read, so it is
**automatically included in `reads`.**

```rust,ignore   // needs a macro that arrives in M2
#[contract(
    reads   = [User::id, User::status],
    mutates = [User::name],
    when(EmailChanged) => {
        mutates = [User::email],
    },
)]
// effective read set: id, status, name, email
```

**A `mutates` inside `when` is included in `reads` too**, since the previous
value has to be read when the condition holds. Read permission is valid
regardless of scope: constraining reads to the condition would force a second
`find` inside the `when` block, which costs more than it is worth.

When projections arrive, setter signatures change from `&mut User` to
`&mut Projection<User, F>`. The signature differing between the First PoC (no
projections) and the Full PoC is stated here deliberately.

---

## Treatment in the PoC

**Projections are not implemented in the First PoC — and #15 measured that they
are *not* redundant either.**

> ### `reads` is enforced by the getter's bound — in one shape, conditionally
>
> T-M1-03 (`spikes/reads-getter-enforcement/`, 11 probes on rustc 1.85.0, two
> crates, against `verum`'s real `Has`): `where Self::Set: Has<Read<D, F>, I>` on a
> getter rejects an undeclared read with `E0277`. `find()` can keep returning the
> plain opaque `Domain`. Decision in
> [ADR-0004](../adr/0004-reads-enforcement-level.md), which stays `proposed`.
>
> **The getter lives in a derive-emitted extension trait, not on the repository.**
> An inherent `impl Repo<'_, Domain, ..>` is `E0116` in the real layering, where the
> framework owns `Repo` and the user's crate owns the `Domain`. So
> `user.email()` becomes `ctx.users().email(&user)`, which is symmetric with the
> setters and consistent with [`handler-rules.md`](./handler-rules.md) Rule 2. The
> Domain-side shape is `E0283`; naming `R` at the call site compiles and still
> enforces, but it is not `user.email()`.
>
> **Two preconditions are undesigned, and both void the guarantee.** The trait must
> not take `R` as a type parameter — written `UserRead<R>` a downstream crate
> forges a wider read set in one line, and coherence does not object. And `Repo`
> must be unreachable except through `Ctx`: the bound constrains `R`, not who
> supplies it, so a public constructor lets the caller choose its own read set.
> Both are measured; neither is closed.
>
> **Scope: `handle_via_ctx`, the same as `mutates`.** A `Domain`'s `Debug` and any
> free function taking `&Domain` read every field with no capability, and no getter
> shape reaches them
> ([`unverified-boundaries.md`](./unverified-boundaries.md)).
>
> **What a projection still buys, measured:** its derived `Debug` prints the
> declared fields and nothing else. The claim above — that a projection's derive
> "emits an implementation printing declared fields only" — was briefly deleted
> from this document as unachievable, on the grounds that a derive sees tokens and
> `F` is a type parameter. That reasoning is wrong and P4 is the counter-example:
> the derive emits one impl **per field of the Domain**, which it can see, and one
> fixed recursive walk resolves `F` at monomorphisation. A `Domain`-side derive
> cannot do this, because the `Domain` carries no read set. This is the one axis on
> which projections are strictly stronger than getters.

Reasons projections are still not built:

- The First PoC proves one thing: that a GET cannot call a mutation.
- Their remaining value is narrowing the `SELECT` clause — codegen, addressable on
  its own — and narrowing a derived `Debug`/`Serialize` to the declared set, which
  #15 measured and which nothing else in the design provides. Whether that is worth
  the five costs above is #18's call.

### Do not hide the gap between stages

The AI Context states that `reads` is metadata only for now.

> **The getters do enforce — and the level still does not move.** #15 measured
> that reading an undeclared field through a capability-checked getter is a compile
> error. What has not happened is the *implementation*: `crates/verum` has no
> derive, no `Repo` and no getters, so emitting `upper_bound_checked` would claim
> enforcement no code provides. Nor are the two preconditions closed — an
> unparameterised extension trait, and a `Repo` reachable only through `Ctx`. The
> reason for `metadata_only` changed from "unmeasured" to "unimplemented, and two
> preconditions open"; it promotes when M2's derive lands and both are closed, and
> that promotion is a breaking change. Account in
> [ADR-0004](../adr/0004-reads-enforcement-level.md).

```json
{
  "reads": {
    "fields": ["User.id", "User.name", "User.email", "User.status"],
    "enforcement": { "level": "metadata_only", "scope": "none", "voided_by": "not_applicable" }
  },
  "mutates": {
    "fields": ["User.name", "User.email"],
    "enforcement": {
      "level": "upper_bound_checked",
      "scope": "handle_via_ctx",
      "voided_by": [
        "domain_repr", "domain_swap", "repository_impl", "unscanned_effect",
        "middleware", "constructor_body", "malformed_set",
        "upsert_granularity", "event_subscriber"
      ]
    }
  }
}
```

`reads` is promoted to `upper_bound_checked` in the Full PoC.

> **`reads` is upper-bound-**only**, permanently** — #42 /
> [ADR-0014](../adr/0014-syntactically-present-replaces-observed.md). Generation
> recovers what is spelled `ctx.<accessor>()`; a read is `user.name()` or
> `UserView::from(user)` and goes nowhere near `ctx`. So there is no mechanism that
> could give `reads` a lower bound, now or after the two causes #37 measured are
> fixed. The promotion above raises the ceiling; nothing raises a floor.

> The value `type_checked` is never used. A contract is an upper-bound check —
> implementation ⊆ contract — not a bidirectional verification. See
> [`effect-inference.md`](./effect-inference.md).

---

## Open problems

- **Listing.** Only the `find(id) -> Projection<User, F>` shape is defined. A
  listing API returning `Vec<Projection<..>>`, and how pagination, sorting and
  dynamic filtering are expressed there.
- **Aggregation.** COUNT, SUM and GROUP BY are not the value of a particular
  field, and the result belongs to no domain instance.
- **JOIN.** `Projection<Domain, Fields>` covers one domain. A composite
  projection (`Projection<(User, Order), (..)>`) is undefined.
- **N+1 and eager loading.** These collide structurally with per-field methods
  (Rule 1).
- Whether domain opacity's getters alone suffice to enforce `reads`, which would
  make the projection type unnecessary —
  [ADR-0004](../adr/0004-reads-enforcement-level.md).

See [`research-questions.md`](./research-questions.md).
