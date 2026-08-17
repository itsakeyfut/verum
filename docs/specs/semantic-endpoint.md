# Semantic endpoint

Expressing an endpoint as a semantic contract rather than an HTTP function. The
specification of the contract declaration syntax.

Related: [`handler-rules.md`](./handler-rules.md),
[`effect-system.md`](./effect-system.md),
[`capability-system.md`](./capability-system.md),
[`diagnostics.md`](./diagnostics.md).

---

## The problem

In an ordinary web framework, a signature like this tells you nothing without
reading the endpoint's body.

```rust,ignore   // needs a macro that arrives in M2
#[put("/users/{user_id}")]
async fn update_user(...) -> Result<User>
```

What is unknown: what it changes, what it reads, whether it writes to the
database, whether it calls an external service, whether it emits an event, and
under which conditions any of that differs.

---

## Decision: declare it in an attribute, and let the derive expand it into types

```rust,ignore   // needs a macro that arrives in M2
#[endpoint(PUT "/users/{id}")]
#[contract(
    domain    = User,
    request   = UpdateUserRequest,
    response  = UserView,

    reads     = [User::id, User::status],
    mutates   = [User::name],              // changed unconditionally
    forbidden = [User::password_hash],
    creates   = [AuditLog],
    emits     = [UserUpdated],

    when(EmailChanged) => {
        mutates = [User::email],           // changed only under this condition
        emits   = [EmailVerificationRequested],
        calls   = [EmailService],
    },
)]
pub struct UpdateUser;
```

The types the derive expands to:

```rust,compile_fail
impl Endpoint for UpdateUser {
    type Method = Put;                    // a type-level marker, not a const
    const PATH: &'static str = "/users/{id}";

    type Domain    = User;
    type Request   = UpdateUserRequest;
    type Response  = UserView;

    // cons-list representation (membership cannot be implemented with flat tuples)
    // A field in `mutates` needs its previous value read, so it is automatically
    // included in `reads` ([`read-contract.md`](./read-contract.md)). The declaration
    // names two — id and status — but after expansion `name` from
    // `mutates = [User::name]` joins them, making three.
    type Reads   = (Read<User, user::Id>,
                   (Read<User, user::Status>,
                   (Read<User, user::Name>, ())));
    type Mutates = (Mutate<User, user::Name>, ());
    type Creates = (Create<AuditLog>, ());
    type Emits   = (Emit<UserUpdated>, ());
    type Deletes = ();
    type Calls   = ();

    type Conditional = (
        When<EmailChanged,
             (Mutate<User, user::Email>, ()),          // CondMutates
             (Emit<EmailVerificationRequested>, ()),   // CondEmits
             (Call<EmailService>, ())>,                // CondCalls
        (),
    );
}
```

> **The rule for where things are declared**: a top-level `mutates` / `emits` /
> `calls` is what may happen unconditionally; one inside `when(C)` is what may
> happen only under that condition. Writing the same element in both is forbidden
> (the macro rejects it). Detail in
> [`conditional-effects.md`](./conditional-effects.md).

### Supported HTTP methods

The following are provided as type-level markers.

| Method | Marker type | read-only |
|---|---|---|
| GET | `Get` | ✅ |
| HEAD | `Head` | ✅ |
| POST | `Post` | |
| PUT | `Put` | |
| PATCH | `Patch` | |
| DELETE | `Delete` | |

`OPTIONS` is not declared as an endpoint (CORS is handled by a tower-http layer —
[`middleware.md`](./middleware.md)).

A read-only method cannot declare `mutates` / `creates` / `deletes` — the macro
rejects it, including inside `when`.

> Why `Method` is a type, why cons lists are used, and why `Conditional` is split
> by category are all in [`rust-type-model.md`](./rust-type-model.md) (constraints
> confirmed by compiling).

### An endpoint is a unit struct only

```rust
pub struct UpdateUser;                   // ✅
```

