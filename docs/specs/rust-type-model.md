# Rust type model

Which of Rust's features Verum's type representation uses, and the constraints
confirmed by compiling.

Related: [`capability-system.md`](./capability-system.md),
[`diagnostics.md`](./diagnostics.md),
[`unverified-boundaries.md`](./unverified-boundaries.md).

> The constraints in this file reflect results obtained by actually compiling on
> `rustc 1.99.0-nightly`.

---

## Prerequisites

| Item | Requirement | Reason |
|---|---|---|
| **edition** | **2024** | The `when` scope requires async closures (`AsyncFnOnce`) |
| **MSRV** | **1.85+** | Async closures / `#[diagnostic::do_not_recommend]` |

Fixed as specification alongside `runtime-stack.md`'s dependency policy.

---

## Features used

- Traits / associated types / associated consts
- Generics / phantom types / typestate
- Newtypes
- Proc macros / derive macros
- **Associated-type equality bounds** (`Endpoint<Mutates = ()>`) — stable
- **`#[diagnostic::on_unimplemented]`** (1.78+)
- **`#[diagnostic::do_not_recommend]`** (1.85+) — removing noise from recursive
  impls
- **Async closures / `AsyncFnOnce`** (1.85+, edition 2024)
- Sealed traits (a private supertrait)

### Features that cannot be used

| Feature | State |
|---|---|
| **Associated-const equality bounds** (`Endpoint<METHOD = Method::GET>`) | **Unstable.** Folded into `min_generic_const_args` (incomplete). It would also need the new syntax `type const METHOD: Method;` on the trait side |
| Negative trait bounds (`!Trait`) | Unstable |
| A wildcard over a type parameter (`NotHas<Mutate<_, _>>`) | Cannot be written |
| An inherent impl (from a user crate, on a framework type) | E0116. Replaced by an extension trait |

---

## The `Endpoint` trait

```rust,ignore   // needs a crate or a verum-private module this harness does not carry
pub trait Endpoint: derive_facing::SealedEndpoint {
    type Method;                 // Get / Head / Post / Put / Patch / Delete
    const PATH: &'static str;

    type Domain;
    type Request;
    type Response;

    // effects that may happen unconditionally
    type Reads;
    type Mutates;
    type Creates;
    type Deletes;
    type Emits;
    type Calls;

    // effects that may happen only under a condition
    // (When<C, CondMutates, CondEmits, CondCalls>, (..., ()))
    type Conditional;
}
```

`Conditional`'s elements are **`When`s split by category.** Mixed together, a
type-level `Filter` becomes necessary and the catch-all impl always collides.

```rust
pub struct When<C, CondMutates, CondEmits, CondCalls>(PhantomData<(C, CondMutates, CondEmits, CondCalls)>);
```

The rule for where things are declared (top level = unconditional, inside `when` =
conditional, no duplicates) is in
[`conditional-effects.md`](./conditional-effects.md).

### `Method` is a type-level marker

`type Method = Get`, not `const METHOD: Method`. Two reasons.

1. Associated-**const** equality bounds are unstable (above).
2. **More fundamentally, the logic of
   `impl<E: Endpoint<METHOD = Get>> ReadOnly for E {}` does not work.**

Because `ReadOnly` has `Endpoint<Mutates = (), ...>` as a supertrait, the blanket
impl is forced to require `Mutates = ()` too.

```rust,compile_fail
impl<E: Endpoint<Method = Get>> ReadOnly for E {}
// error[E0271]: type mismatch resolving `<E as Endpoint>::Deletes == ()`
```

So the only impl that can be written is
`impl<E: Endpoint<Mutates=(), Creates=(), Deletes=()>> ReadOnly for E {}`, which
**enforces nothing about the method.** There is no route to enforcing "a GET is
always ReadOnly" through an impl.

### How GET ⇒ ReadOnly is enforced

By a compile-time assertion the derive generates.

```rust
// derive-generated
const _: () = {
    fn assert_readonly<E: Endpoint<Method = Get> + ReadOnly>() {}
    fn check() { assert_readonly::<GetUser>(); }
};
```

The error this produces matches the target form (verified).

```text
error[E0271]: type mismatch resolving `<BadGet as Endpoint>::Mutates == ()`
note: expected this to be `()`
   |     type Mutates = (MutateEmail, ());
   = note: expected unit type `()` found tuple `(MutateEmail, ())`
```

And the `note:` points at the span of the derive-generated `type Mutates`.
**Re-pointing that span at the contract attribute's tokens reaches
[`diagnostics.md`](./diagnostics.md)'s ideal form.**

