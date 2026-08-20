# Conditional effects

Expressing an effect that happens under a condition. The hardest area of the
design.

Related: [`handler-rules.md`](./handler-rules.md),
[`mutation-contract.md`](./mutation-contract.md),
[`rust-type-model.md`](./rust-type-model.md),
[`unverified-boundaries.md`](./unverified-boundaries.md).

---

## The problem

A plain effect declaration is not enough.

```text
if email_changed:
    Mutate(User.email)
    Emit(EmailVerificationRequested)

if status == suspended:
    Mutate(User.status)
    Revoke(UserSession)
```

---

## The constraint that emerged

**Rust's type system cannot express `if email_changed then Emit<X>` directly.**
Full dependent types are impossible.

```text
types      → guarantee "what may happen in which scope"
conditions → a runtime witness plus metadata
```

What the types guarantee is that it **never happens outside this scope.** They do
not guarantee "it happens only under this condition" — the body of a condition is
unverifiable.

---

## Decision: the rule for where things are declared

> The point the Q-C experiment flagged: "whether a conditional mutation is written
> inside `when` or at the top level had to be reverse-engineered from the code
> examples". Fixed here as specification.

### The rule

```text
mutates / emits / calls at the top level  → may happen unconditionally
mutates / emits / calls inside when(C)    → may happen only under that condition
Writing the same element in both is forbidden (the macro rejects it)
```

### An example declaration

```rust,ignore   // needs a macro that arrives in M2
#[endpoint(PUT "/users/{id}")]
#[contract(
    domain    = User,
    request   = UpdateUserRequest,
    response  = UserView,

    reads     = [User::id],
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

### The expansion into types

```rust
type Mutates = (Mutate<User, user::Name>, ());
type Emits   = (Emit<UserUpdated>, ());
type Calls   = ();

type Conditional = (
    When<EmailChanged,
         /* CondMutates */ (Mutate<User, user::Email>, ()),
         /* CondEmits   */ (Emit<EmailVerificationRequested>, ()),
         /* CondCalls   */ (Call<EmailService>, ())>,
    (),
);
```

**Splitting by category is mandatory.** Mixed together, a type-level `Filter`
would be needed to extract only the mutations from `Conditional`, and the
catch-all impl always collides
([`rust-type-model.md`](./rust-type-model.md)).

### The consequence in an implementation

```rust,ignore   // fragment, not a complete item
ctx.users().set_name(&mut user, req.name.clone())?;      // ✅ declared at the top level
```

```rust,compile_fail
ctx.users().set_email(&mut user, req.email)?;    // ❌ type error
//          ^^^^^^^^^ the outer ctx does not hold Mutate<User, user::Email>

