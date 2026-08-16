# Handler rules

The conventions for implementing a handler. **The mechanism that keeps "reading
the implementation makes the endpoint's behaviour self-evident" true.**

Related: [`capability-system.md`](./capability-system.md),
[`unverified-boundaries.md`](./unverified-boundaries.md),
[`persistence.md`](./persistence.md).

---

## Background to the decision

After considering how a capability token should be passed around inside a
handler, **passing it through the context (implicitly)** was chosen.

```rust,ignore   // fragment, not a complete item
// chosen: through the context
ctx.users().set_email(&mut user, req.email)?;

// rejected: an explicit argument
ctx.users().set_email(&mut user, req.email, caps.mutate::<user::Email>())?;
```

The rejection is not because it is long. It is because **the information the
explicit argument adds — `user::Email` — is already obvious from the name
`set_email`**: it adds length without adding information.

That reasoning **holds only where the context route preserves self-evidence.**
Self-evidence is determined by the shape of the API, not by the presence of a
capability token. The following three rules are therefore part of the
specification.

> **If the rules are not kept, the context-passing design loses its
> self-evidence.** These three are not optional; they are preconditions of the
> capability design.

---

## Rule 1 — a repository has per-field methods only

```rust,ignore   // fragment, not a complete item
// ✅ provided
ctx.users().set_email(&mut user, v)?;   // what changes is obvious
ctx.users().set_name(&mut user, v)?;
ctx.users().set_status(&mut user, v)?;
```

```rust,compile_fail
// ❌ not provided
ctx.users().save(&mut user)?;            // what changed is unknown
ctx.users().update(&mut user, patch)?;   // ditto
ctx.users().apply(&mut user, changes)?;  // ditto
```

Allow a blanket method, and even with `mutates = [name, email]` written in the
contract, **which line changes what cannot be read off the implementation.**

### Why `&mut User` may be passed

A domain is exposed as an opaque type with private fields
([`mutation-contract.md`](./mutation-contract.md)), so holding a `&mut User` does
not permit direct assignment.

```rust,ignore   // fragment, not a complete item
ctx.users().set_email(&mut user, req.email)?;   // ✅
```

```rust,compile_fail
user.email = req.email;                          // ❌ private field
```

**With a domain that has `pub` fields this rule is meaningless.** The derive
rejecting `pub` is Rule 1's precondition.

### The exception on the read side (unresolved)

Avoiding N+1 (eager loading) collides structurally with per-field methods.
Writing "fetch 100 users and return each one's order count" as a single method
looks much like the blanket methods Rule 1 rejects.

An exception is likely to be needed on the read side alone: "a composite query is
permitted, limited to combinations of declared fields". See
[`research-questions.md`](./research-questions.md).

---

## Rule 2 — every effect-causing operation goes through `ctx`

```text
ctx.users()      → state effect (Read / Mutate / Delete)
ctx.audit_logs() → Create
ctx.events()     → Emit
ctx.email()      → external effect
ctx.cache()      → infrastructure effect
ctx.spawn()      → Spawn<Job>
```

This keeps the state where **following the lines that begin with `ctx.`
enumerates every effect that handler causes.**

### The part enforced by types

| Route | Means of enforcement |
|---|---|
| Holding a `PgPool` on the endpoint struct and running SQL directly | `#[endpoint]` rejects anything but a unit struct |
| Carrying a capability out through `tokio::spawn` | `Ctx<'req, E>` is not `'static` |
| Passing a `dyn Repository` to a service | `dyn Repository` is not exposed |

### The part that remains convention (an important limit)

Rule 2's consequence — "grep enumerates every effect" — **depends on free
associated functions being pure.**

```rust,compile_fail
ctx.audit_logs().create(AuditLog::user_updated(&user))?;
//                      ^^^^^^^^^^^^^^^^^^^^^^ hitting the DB in here is undetectable
ctx.events().emit(UserUpdated::from(&user))?;
Ok(UserView::from(user))
```

The following **must be pure** (by convention):

- Constructors of types declared in the contract (`AuditLog::user_updated` and
  the like)
- `Condition::holds`
- View conversions (`UserView::from`)

Eventually `#[derive(Event)]` / `#[derive(View)]` will generate the constructors
and remove the room for hand-writing them. Tracked as #18 in
[`unverified-boundaries.md`](./unverified-boundaries.md).

---

## Rule 3 — a conditional effect fires only inside a `ctx.when::<Cond>` scope

```rust,ignore   // fragment, not a complete item
ctx.when::<EmailChanged, _>(&mut user, &req, async |ctx, user, req| {
    ctx.users().set_email(user, req.email.clone())?;
    ctx.events().emit(EmailVerificationRequested::for_user(user))?;
    Ok(())
}).await?;
```

Only the `ctx` passed into the closure carries the capability for
`EmailVerificationRequested`. The outer `ctx` does not, so firing it outside the
scope is a type error.

### The signature

`user` and `req` are **lent as closure arguments, not captured.**

```rust,ignore   // fragment, not a complete item
pub async fn when<C, F>(&self, u: &mut Domain, r: &Req, f: F) -> Result<()>
where
    C: Condition<Domain, Req>,
    F: AsyncFnOnce(Ctx<'_, Extended<E, C>>, &mut Domain, &Req) -> Result<()>;
```