A simpler alternative is for the proc macro to reject "a GET with
mutates/creates/deletes" at expansion time. That gives the most precise error, so
both are implemented.

### A mutation inside `Conditional` is rejected by the macro

Because conditional mutations are declared inside `when`
([`conditional-effects.md`](./conditional-effects.md)), `Mutates = ()` alone
cannot guarantee read-only: the `CondMutates` inside `Conditional` must be empty
too.

Checking that in types needs a recursive fold over `Conditional`
(`AllCondMutatesEmpty`), which approaches negative reasoning and degrades the
error into a form that does not say which element caused it.

**The macro rejects it.** On a read-only method (`Get` / `Head`), `mutates` /
`creates` / `deletes` cannot appear inside `when` either. Catch at layer 1
whatever layer 1 can catch ([`diagnostics.md`](./diagnostics.md)).

---

## Effect sets are cons lists

**A flat tuple `(A, B, C)` cannot implement a membership decision.**

```rust,compile_fail
// ❌ per-position impls on a flat tuple are always E0119
impl<A, B> Has<A> for (A, B) {}
impl<A, B> Has<B> for (A, B) {}
// error[E0119]: conflicting implementations of trait `Has<_>` for type `(_, _)`
```

It fails **at the impl definition**, regardless of whether a user ever writes a
duplicate element.

So cons lists are used throughout.

```rust
type Mutates = (Mutate<User, user::Name>, (Mutate<User, user::Email>, ()));
```

The derive generates them so users never write them, but **cons lists are exposed
in error messages.**

| Set | Representation |
|---|---|
| Empty | `()` |
| One element | `(A, ())` |
| Two elements | `(A, (B, ()))` |

---

## `Has<Set, Elem, Idx>` — the index parameter is mandatory

The naive recursive impls violate coherence.

```rust,compile_fail
// ❌ a coherence violation
pub trait Has<T> {}
impl<H, T> Has<H> for (H, T) {}
impl<H, X, T> Has<H> for (X, T) where T: Has<H> {}
// error[E0119]: conflicting implementations
```

The two impls overlap when `H == X`. The reason is **not** that where clauses are
ignored, but that the tail's `T: Has<H>` is **satisfiable at that intersection**,
so it does not separate the impls (corrected in T-M0-08; this distinction is what
the seal design rests on — [`../rules/api-surface.md`](../rules/api-surface.md)
§2).

Solved with a frunk-style index type parameter (verified).

```rust,ignore   // needs a crate or a verum-private module this harness does not carry
pub trait Has<T, Idx>: private::SealedHas<T, Idx> {}

// Added in T-M0-07. The bound goes on the impl, not the trait (so callers do not restate it).
pub trait ConsList: private::SealedConsList {}   // well-formedness of the shape
pub trait Index: private::SealedIndex {}         // the position of membership

pub struct Here(PhantomData<()>);              // private field: not constructible downstream (E0423)
pub struct There<I>(PhantomData<fn() -> I>);   // `fn() -> I`: does not inherit `I`'s auto traits

#[diagnostic::do_not_recommend]
impl<H, T: ConsList> Has<H, Here> for (H, T) {}

#[diagnostic::do_not_recommend]
impl<H, X, T: ConsList, I: Index> Has<H, There<I>> for (X, T) where T: Has<H, I> {}
```

### The cost

**Every method that uses `Has` gains an inference-only type parameter `I`.**

```rust,ignore   // fragment, not a complete item
fn set_email<I>(&self, u: &mut User, v: Email) -> Result<()>
where M: Has<Mutate<User, user::Email>, I>;
```

The derive generates them so users never write them, but every signature in the
documentation takes this shape.

### A duplicate element gives E0283

The index approach assumes an element appears exactly once. With a duplicate, `I`
is not uniquely determined.

```text
error[E0283]: type annotations needed
note: multiple `impl`s satisfying `(Mn, (Mn, ())): Has<Mn, _>` found
```

There are two routes to it.

1. An AI writes a duplicate: `mutates = [User::email, User::email]`
2. **Appending the outer emits and the conditional emits in a `when` scope
   produces the same effect twice** (`emits = [UserUpdated]` together with
   `when(X) => { emits = [UserUpdated] }`)

Route 2 breaks on a perfectly legitimate contract. **The derive detects duplicates
and rejects them, and dedups before calling `Append`** (**`Append` itself cannot
dedup** — settled in T-M0-09;
[`../rules/type-level.md`](../rules/type-level.md) §3).

---

## Which type-level operations are viable

