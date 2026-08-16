# Mutation contract

Expressing field-level mutability in types. A particularly important theme for
this project.

Related: [`read-contract.md`](./read-contract.md),
[`capability-system.md`](./capability-system.md),
[`handler-rules.md`](./handler-rules.md),
[`unverified-boundaries.md`](./unverified-boundaries.md).

---

## Example: updating a user

The domain model in question:

```text
User
├── id
├── name
├── email
├── password
├── status
├── last_login_at
└── created_at
```

The contract declaration:

```rust,ignore   // needs a macro that arrives in M2
#[endpoint(PUT "/users/{id}")]
#[contract(
    domain    = User,
    reads     = [User::id, User::status],
    mutates   = [User::name, User::email],
    when(EmailChanged) => {
        emits = [EmailVerificationRequested],
    },
)]
pub struct UpdateUser;
```

---

## The goal

Without reading the endpoint's body, an AI should be able to tell:

- which fields may change
- which fields never change
- under which conditions they change
- which events are emitted

---

## Decision: a domain is exposed as an opaque type

**This is the most important design decision.** Exposing a domain as an ordinary
Rust struct voids the whole contract.

```rust,compile_fail
// ❌ in this shape the contract means nothing
pub struct User { pub email: Email, pub status: UserStatus, ... }

// in the handler
let mut user = ctx.users().find(req.id).await?;
user.email = req.email;              // the contract is ignored. It compiles
user.status = UserStatus::Admin;     // an undeclared field is free too
```

Calling a setter requires the handler to hold a `&mut User`, and if the fields
are `pub` then arbitrary assignment is legal. **A GET handler can write
`let mut user = ctx.users().find(id).await?` too, so even the guarantee "a GET
cannot call a mutation" is walk-aroundable.**

### The shape adopted

```rust,ignore   // needs a macro that arrives in M2
#[derive(Domain)]
pub struct User {
    id:            UserId,      // adding pub makes the derive emit a compile error
    name:          String,
    email:         Email,
    password:      PasswordHash,
    status:        UserStatus,
    last_login_at: Option<DateTime<Utc>>,
    created_at:    DateTime<Utc>,
}
```

What the derive generates:

```text
1. Field marker types (ZSTs)         → mod user { pub struct Name; ... }
2. Capability-checked getters        → read-contract.md (**whether that amounts to enforcing `reads`
                                        is undecided** — ADR-0004 / #15)
3. A pub(crate) Repr                 → the internal representation (⚠️ it does not end up
                                        "for the repository implementation only" — path 21)
4. A Debug emitting declared fields only     → prevents secrets leaking into logs
5. A Serialize emitting declared fields only → same
```

### Passing `&mut User` becomes safe

With private fields, holding a `&mut User` does not permit direct assignment.

```rust,ignore   // fragment, not a complete item
ctx.users().set_email(&mut user, req.email)?;   // ✅ through a capability
```

```rust,compile_fail
user.email = req.email;                          // ❌ private field
```

**So the setter signatures do not have to change.** The direct-assignment route is
closed while the ergonomics are preserved.

### Why `pub` fields are rejected

The derive makes a detected `pub` field a compile error.

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

"Adding `pub` by accident" is one of the guarantee-breaking routes that **a macro
can reject**, so it is rejected at the macro stage.

