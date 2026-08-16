# Architecture contract

Constraining the handler → service → repository path with types and static
analysis rather than convention.

Related: [`capability-system.md`](./capability-system.md),
[`semantic-endpoint.md`](./semantic-endpoint.md).

---

## The structure

```text
Handler
   ↓
Service
   ↓
Repository
```

This is constrained by types and static analysis, not by convention alone.

### The permitted path

```text
UserUpdateEndpoint
    ↓
UserUpdateService
    ↓
UserUpdateRepository
```

### The forbidden path

```text
UserHandler
    ↓
OrderRepository
```

---

## How it works: a where clause on `Ctx`

The same mechanism as the capability system. `ctx.users()`'s where clause
requires that the domain is one the contract declares.

`Ctx` is a framework type, so an inherent impl cannot be written (E0116). The
derive generates an extension trait per domain.

```rust,ignore   // fragment, not a complete item
pub trait CtxUsers {
    type R; type M;
    // The endpoint type. `Includes` is implemented on the endpoint, and a trait
    // method's where clause cannot name `E`, so the trait exposes it as an
    // associated type — see ../adr/0001 and ../adr/0002.
    type Owner;
    fn users(&self) -> Repo<User, Self::R, Self::M>
    where <Self as CtxUsers>::Owner: Includes<User>;   // ← the where goes on the method
}

impl<'req, E: Endpoint> CtxUsers for Ctx<'req, E> { ... }
```

> **The where clause goes on the method, not the impl.** On the impl it becomes
> E0599 (the method exists but its trait bounds are not satisfied) and
> `#[diagnostic::on_unimplemented]` is **discarded** (confirmed by compiling).
>
> ```text
> // ❌ where on the impl
> error[E0599]: the method `orders` exists for struct `Ctx<UpdateUser>`,
>               but its trait bounds were not satisfied
>
> // ✅ where on the method
> error[E0277]: `Order` is not in this endpoint's domain contract
> ```
>
> Fix it in the derive's generated template. Detail in
> [`diagnostics.md`](./diagnostics.md).

```rust,compile_fail
// UpdateUser's contract has domain = User
ctx.orders()
//  ^ type error: `Order` is not in this endpoint's domain contract
```

**Because obtaining the repository is itself the checkpoint, no dedicated linter
is needed.**

This is a side benefit of passing capabilities through the context
([`capability-system.md`](./capability-system.md)). Under the explicit-argument
alternative — obtaining it from `self.repo` — the endpoint would have to be tied
to its repository separately, and this check would not come for free.

---

## The service layer — its position is undecided

The "permitted path" above includes a service, but **no code example anywhere
contains one.** The implementations in
[`handler-rules.md`](./handler-rules.md) and
[`semantic-endpoint.md`](./semantic-endpoint.md) both call `ctx.users()` directly
from the handler.

Worse, `ctx.users()` returning a repository directly **makes bypassing the
service the shortest path.**

### The route that loses capabilities

Passing a `dyn Repository` to a service erases the type parameters, and the
service can then call every setter.

```rust,compile_fail
// ❌ allowing this erases the capability constraint
let svc = UserUpdateService::new(Arc::new(repo) as Arc<dyn UserRepository>);
```

**Do not expose `dyn Repository`.** What a service may receive is a
parameterised `Repo<D, R, M>`, and the service itself carries capabilities in its
type as `Service<Reads, Mutates>`.

Effects reached through a service also fall outside
[`handler-rules.md`](./handler-rules.md) Rule 2's grep guarantee, which counts
lines containing `ctx.`.

### What has to be decided

| Option | Contents |
|---|---|
| **A. Services are optional** | Redraw the diagram as endpoint → repository, and state that a service is used only when business logic is shared across endpoints |
| **B. Services are required** | Provide an example showing in types how a service receives capabilities, and add "capabilities do not leak through a service" to the First PoC's verification list |

A is preferred, to keep the PoC's scope from growing. Recorded in
[`research-questions.md`](./research-questions.md).

---

## Architecture per endpoint pattern

Rather than fixing one shape, an appropriate architecture is defined per
endpoint pattern.

```text
CRUD API
    → Domain / Endpoint / Service / Repository

Read-heavy API
    → Query / Repository

WebSocket
    → Connection / Handler / Session

Background job
    → Job / Service

Streaming
    → Stream / Handler
```

---

## Multi-domain endpoints (undecided)

How to declare an endpoint that touches several domains is not settled.

```rust,ignore   // needs a macro that arrives in M2
// a candidate: declare several under `domains`
#[contract(
    domains = [User, AuditLog],
    reads   = [User::id],
    mutates = [User::status],
    creates = [AuditLog],
)]
```

A domain created incidentally, like `AuditLog`, already appears under `creates`,
so it duplicates the `domain` declaration. This needs sorting out.

Questions to settle:

- Should a domain appearing in `creates` or `emits` become accessible
  automatically?
- Should an endpoint be allowed to update two business-independent domains (a
  user and an order) at once, or should that be pushed into the service layer?
- If allowed, how are transactions across an aggregate boundary handled
  ([`persistence.md`](./persistence.md))?

Recorded in [`research-questions.md`](./research-questions.md).

---

## What must be verified

- The service and repository path can be verified in types.
- Depending on an undeclared repository is a compile error.
- A different architecture can be expressed per endpoint pattern.
- The error message points at the contract declaration
  ([`diagnostics.md`](./diagnostics.md)).