| Operation | Viable? | Use |
|---|---|---|
| `Has<Set, Elem, Idx>` (implemented as `Has<T, Idx>`, with `Self` the set) — membership of a single element | **Safe** (linear in the element count) | The capability check |
| `Append<A, B>` (implemented as `Append<B>`, with `Self` the left side) — cons-list concatenation | **Safe** (no coherence problem, no index needed, verified) | Composing capabilities in a `when` scope |
| `Lookup<Set, Key, Idx>` — a type-level map lookup | **Safe** (with the index-parameter version, verified) | Retrieving a condition from `Conditional` |
| `Subset<A, B>` — containment between sets | **Avoid** (combinatorial explosion) | — |
| `Filter<Set, Pred>` — a type-level filter | **Avoid** (the catch-all impl always collides) | — |
| Negative reasoning (`NotHas`) | **Impossible** | Replaced by `Mutates = ()` |

> This originally said "avoid set operations; all that is needed is membership of
> a single element", but **conditional effects require `Lookup` and `Append`.** The
> policy has been refined into the table above.
>
> To avoid needing `Filter`, **the derive generates `Conditional` split by
> category** (`When<C, CondEmits, CondCalls, ...>`). That is a mandatory design
> constraint.

---

## Field marker types

```rust,ignore   // needs a macro that arrives in M2
#[derive(Domain)]
pub struct User {
    id:    UserId,      // private is required (pub is a derive error)
    email: Email,
}

// generated
pub mod user {
    pub struct Id;
    pub struct Email;

    impl Field<User> for Email {
        const NAME: &'static str = "email";
        type Ty = Email;
    }
}
```

`const NAME: &str` (with the lifetime elided) and `type Ty` both work without
trouble (verified).

Why a domain is made opaque is in
[`mutation-contract.md`](./mutation-contract.md).

---

## `Ctx` and `Repo` are provided through extension traits

`Ctx` / `Repo` / `Projection` are framework types. **An inherent impl can only be
written in the crate that defines the type** (E0116). The derive runs in the
user's crate, so the shape in the documentation's initial draft is impossible in
principle.

```rust,compile_fail
// ❌ cannot be written in the user's crate
impl<E: Endpoint> Ctx<E> { fn users(&self) -> Repo<User, ...> { ... } }
// error[E0116]: cannot define inherent `impl` for a type outside of the crate
```

The derive generates a local extension trait per domain (verified with a
two-crate setup).

```rust,ignore   // fragment, not a complete item
pub trait CtxUsers {
    type R; type M;
    fn users(&self) -> Repo<User, Self::R, Self::M>;
}

impl<'req, E: Endpoint> CtxUsers for Ctx<'req, E> { ... }

pub trait UserRepo<M> {
    fn set_email<I>(&self, u: &mut User, v: Email) -> Result<()>
    where M: Has<Mutate<User, user::Email>, I>;
}

impl<R, M> UserRepo<M> for Repo<User, R, M> { ... }
```

### Side effects

- The user has to `use` the extension trait. Forgetting it gives the unrelated
  error "no method named `users`" → **the derive emits a `pub use`, or
  `verum::prelude` is provided**
- Going through associated types lengthens the type names
  (`Repo<User, <Ctx<E> as CtxUsers>::R, _>`)

### The where clause goes on the method

On the impl it becomes E0599 and `on_unimplemented` is ignored (verified).

```text
// ❌ where on the impl → the intended message does not appear
// error[E0599]: the method `orders` exists ... but its trait bounds were not satisfied

// ✅ where on the method → as intended
// error[E0277]: `Order` is not in this endpoint's domain contract
```

**Fixed in the derive's generated template.**

---

## The `Handler` trait — RPITIT + Send + an erasure layer

Using AFIT (`async fn` in trait) directly has two problems (verified).

```text
// 1. the router cannot hold a Box<dyn Handler>
// error[E0038]: the trait `Handler` is not dyn compatible

// 2. it does not load onto tokio::spawn / hyper
// error: future cannot be sent between threads safely
```

The fix:

```rust
// Send is solved by RPITIT
pub trait Handler: Endpoint {
    fn handle(&self, req: Self::Request, ctx: Ctx<'_, Self>)
        -> impl Future<Output = Result<Self::Response>> + Send;
}
```

That does not solve dyn compatibility, so **the derive generates an object-safe
erasure layer.**

```rust,ignore   // fragment, not a complete item
fn call(&self, rt: &Runtime, req: Request<Body>)
    -> Pin<Box<dyn Future<Output = Response> + Send + '_>>;
```

