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
> - Do not derive `Debug` or `Serialize` on a `Projection` — the derive emits an
>   implementation printing declared fields only.
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

**Projections are not implemented in the First PoC.**

Reasons:

- The First PoC proves one thing: that a GET cannot call a mutation.
- Projections are faster to build symmetrically, once the mutation contract's
  type design has settled.
- Domain opacity alone — private fields plus capability-checked getters — may
  already restrict **reading** an undeclared field. Whether checking `Reads` in a
  getter's where clause achieves the same effect without a projection type needs
  measuring.

### Do not hide the gap between stages

The AI Context states that `reads` is metadata only for now.

> **Whether capability-checked getters amount to enforcing `reads` has not been
> measured.** That they are generated is settled; whether they turn reading an
> undeclared field into a compile error is the subject of #15 / T-M1-03, which
> has not run. Emitting `metadata_only` here is **choosing the weaker claim**, not
> a finding that the getters have no effect. The account is in
> [ADR-0004](../adr/0004-reads-enforcement-level.md).

```json
{
  "reads": {
    "fields": ["User.id", "User.name", "User.email", "User.status"],
    "enforcement": "metadata_only"
  },
  "mutates": {
    "fields": ["User.name", "User.email"],
    "enforcement": "upper_bound_checked"
  }
}
```

`reads` is promoted to `upper_bound_checked` in the Full PoC.

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