```rust,compile_fail
pub struct UpdateUser { pool: PgPool }   // ❌ the derive errors
```

With fields, `self.pool` could run SQL directly and bypass `ctx`. This is the
condition that makes [`handler-rules.md`](./handler-rules.md) Rule 2 hold in
types.

### Why this approach

| Aspect | attribute → types | pure associated types | declarative macro DSL | external file |
|---|---|---|---|---|
| Strength of type checking | strong | strong | strong | weak (a generation boundary) |
| **Error precision at the macro stage** | **can emit `did you mean` for a field-name typo** | cannot | spans drift easily | diverges from the types |
| IDE completion | works (`User::name` is a real path) | works | **does not work** | does not work |
| Verbosity | low | high | lowest | medium |

**The deciding factor is the precision of the errors that can be rejected at the
macro stage.** A derive macro preserves the spans of the tokens inside the
attribute, so a nonexistent field or domain is rejected with a precise error
before type checking.

> **Correction**: the deciding factor was originally given as "a `note` pointing
> at the contract declaration can be emitted when a trait bound is violated". That
> **does not hold.** An `on_unimplemented` note is plain text with no span, and the
> span rustc emits is the location of `Has`'s impl definition. A note with a span
> appears only for an associated-type equality bound (a `Mutates = ()` violation,
> for instance). Detail in [`diagnostics.md`](./diagnostics.md).

### Relationship to the external-file approach (the Goa approach)

**The framing "types are authoritative vs. an external file is authoritative"
does not hold.** The contents of `#[contract(...)]` are not Rust type syntax
either but a token stream a proc macro interprets, and the types are its output.
Structurally it is the same as Goa.

Two differentiators do hold:

1. **What the contract covers** — not only the HTTP contract but internal state
   changes, effects, capabilities and architecture.
2. **The locality of errors** — a violation comes back as a compile error pointing
   at the declaration.

[`../concepts.md`](../concepts.md)'s differentiation is stated in these two terms.

### The form of a field reference

Inside the attribute it is written `User::name` (the field name), and the derive
converts it to the `user::Name` marker type after checking that it exists.

- It keeps the form an AI writes naturally.
- A nonexistent field is rejected by the macro, with `did you mean`.
- `User::name` is a real path, so IDE completion and go-to-definition work.

---

## What a semantic endpoint is made of

```text
Endpoint
├── Method (a type-level marker)
├── Path
├── Domain
├── Request / Response
├── Reads          → read-contract.md
├── Mutates        → mutation-contract.md
├── Creates / Deletes
├── Emits / Calls  → effect-system.md
├── Conditional    → conditional-effects.md
└── Capabilities   → capability-system.md
```

### `operation` was removed

The initial design had `operation = Update`. It has been **removed.**

Why:

- **Its value set could not be defined** — the subject of the Q-C experiment
  reported that when adding a new endpoint it "reused the existing `Update` to
  avoid the risk of inventing an enum variant that does not exist". It was a field
  the AI hesitated over every time and which guaranteed nothing.
- **It duplicated information** — the kind of operation is derivable from `Method`
  plus `Domain` plus `mutates` / `creates` / `deletes`.
- **It collided with effect names** — `operation = Read` against
  `Read<User, user::Id>`, `operation = Create` against `Create<AuditLog>`.

**The endpoint's type name carries the business operation name.** The type name
`SuspendUser` already says "a Suspend operation on User"; writing
`domain = User, operation = Suspend` was redundant.

The type name is emitted in the AI Context as `"endpoint": "SuspendUser"`, so no
information is lost.

> Naming convention: an endpoint's type name is recommended to follow
> `<Operation><Domain>` (`GetUser` / `UpdateUser` / `SuspendUser` / `DeleteUser`).
> It is not enforced.

---

## The handler signature

A fixed signature is used. A variadic handler — one taking any number of
arbitrary extractors — does not suit the goal of constraining capabilities in
types.