ctx.when::<EmailChanged, _>(&mut user, &req, async |ctx, user, req| {
```

```rust,ignore   // fragment, not a complete item
    ctx.users().set_email(user, req.email.clone())?;   // ✅ the inner ctx only
    ctx.events().emit(EmailVerificationRequested { .. })?;
    Ok(())
}).await?;
```

The context inside a `when` scope has these types.

```text
Mutates = <E::Mutates as Append<CondMutates>>::Out
Emits   = <E::Emits   as Append<CondEmits>>::Out
Calls   = <E::Calls   as Append<CondCalls>>::Out
```

`Append` is cons-list concatenation and is implementable without a coherence
problem (implemented and verified in T-M0-09). `Lookup` retrieves the matching
`When<C, ..>` from `E::Conditional`.

> **Correction (T-M0-09)**: an earlier version said "both need an index-parameter
> version", but **`Append` does not need an index parameter.** Its two impls target
> `()` and `(H, T)` and are structurally disjoint, so they never overlap. An index
> is needed only where **both impls target `(_, _)`**, as with `Has` and `Lookup`.
> [`rust-type-model.md`](./rust-type-model.md)'s table had written them
> differently from the start — `Append<A, B>` (no index) and
> `Lookup<Set, Key, Idx>` (with one) — so **the two specs contradicted each other.
> The compiler agreed with the latter.**

`Lookup`'s map is **a cons list of `(key, value)` pairs**
(`((C, When<C, ..>), rest)`). It is not the `Keyed` approach in which an entry
declares its own key, because `typelevel` is the bottom of the dependency stack
and must not know about `When` ([`../rules/design.md`](../rules/design.md) §2).
The redundancy of the key appearing twice is absorbed by the derive. Adding
`Keyed` is non-breaking, so it can be added later if needed.

**`Append` cannot dedup.** It would have to branch on an element being **absent**
from the other side, which needs a total boolean membership decision — the
catch-all impl collides (E0119) and there is nowhere to put the index witness
(E0207). `Has` works because it is a *partial* relation. **It is not because
`Subset` is banned** (`Subset` can be written as a partial predicate; the reason
it is banned is cost) — corrected in T-M0-09. So composing `emits = [X]` with
`when(C) => { emits = [X] }` silently produces `(X, (X, ()))`, and the E0283
surfaces at a distant `Has`. **Dedup is unconditionally the macro's
responsibility**, and
`compile_fail/append_duplicate_breaks_membership.rs` pins that route.

### Why "declare everything at the top level" was not chosen

```rust,ignore   // fragment, not a complete item
// the rejected option
mutates = [User::name, User::email],   // email treated as unconditional too
when(EmailChanged) => { emits = [..] },
```

In that shape, `set_email` can be called unconditionally outside the `when`.
**Verum's core claim — that email changes only under a condition — would not hold
for mutations.**

The types get simpler (no `CondMutates`), but `Append` is already needed for
emits and calls, so the additional cost is close to nothing.

### Why duplicates are forbidden

Two reasons.

1. **A semantic contradiction** — a field that is "both unconditional and
   conditional" has no definition.
2. **A technical breakdown** — a duplicate surviving `Append` breaks `Has`'s index
   inference, producing an unrelated E0283 (type annotations needed).

```text
error[E0283]: type annotations needed
note: multiple `impl`s satisfying `(Mn, (Mn, ())): Has<Mn, _>` found
```

The macro rejects it ([`diagnostics.md`](./diagnostics.md), layer 1).

```text
error: `User::email` is declared both unconditionally and under `when(EmailChanged)`
  --> src/endpoints/user.rs:12:16
   |
12 |     mutates = [User::name, User::email],
   |                            ^^^^^^^^^^^^ declared unconditionally here
...
17 |         mutates = [User::email],
   |                    ^^^^^^^^^^^^ and conditionally here
   |
   = help: remove one of them — a field is either unconditional or conditional
```

### The effective set (the upper bound)

"Every field this endpoint may change" is:

```text
effective_mutates = mutates ∪ (the CondMutates of every when)
```

The source splits it across two places, so **the AI Context emits the combined,
complete form.** This is the same structure as
[`effect-system.md`](./effect-system.md)'s "a delta to write, the complete form to
read".

---

## Relationship to a GET's read-only guarantee

`Mutates = ()` alone is not enough: the `CondMutates` inside `Conditional` must be
empty too.

Checking that in types needs a recursive fold over `Conditional`, which
approaches negative reasoning — the area
[`rust-type-model.md`](./rust-type-model.md) rules out.

**The macro rejects it.**

```text
error: GET endpoint `GetUser` cannot declare mutations
  --> src/endpoints/user.rs:16:9
   |
16 |         mutates = [User::status],
   |         ^^^^^^^^^^^^^^^^^^^^^^^^ inside `when(...)` on a GET endpoint
   |
   = note: GET endpoints are read-only by construction
   = help: use PUT / PATCH / POST / DELETE