**Note the `Runtime` parameter.** A `Ctx<'req, E>` cannot cross the erasure
boundary at all, so the erased handler takes the `Runtime` and **builds the
`Ctx` on the far side of the boxing** — without it there is no source for
`'req`. Measured in T-M1-02 (#14); the reasoning is in
[`capability-system.md`](./capability-system.md)「The erasure layer builds the
`Ctx` — a signature no document describes」.

The router holds `dyn ErasedHandler`. The middleware chain is under the same
constraint, so [`runtime-stack.md`](./runtime-stack.md)'s line estimates have to
include the erasure layer's cost.

---

## The runtime representation of a capability

Capabilities are never materialised as values; they are **expressed through
`Ctx<'req, E>`'s type parameters.** They are all ZSTs with no runtime presence
([`performance.md`](./performance.md)).

> [`persistence.md`](./persistence.md)'s repository trait definition carried a
> `cap: &Cap<...>` argument. It contradicted this policy and was removed.
> Capabilities are not passed as arguments.

---

## What a proc macro can see

A proc macro sees only the tokens of a single item, not the bodies of what it
calls.

An attribute macro on the impl block **can** see all of `handle`'s body tokens —
compile-verified in T-M1-07 (#37), which recovered three of the five distinct
contract keys from `handler-rules.md`'s worked example (five of seven counting
`when`-scoped instances separately), **including the conditional split**: an
effect inside `ctx.when::<C>` comes back tagged with `C`, never at top level,
and a nested `when` carries both conditions.

> **What does not hold is the second half of the claim, and the scan is not
> monotone.** An earlier version of this section said effects are "syntactically
> confined inside a single item, `handle`", and that *that* is what makes
> generation feasible. Confinement is a **convention**, and the measured
> failures fall into two groups with different fixes:
>
> | The scan cannot leave its item | The scan matches by spelling |
> |---|---|
> | a free associated function taking `&ctx` | **the handler parameter named anything but `ctx`** — voids every key at once, and nothing warns |
> | a helper in a **sibling** `impl` block | `let repo = ctx.users(); repo.set_name(..)` |
> | **an effect from a `macro_rules!` expansion** — unreachable even with cross-item analysis, since the macro may come from another crate | `Repo::set_name(&ctx.users(), ..)` (UFCS) |
>
> The right-hand column is closable — in the scanner, or at layer 1 for the
> parameter name. The left column's third row is not. `E0407` does forbid a
> helper *beside* `handle` in the trait impl, but a nested `fn` inside `handle`
> and a trait default method are both visible, so placement is the variable, not
> the language.
>
> **And it reports effects that never run.** A proc macro executes before
> cfg-stripping, so a `#[cfg]`-gated statement naming a nonexistent type appears
> in the output. So the generated set is neither a subset nor a superset of what
> the program does. A missing effect reads as over-declaration; a phantom one
> reads as **under-declaration**, whose repair is to widen the contract — the
> bias `evaluation.md`'s Q-C measured. The type system refuses narrowing only for
> keys at `upper_bound_checked`; **`reads` is `metadata_only` and has no such
> backstop.**

Measurement and reproduction: `spikes/contract-from-tokens/`. Decision:
[`effect-inference.md`](./effect-inference.md).

Conversely, a macro seeing a single item is also what allows spans inside the
attribute to be preserved ([`diagnostics.md`](./diagnostics.md)).

---

## Open questions

- The exposure of cons lists and `There<There<...>>` in error messages (mitigated
  by `do_not_recommend`, but not eliminated)
- Whether the derive can generate type aliases to emit shorter names
- ~~Interoperating domain opacity with sqlx's `query_as!` / `FromRow`~~ —
  **verified (T-M1-01 / #13).** The interoperation holds; the trust-boundary claim
  does not. [`persistence.md`](./persistence.md) §Verdict
- ~~Combining `Ctx<'req, E>` with RPITIT and async closures~~ — **settled
  (T-M1-02 / #14, 21 probes, compile-verified)**. RPITIT `Handler` holds, the
  future loads on a multi-thread hyper server, `tokio::spawn` is rejected
  (`E0521`), and `when`'s elided `AsyncFnOnce` compiles and runs. Two things the
  design did not intend: the elision is load-bearing
  ([`conditional-effects.md`](./conditional-effects.md) §The elision is
  load-bearing), and ledger path 8 is closed by the higher-ranked `Ctx` rather than
  by the remedy three documents recorded — while a *named* `'req` leaks and
  nothing stops it (RK-017: `+ Send` is not a containment bound)
- The ergonomics of the projection type
- Whether this extends beyond Rust (Go / TypeScript)

See [`research-questions.md`](./research-questions.md).