```rust,ignore   // fragment, not a complete item
impl Handler for UpdateUser {
    fn handle(&self, req: UpdateUserRequest, ctx: Ctx<'_, Self>)
        -> impl Future<Output = Result<UserView>> + Send;
}
```

**RPITIT + `Send` is used, not AFIT (`async fn` in trait).** With AFIT the future
is not `Send` and does not load onto hyper's multi-thread runtime (confirmed by
compiling). dyn compatibility is solved separately, by the derive generating an
object-safe erasure layer.

`Ctx<'req, Self>` is bound to the request's lifetime (it is not `'static`) and
carries the capabilities. The implementation conventions are in
[`handler-rules.md`](./handler-rules.md).

---

## A complete example

### The domain definition

```rust,ignore   // needs a macro that arrives in M2
#[domain]
pub struct User {
    id:            UserId,      // private is required (pub is a derive error)
    name:          String,
    email:         Email,
    password:      PasswordHash,
    status:        UserStatus,
    last_login_at: Option<DateTime<Utc>>,
    created_at:    DateTime<Utc>,
}
```

The macro generates: field marker types, capability-checked accessors, a
`pub(crate)` `Repr`, and `Debug` and `Serialize` implementations that emit only
the declared fields.

> **⚠️ Two points here were overturned by T-M1-01 / #13.** `Repr` does **not**
> become "for the repository implementation only" (`pub(crate)` means the whole
> application crate — ledger path 21). And whether it is `#[derive(...)]` or an
> attribute macro is **undecided** (a derive cannot add an item with the same name
> as its input). See [`persistence.md`](./persistence.md) §Verdict.

**Under `#[domain]` a field cannot be `pub`** — the attribute consumes the user's
`pub` and emits a private inner field, so `user.email = v` cannot compile whether
or not the macro's check runs. The check is a **lint**
([ADR-0011](../adr/0011-domain-is-an-attribute-macro.md)); the reasoning is in
[`mutation-contract.md`](./mutation-contract.md).

### GET

```rust,ignore   // needs a macro that arrives in M2
#[endpoint(GET "/users/{id}")]
#[contract(
    domain    = User,
    request   = GetUserRequest,
    response  = UserView,
    reads     = [User::id, User::name, User::email, User::status],
)]
pub struct GetUser;

impl Handler for GetUser {
    fn handle(&self, req: GetUserRequest, ctx: Ctx<'_, Self>)
        -> impl Future<Output = Result<UserView>> + Send
    {
        async move {
            let user = ctx.users().find(req.id).await?;
            Ok(UserView::from(user))
        }
    }
}
```

Because `mutates` / `creates` / `deletes` are not declared, they become `()`. For
a GET, `Mutates = ()` is structurally required.

### PUT

The complete implementation example, including `when` and `after_commit`, is in
[`handler-rules.md`](./handler-rules.md). **Calling `when` requires an async
closure (edition 2024 / MSRV 1.85+)**, and takes the shape of lending `user` and
`req` as closure arguments rather than letting them be captured.

---

## Undecided points

- **Errors** — whether the contract carries which errors can be returned
  (`fails = [NotFound, Conflict]`). Required for OpenAPI generation.
- **Validation** — whether request constraints are declared in the contract
  (entirely out of scope today).
- **Transactions** — the relationship between an endpoint and a transaction
  boundary.
- **Multi-domain** — the declaration form when one endpoint touches several
  domains.
- **Listing, aggregation, JOIN** — consequences of `Read<Domain, Field>` assuming
  a single instance. The shape that accounts for the most screens in a real web
  application cannot be written.
- **Jobs and background work** — the endpoint framing does not apply to processing
  with no HTTP request.
- **State transitions** — whether `status: active → suspended` is expressed in the
  contract.

Recorded in [`research-questions.md`](./research-questions.md).