```

The same for `creates` and `deletes`. Catch at layer 1 whatever layer 1 can catch
([`diagnostics.md`](./diagnostics.md)).

---

## The implementation signature

```rust,ignore   // fragment, not a complete item
pub async fn when<C, F>(&self, u: &mut E::Domain, r: &E::Request, f: F) -> Result<()>
where
    C: Condition<E::Domain, E::Request>,
    F: AsyncFnOnce(Ctx<'_, WhenScope<E, C, I>>, &mut E::Domain, &E::Request) -> Result<()>;
```

- `user` and `req` are **lent as arguments, not captured.** Passing `&user` while
  capturing it in an `async move` is a borrow error (confirmed by compiling:
  E0382 / E0505 ×2 / E0382)
- **Rust 2024 edition async closures (`AsyncFnOnce`, 1.85+) are required.** The
  `FnOnce(..) -> Fut` form cannot carry the borrow across
- **The return type is fixed to `Result<()>`** — but it is **not** what closes the
  scope, and for a named `'req` it closes nothing. §What actually closes the scope,
  below, is the measurement; ledger path 8 is the row. Stated here because this
  bullet is what a skimmer reads

```rust,compile_fail
let elevated = ctx.when::<C, _>(.., async |ctx, ..| Ok(ctx)).await?;
//                                                   ^^^^^^^ type error
```

### The elision is load-bearing — do not write it out

`Ctx<'_, ..>`, `&mut E::Domain` and `&E::Request` above elide **three
independent** higher-ranked lifetimes. Binding them together — the obvious way to
"make the signature explicit" — stops compiling inside a `+ Send` handler future.
Measured in #14 against the realistic pattern (`user` read after the scope):

```text
AsyncFnOnce(Ctx<'_,E>, &mut D, &R)                    the spec, elided   compiles
for<'a,'b,'c> AsyncFnOnce(Ctx<'a,E>, &'b mut D, &'c R)                   compiles
for<'a,'b>    AsyncFnOnce(Ctx<'a,E>, &'b mut D, &'b R)                   rejected
for<'a>       AsyncFnOnce(Ctx<'a,E>, &'a mut D, &'a R)                   rejected
```

`error: implementation of AsyncFnOnce is not general enough`. The last row is
D1b, and it is the form an implementer reaches for first. **The first version of
the #14 spike measured exactly that row, believed it was measuring the specified
signature, and reported that `when` does not work.**

### What actually closes the scope — and what does not

The bullet above says the return type is what stops `Ok(ctx)`. For the signature
as written that is true but **redundant**; for a signature an implementer might
write instead it is **false**. Measured:

| Form | Result |
|---|---|
| `Result<()>`, higher-ranked `Ctx` (D3) | `E0308` — the return type rejects it |
| free return type, higher-ranked `Ctx` (D4) | rejected anyway — the higher-ranked `Ctx` rejects it |
| `Result<()>`, **named** `'req`, leaking (D5c) | **compiles — the scope leaks** |

**The specified signature is closed by the higher-ranked `Ctx`**, which D4
isolates: strip the return type constraint and it is still rejected. The return
type is real but not the mechanism.

**A named `'req` is closed by nothing.** D5c type-checks, and it is reachable
from an ordinary handler — measured in review by two independent agents and
reproduced by a third: it compiles, runs against a real multi-thread hyper
server, and mutates the store through a `Ctx` that outlived its `when` scope.

> ### ⚠️ `+ Send` is not a containment bound
>
> An earlier version of this section said `+ Send` on the handler's future was
> what closed the path. **That is withdrawn.** `Handler::handle` is
> `fn .. -> impl Future + Send`, not `async fn`, so the bound constrains only what
> the *returned future* holds across awaits. A handler body is synchronous, already
> holds `Ctx<'req, Self>` with `'req` named, and can drive the leaking future to
> completion before it ever builds the future it returns. `.await` is the only
> thing that propagates the obligation.
>
> `+ Send` *is* the discriminator for the awaited form — D5a/D5b/D5d fail under it
> and pass without it, measured both directions. That is a fact about HRTB
> inference, not a containment guarantee, and the two must not be conflated again.

**The remedy is a constraint on the signature.** `when` must be generated with the
elided (higher-ranked) form, never with a named `'req`. `when` is macro-generated,
so this belongs at **defence layer 1** — the macro emits the signature and can
refuse to emit the broken one ([`diagnostics.md`](./diagnostics.md)). Nothing
enforces it today; the ledger entry is
[`unverified-boundaries.md`](./unverified-boundaries.md) path 8.

---

## What is guaranteed

| Guarantee | Mechanism |
|---|---|
| A conditional mutation / emit / call does not fire outside the scope | The outer `ctx` does not hold the capability → type error |
| An undeclared conditional effect cannot fire | It is not in `Conditional` → `Lookup` fails |
| The elevated context cannot be carried out of the scope | **Only for the signature as written**, where the higher-ranked `Ctx` rejects it (the return type does too, redundantly). A named `'req` leaks and nothing stops it — see §What actually closes the scope |
| The correspondence between condition and effect is visible in the code | It is visualised as a block structure |

---

## What is not guaranteed — the limit in principle

**The body of `Condition::holds` cannot be verified in types.**

```rust
impl Condition<User, UpdateUserRequest> for EmailChanged {
    const NAME: &'static str = "EmailChanged";
    fn holds(user: &User, req: &UpdateUserRequest) -> bool {
        true      // ← this makes every conditional effect unconditional
    }
}
```

`when` is not a mechanism by which a condition restricts effects; it is **a
mechanism by which an unverified boolean the user wrote unlocks a capability.**

And because the AI Context keeps emitting `"conditional": [...]`, **the metadata
actively lies.**

### What is done about it

- **Always** emit `condition_verified: false` in the AI Context
  ([`unverified-boundaries.md`](./unverified-boundaries.md) #20)
- Make it a convention that a `Condition` implementation is a **pure function** (no
  external I/O, clock or randomness)
- Require a condition to be defined once as a named type, so it can be identified
  as a subject for review and testing

**It must not be described as "guaranteed by types".**

---

## The `Condition` trait

```rust,ignore   // needs a crate or a verum-private module this harness does not carry
pub trait Condition<Domain, Request>: derive_facing::SealedCondition<Domain, Request> {
    const NAME: &'static str;
    fn holds(domain: &Domain, req: &Request) -> bool;
}
```

### The limits of being synchronous and pure

These cannot be expressed:

- **Feature flags / A-B tests** — they need an asynchronous query to an external
  service
- **Conditions depending on the clock or a rollout percentage** — unreachable from
  the domain or the request

Extending to `async fn holds(ctx: &Ctx<..>, ..)` would permit external I/O inside
the capability boundary, and its consistency with the effect system would have to
be reconsidered. Recorded in
[`research-questions.md`](./research-questions.md).

---

## Composing conditions (undecided)

```text
when(EmailChanged and NotSuspended)
when(EmailChanged or PhoneChanged)
when(not Verified)
```

- `and` → the union of both conditions' effects, or separate declarations?
- `or` → which one held is unknown at run time, so only the union can be permitted
- `not` → may run into the negative-reasoning problem

The First PoC handles single conditions only.

---

## Nested conditions

Structurally possible, but the declaration form on the contract side is
undecided. Readability degrades fast, so **limiting nesting to two levels** is
under consideration. Beyond that, compose conditions.

---

## AI Context output

```json
{
  "mutates": {
    "unconditional": ["User.name"],
    "conditional": [
      { "condition": "EmailChanged", "fields": ["User.email"] }
    ],
    "effective": ["User.name", "User.email"],
    "enforcement": {
      "level": "upper_bound_checked",
      "scope": "handle_via_ctx",
      "voided_by": [
        "domain_repr", "domain_swap", "repository_impl", "unscanned_effect",
        "middleware", "constructor_body", "malformed_set",
        "upsert_granularity", "event_subscriber"
      ]
    }
  },
  "conditional": [
    {
      "condition": "EmailChanged",
      "condition_defined_at": "src/conditions/user.rs:12",
      "condition_verified": false,
      "mutates": ["User.email"],
      "emits":   ["EmailVerificationRequested"],
      "calls":   ["EmailService"],
      "enforcement": {
        "level": "upper_bound_checked",
        "scope": "handle_via_ctx",
        "voided_by": ["repository_impl", "unscanned_effect", "middleware",
                    "constructor_body", "malformed_set", "event_subscriber"]
      }
    }
  ]
}
```

An AI has to be able to distinguish three things:

1. What always happens (`unconditional`)
2. What happens depending on a condition (`conditional`)
3. That the condition itself is not trustworthy (`condition_verified: false`)

---

## Priority

`when` is not implemented in the First PoC. But **the rule for where things are
declared is implemented in the macro from the start** — changing it later means
rewriting every contract.

Emitting `condition_body` into `unverified_boundaries` is included from the First
PoC too (while `when` is unimplemented, the entry is simply empty).