> **It is not the only route** (measured in T-M1-01 / #13). The `Repr` the same
> macro generates opens a bypass alongside it (ledger path 21), and deriving
> `Debug` or `Clone` on `Repr` brings paths 3 and 4 back. This check is necessary
> but not sufficient.

### Interoperating with a repository implementation

sqlx's `query_as!` requires `pub` fields, so a `pub(crate)` `Repr` is generated.

```rust,ignore   // fragment, not a complete item
// macro-generated (derive or attribute is undecided — see the note below)
pub(crate) struct UserRepr {
    pub id: UserId,          // pub is required (query_as! expands to a struct literal at the call site)
    pub name: String,
    pub email: Email,
    // Do not derive Debug / Clone / Serialize (ledger paths 3 / 4 come back through Repr)
}

// The domain owns a borrowable Repr (a newtype is one way to do that, not a requirement).
// The inner field is private — making it pub(crate) lets `u.0.email = v` compile,
// voiding the opacity above directly.
// ⚠️ This newtype shape cannot be emitted by a derive (E0428). See the note below.
pub struct User(UserRepr);

impl User {
    pub(crate) fn from_repr(r: UserRepr) -> Self { ... }
    pub(crate) fn as_repr(&self) -> &UserRepr { ... }
}
```

> **⚠️ This shape does mesh with sqlx, but it does not express "the trust boundary
> is the repository implementation" in types** (compile-verified in T-M1-01 / #13).
>
> `#[derive(Domain)]` **expands in the user's crate**, so `pub(crate)` means the
> whole application crate. The `from_repr` / `as_repr` above are therefore
> **reachable from every handler in that crate**, and
> `User::from_repr(UserRepr { email: anything, .. })` assembles a domain from
> arbitrary values. Putting the repository in a separate crate has the opposite
> problem: `Repr` is invisible (`E0603`) and the design does not function at all.
>
> In other words **making the fields private does work**, but `Repr` has opened a
> bypass alongside it. Ledger **path 21**.
>
> **But the guarantee from privacy is "from outside the defining module", not a
> type boundary** (measured). From the defining module and its children,
> `u.0.email = v` compiles, and **the macro expands in the same module as the
> user's `struct User`**, so a helper the user writes next to it stands on the
> permissive side.
>
> The error code varies with the shape too (measured): with a newtype and an
> `email()` getter, `u.email = v` is **`E0615`**; without the getter it is `E0609`;
> only a flat private named field touched **from outside the module** gives
> `E0616`. **`E0615` and `E0609` cannot have their wording replaced with
> `#[diagnostic::…]`**, so the guidance `E0616` carries is not available.
>
> On top of that, **a derive cannot add an item with the same name as its input**,
> so the `pub struct User(UserRepr)` above is not something `#[derive(Domain)]` can
> emit (`E0428`). Several shapes **do satisfy the `as_repr(&self) -> &Repr`
> signature itself** through a derive (measured), so it is not the case that the
> signature and a derive are incompatible. Which macro shape to use is
> **undecided** (#18).
>
> The full verdict and the 21-probe table are in
> [`persistence.md`](./persistence.md) §Verdict and
> `spikes/domain-opacity-sqlx/README.md`.

---

## The type representation

### 1. Field marker types (derive-generated)

```rust,ignore   // module shown without its imports; `Field` is in scope in the real file
pub mod user {
    pub struct Id;
    pub struct Name;
    pub struct Email;

    impl Field<User> for Name {
        const NAME: &'static str = "name";
        type Ty = String;
    }
}
```

### 2. A mutation requires a capability

Repository access is provided through an extension trait (an inherent impl is
impossible under the orphan rule —
[`rust-type-model.md`](./rust-type-model.md)).

```rust
// the extension trait the derive generates per domain
pub trait UserRepo<M> {
    fn set_email<I>(&self, u: &mut User, v: Email) -> Result<()>
    where M: Has<Mutate<User, user::Email>, I>;

    fn set_name<I>(&self, u: &mut User, v: String) -> Result<()>
    where M: Has<Mutate<User, user::Name>, I>;
}
```

`M` is `E::Mutates`, the cons list expanded from the contract. An undeclared
field's setter has an unsatisfied where clause, so **calling it is a compile
error.**

> Note: `I` is the inference parameter for the membership decision. It is required
> for `Has`'s recursive impls to satisfy coherence
> ([`rust-type-model.md`](./rust-type-model.md)). The derive generates it; users
> never write it.

### 3. Calling it from a handler

```rust,ignore   // fragment, not a complete item
ctx.users().set_email(&mut user, req.email)?;
```

The capability token does not appear as an argument — `Ctx<'req, Self>` holds it.
The reasoning is in [`capability-system.md`](./capability-system.md).

---

## MustNotMutate needs no declaration

It holds naturally, because no capability is issued for that field.

```rust,compile_fail
// the contract has no User::id / User::created_at
ctx.users().set_id(&mut user, other_id)?;
//          ^^^^^^ type error (E0277): the where clause is unsatisfied
```

> **On stating this accurately**: the derive generates a setter per domain field,
> so the method `set_id` **does exist.** What is unsatisfied is the where clause,
> and the error is E0277 (not E0599, "no such method").
>
> In practice this means **rust-analyzer keeps completing `set_email` even in a
> GET handler.** The feeling of "it cannot be called at all" is not available; the
> accurate guarantee is "calling it is a compile error".

---

## The semantics of `forbidden`

> The point the Q-C experiment flagged: "`forbidden`'s specification was not in the
> cheatsheet, and there is no confirmation of what the macro actually checks".
> Fixed here as specification.

### Definition

```rust,ignore   // needs a macro that arrives in M2
#[contract(
    mutates   = [User::name, User::email],
    forbidden = [User::status, User::password_hash],
)]
```

**`forbidden` is a declarative statement of intent, and redundant as far as the
type check goes.** A field absent from `mutates` gets no capability, so the call
is a compile error whether or not it appears in `forbidden`.

The macro checks **exactly one thing.**

```text
error: `User::status` is declared both in `mutates` and `forbidden`
  --> src/endpoints/user.rs:18:5
   |
17 |     mutates   = [User::name, User::status],
   |                              ^^^^^^^^^^^^ declared mutable here
18 |     forbidden = [User::status],
   |                  ^^^^^^^^^^^^ and forbidden here
   |
   = help: remove one of them
```

An overlap with a conditional mutation (a `mutates` inside `when`) is rejected the
same way.

### Why keep something redundant — the value the experiment found

In task C of the Q-C experiment, faced with the requirement "reset status to
Unverified when the email changes", the subject **removed `User::status` from
`forbidden` and then added it to `mutates`.**

That removal **stayed in the diff.**

Under the generated-metadata approach (condition 2), the contract file was not
updated and **the same relaxation left no trace in the diff at all.**

> `forbidden` is not a means of type enforcement but **a recorder of intent.**
>
> Undoing something declared as "never changed" is left in the diff as an explicit
> deletion. That was the only substantive differentiator the Q-C experiment
> confirmed.

Detail in [`evaluation.md`](./evaluation.md).

### When to use it

Requiring a prohibition for every field is verbose, and makes forgetting
indistinguishable from intent. **The principle is "only what `mutates` declares is
possible"**, and `forbidden` is limited to:

- Fields where "never touch this" is worth recording for security
  (`password_hash` and the like)
- Business invariants (`created_at`, `id`)
- Fields a reviewer should be pointed at

### Treatment in the AI Context

That it is not type-enforced is not hidden.

```json
"forbidden": {
  "fields": ["User.status", "User.password_hash"],
  "enforcement": { "level": "intent_only", "scope": "declaration_only", "voided_by": "not_applicable" },
  "note": "Records intent. The macro checks only that no field appears in both `mutates` and `forbidden`. A field absent from `mutates` gets no capability, so calling its setter through `ctx` does not compile — but that is `mutates`' guarantee, with `mutates`' scope and `voided_by`, not this key's."
}
```

`level: "intent_only"` distinguishes it from `upper_bound_checked`, and
`scope: "declaration_only"` names the one thing that *is* checked: the
`mutates` / `forbidden` overlap, at the macro layer.

> **The note no longer says "already uncallable".** The setter exists — the derive
> generates one per field — and what fails is its where clause, E0277 rather than
> E0599 (§MustNotMutate needs no declaration above). Writing "uncallable"
> contradicted this file two sections earlier, and under ledger path 21 a handler
> can build a `User` with any `password_hash` without calling a setter at all.

---

## Conditional mutation

A field changed only under a condition is **declared inside the `when` block.**

```rust,ignore   // needs a macro that arrives in M2
#[contract(
    mutates   = [User::name],              // unconditional
    when(EmailChanged) => {
        mutates = [User::email],           // only under this condition
    },
)]
```

Writing the same field both at the top level and inside `when` is forbidden (the
macro rejects it).

The effective set of mutable fields is `mutates ∪ the mutates of every when`, and
the AI Context emits the combined, complete form.

Detail in [`conditional-effects.md`](./conditional-effects.md).

---

## Enforcing per-field methods

A repository does **not** provide a blanket `save()` or `update()`.

```rust,compile_fail
// ❌ not provided
ctx.users().save(&mut user)?;
```

Two reasons.

1. **Type checking stops working** — `save` writes back every field, so which
   capability it should require is undetermined.
2. **The implementation becomes unreadable** — even with a correct contract, which
   line changed what is unknown.

This is specified as [`handler-rules.md`](./handler-rules.md) Rule 1.

> The read side has an unresolved collision with N+1 avoidance (eager loading).
> See [`research-questions.md`](./research-questions.md).

---

## Relationship to `reads`

A field declared in `mutates` needs its previous value read, so it is
**automatically included in `reads`.**

```rust,ignore   // needs a macro that arrives in M2
#[contract(
    reads   = [User::id, User::status],
    mutates = [User::name, User::email],
)]
// effective read set: id, status, name, email
```

Detail in [`read-contract.md`](./read-contract.md).

---

## The limit of what this reaches

Type checking reaches only as far as whether a method may be called. **It does not
reach the SQL inside a repository implementation.**

```rust,compile_fail
impl UserRepository for PgUserRepository {
    async fn set_email(&self, u: &mut User, v: Email) -> Result<()> {
        sqlx::query!("UPDATE users SET email = $1, status = 'x' WHERE id = $2", ...)
        //                                        ^^^^^^^^^^^^ undeclared, and undetectable
    }
}
```

The repository implementation is the trust boundary. Detail and mitigations in
[`persistence.md`](./persistence.md).

**The routes that survive domain opacity** are listed in
[`unverified-boundaries.md`](./unverified-boundaries.md). In particular:

- `*user = other_user` (fetch two with `find` and replace wholesale)
- Row-level permissions (`Mutate<User, Email>` does not mean "this user")

---

## Open problems

- **Soft delete** — `mutates = [User::deleted_at]` and `mutates = [User::name]`
  are syntactically indistinguishable. "Semantics over syntax" fails on the most
  common CRUD pattern
- **Optimistic locking** — per-field setters cannot express
  `WHERE id=? AND version=?` compare-and-swap as an atomic operation
- **Bulk operations** — how a 100-row batch update is written with per-field
  setters
- **Listing, aggregation, JOIN** — consequences of `Read<Domain, Field>` assuming
  a single instance

See [`research-questions.md`](./research-questions.md).

---

## What must be verified

- Field-level mutation can be expressed in types
- An undeclared mutation is a compile error
- There is no route to changing a MustNotMutate field
- **Domain opacity meshes with sqlx** — ✅ **it holds** (measured in T-M1-01 /
  #13). But "the trust boundary is the repository implementation" does not (ledger
  path 21)
- **The derive can reject a `pub` field**
- The error message points at the contract declaration
  ([`diagnostics.md`](./diagnostics.md))