- Passing `&user` while capturing it in an `async move` **is a borrow error**
  (verified: E0382 / E0505)
- The `FnOnce(...) -> Fut` form cannot carry the borrow across
  (`lifetime may not live long enough`)
- **Rust 2024 edition async closures (`AsyncFnOnce`, 1.85+) are required**

### The return type is fixed to `Result<()>`

Otherwise the elevated context can be carried out of the scope.

```rust,compile_fail
let elevated = ctx.when::<C, _>(.., async |ctx, ..| Ok(ctx)).await?;
//                                                   ^^^^^^^ type error
```

### What is not guaranteed

**The body of `Condition::holds` cannot be verified in types.**

```rust
fn holds(user: &User, req: &Req) -> bool { true }   // makes it unconditional
```

That route cannot be closed in principle, and it is stated in the AI Context as
`condition_verified: false`. See
[`unverified-boundaries.md`](./unverified-boundaries.md) #20.

---

## Rule 4 — external effects fire after the commit

An external effect that cannot be undone — sending mail, taking payment, calling
a webhook — must not fire inside a transaction.

```rust,compile_fail
// ❌ the mail goes out before the commit
ctx.users().set_email(&mut user, req.email)?;   // not committed
ctx.email().send_verification(&user).await?;     // cannot be undone
ctx.audit_logs().create(...)?;                   // this can still fail
```

```rust,ignore   // fragment, not a complete item
// ✅ fires after the commit
ctx.users().set_email(&mut user, req.email)?;
ctx.audit_logs().create(...)?;
ctx.after_commit(|ctx| async move {
    ctx.email().send_verification(&user).await
}).await?;
```

Issuing the capability for an external effect only inside the `ctx.after_commit`
scope makes this enforceable by the same mechanism as `when`.

> The transaction boundary itself is not designed yet
> ([`research-questions.md`](./research-questions.md)). But **the sample code is
> written in the correct order.** Verum is a framework that supplies "the template
> an AI imitates", and a sample teaching the wrong order gets copied as it is.

---

## The complete implementation example

```rust,ignore   // needs a macro that arrives in M2
#[endpoint(PUT "/users/{id}")]
#[contract(
    domain    = User,
    request   = UpdateUserRequest,
    response  = UserView,

    reads     = [User::id, User::status],
    mutates   = [User::name],
    forbidden = [User::password_hash],
    creates   = [AuditLog],
    emits     = [UserUpdated],

    when(EmailChanged) => {
        mutates = [User::email],
        emits   = [EmailVerificationRequested],
        calls   = [EmailService],
    },
)]
pub struct UpdateUser;

impl Handler for UpdateUser {
    fn handle(&self, req: UpdateUserRequest, ctx: Ctx<'_, Self>)
        -> impl Future<Output = Result<UserView>> + Send
    {
        async move {
            let mut user = ctx.users().find(req.id).await?;

            ctx.users().set_name(&mut user, req.name)?;

            ctx.when::<EmailChanged, _>(&mut user, &req, async |ctx, user, req| {
                ctx.users().set_email(user, req.email.clone())?;
                ctx.events().emit(EmailVerificationRequested::for_user(user))?;
                Ok(())
            }).await?;

            ctx.audit_logs().create(AuditLog::user_updated(&user))?;
            ctx.events().emit(UserUpdated::from(&user))?;

            ctx.after_commit(|ctx| async move {
                ctx.email().send_verification(&user).await
            }).await?;

            Ok(UserView::from(user))
        }
    }
}
```

### Why this implementation is self-evident

| What can be read off it | On what basis |
|---|---|
| The only fields changed are name and email | There are only two lines, `set_name` and `set_email` (Rule 1) |
| **name unconditionally, email conditionally** | Where the contract declares them (top level vs inside `when`). Calling `set_email` outside the `when` is a type error |
| There are six effects in total | Count the `ctx.` lines (Rule 2) |
| The mail goes out after the commit | It is inside the `after_commit` block (Rule 4) |
| status is only read | There is no `set_status` call, and none in the contract |
| password is untouched | Stated in `forbidden`. No capability exists for it either |

**There is not one comment.** This is the concrete form of
[`../concepts.md`](../concepts.md)'s "semantics without comments".

---

## How far each rule is enforced

| Rule | Means | State |
|---|---|---|
| Rule 1 (per-field methods) | Structurally guaranteed within what Verum generates. Depends on domain opacity | A lint (unimplemented) if the user adds methods of their own |
| Rule 2 (through ctx) | Unit-struct enforcement / `Ctx<'req>` / no public `dyn` close the main routes in types | The purity of free-function constructors is **convention** |
| Rule 3 (the when scope) | Capabilities issued only inside the scope. Return type fixed | The body of `Condition::holds` is **unverifiable** |
| Rule 4 (after the commit) | External capabilities issued only in the `after_commit` scope | The transaction design is undecided |

**Do not confuse what is convention with what is enforced by types.** Every
unchecked part is listed in
[`unverified-boundaries.md`](./unverified-boundaries.md) and emitted in the AI
Context.
