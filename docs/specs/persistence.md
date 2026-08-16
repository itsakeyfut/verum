# Persistence

The scope of the persistence layer (database access) and its trust boundary.

Related: [`mutation-contract.md`](./mutation-contract.md),
[`unverified-boundaries.md`](./unverified-boundaries.md),
[`effect-inference.md`](./effect-inference.md).

---

## Decision: provide the repository trait only

Verum **does not know about databases**. It defines the repository trait only;
the implementation is written by the user, with sqlx or anything else.

```rust
// what Verum defines is the trait and the capability constraints, nothing else
pub trait UserRepository {
    async fn find(&self, id: UserId) -> Result<User>;
    async fn set_email(&self, u: &mut User, v: Email) -> Result<()>;
    async fn set_name(&self, u: &mut User, v: String) -> Result<()>;
}
```

> **Capabilities are not passed as arguments.** An earlier version took
> `cap: &Cap<Mutate<User, user::Email>>` as a parameter. That contradicts the
> policy that capabilities are never materialised as values but expressed as type
> parameters of `Ctx<'req, E>` ([`rust-type-model.md`](./rust-type-model.md)), so
> it was removed.
>
> The capability check happens in the where clause of `Repo<User, R, M>`'s
> extension trait.

```rust,ignore   // fragment, not a complete item
// the extension trait the derive generates — the capability check lives here
pub trait UserRepo<M> {
    fn set_email<I>(&self, u: &mut User, v: Email) -> Result<()>
    where M: Has<Mutate<User, user::Email>, I>;
}

impl<R, M> UserRepo<M> for Repo<User, R, M> { ... }
```

`Repo<D, R, M>` is the only exposed surface on which capabilities are checked;
`UserRepository` sits inside it as a plain persistence trait.

### Why

The same criterion as [`runtime-stack.md`](./runtime-stack.md):

> **Do not build a concept whose specification is already settled. Put every
> resource into the unsolved design problems.**

SQL generation and query building are solved problems, and an area sqlx / SeaORM
/ Diesel have invested in for years.

---

## Interoperating with domain opacity

A domain is exposed as an opaque type with private fields
([`mutation-contract.md`](./mutation-contract.md)), while sqlx's `query_as!`
requires `pub` fields.

The derive generates a `pub(crate)` `Repr`.

```rust,ignore   // fragment, not a complete item
// macro-generated (derive or attribute is undecided — see "What is undecided" below)
pub(crate) struct UserRepr {
    pub id:    UserId,      // pub is required: query_as! expands to a struct literal at the call site
    pub name:  String,
    pub email: Email,
    // Do not derive Debug / Clone / Serialize — ledger paths 3 / 4 come back through Repr
}

// The domain must own a borrowable Repr (a newtype is one way to do that)
pub struct User(UserRepr);   // the inner field is private, not pub(crate)

impl User {
    // No visibility modifier — NOT pub(crate). Confined to this module, which is
    // what narrows ledger path 21 (#33 / ADR-0010). pub(crate) would mean
    // "every handler in the application".
    fn from_repr(r: UserRepr) -> Self;
    fn as_repr(&self) -> &UserRepr;
}

// The repository is generated INTO THIS MODULE, beside the domain. That is the
// half that makes the confinement usable rather than merely restrictive: it is
// the only legitimate caller, and it is inside.
impl UserRepository for PgUserRepository {
    async fn find(&self, id: UserId) -> Result<User> {
        let repr = sqlx::query_as!(UserRepr, "SELECT * FROM users WHERE id = $1", id)
            .fetch_one(&self.pool).await?;
        Ok(User::from_repr(repr))
    }
}
```

> The `Repr` above is shown `pub(crate)` because that is what `query_as!` has been
> measured to need at the call site. Where the repository is generated beside the
> domain the call site is *this* module, so the `Repr` can carry no modifier either
> — which also closes ledger paths 3 and 4 through it (P30). See ADR-0010.

### Verdict (T-M1-01 / #13, extended by #33. **37 probes, compile-verified**)

Reproduce with `spikes/domain-opacity-sqlx/` (`bash run.sh` →
`37 as specified, 0 unexpected`). **Run it on an otherwise idle checkout**: a
concurrent `cargo` (an IDE checker, a second shell) has been observed to produce a
spurious `UNEXPECTED` row, because the `touch` that defeats caching runs once at the
top rather than per probe.

**The probe table is this section's canon.** This verdict was reviewed twice, and
**the table was right both times while the prose was wrong both times** — six
generalisations beyond what was measured. What follows is limited to what the
table established.

1. **sqlx interoperation works.** The shape above compiles and runs.
2. **"the trust boundary is the repository implementation" does not hold as
   written.** The macro expands in the user's crate, so `pub(crate)` means the
   whole application crate. In the same crate, any handler can assemble a domain
   from arbitrary values; in a different crate, `Repr` is invisible (`E0603` +
   `E0624`). → ledger **path 21**

   This originally continued "…and the macro cannot emit `pub(in ...)` because it
   does not know where the repository lives", offered as the reason the boundary
   could not be narrowed. The premise is true and the conclusion does not follow:
   **the derive does not need `pub(in ...)`.** Emitting no modifier at all confines
   the constructor to the domain's own module, and emitting the repository into
   that module keeps it callable. Measured in #33 — see below.
3. **The generated shape is undecided.** A derive **cannot add an item with the
   same name as its input** (`E0428`), so `pub struct User(UserRepr)` is not
   something `#[derive(Domain)]` can emit.

**Field-level mutation's type enforcement itself is intact.** What broke is
neither sqlx nor the type enforcement, but `Repr` as a sideways bypass.

#### The shape the macro must preserve (measured)

| Constraint | If violated |
|---|---|
| Do not make `Repr`'s fields **fully private** | `query_as!` expands to a struct literal at the call site, so `E0451` (`pub(crate)` is enough within the crate) |
| Do not derive `Debug` / `Clone` / `Serialize` / `Deserialize` on `Repr` | Ledger paths 4 / 3 come back through `Repr` (**from inside the same crate**; in the specified shape an external crate cannot reach `as_repr` — `E0624`) |
| The domain's inner field is private (not `pub(crate)`) | `u.0.email = v` compiles from anywhere in the crate |
| The domain **owns** a borrowable `Repr` | An `as_repr` returning a temporary is `E0515`. **A newtype is one way to satisfy this, not a requirement** |

**The guarantee is "from outside the defining module", not a type boundary.**
From the defining module and its children, `u.0.email = v` compiles. The macro
expands in the same module as the user's `struct User`, so code written next to
it stands on the permissive side. The error code also varies with the shape: a
newtype plus getter gives `E0615`, no getter gives `E0609`, and only a flat
private named field touched from outside the module gives `E0616`. **`E0615` and
`E0609` cannot be replaced with `#[diagnostic::…]`** — those attributes attach
only to trait definitions and trait impls (measured).

#### Undecided — settled in #34 (the Domain macro form)

> This section was headed "settled in #17 / #18" until #33. Both are closed and
> **neither settled any of the three**, which are all consequences of the macro's
> shape; #34 is where that is decided. How path 21 is *closed* — the other thing
> the old heading was read as promising — is settled below, in #33.

1. **Which macro shape.** `as_repr(&self) -> &Repr` has several shapes a derive
   can satisfy (the user writes the newtype and the derive emits only `Repr`; or
   `Repr` becomes a type alias for the domain — both measured to compile). So
   "the signature and a derive are incompatible" is **not something that was
   measured.** What was measured is `E0428` alone. Moving to an attribute macro
   loses no layer-1 check (measured), but the description of `Domain` as a derive
   propagates to **15 files / 23 places**, two of which are the bodies of issues
   already filed. The versioning impact is effectively zero (`verum-macros`
   generates nothing today).
2. **Who attaches `#[derive(sqlx::FromRow)]`.** A user cannot add a derive to a
   generated item (measured). Pass-through
   (`#[domain(repr_derive(sqlx::FromRow))]`) has been **implemented and confirmed
   to work** — in that shape the generated derive resolves in the user's crate, so
   `verum-macros` does not depend on sqlx. Only the option "verum emits
   `sqlx::FromRow` unconditionally" contradicts the dependency table.
3. **The enforcement level of rejecting `pub` fields depends on choice 1.** In
   the attribute shape the macro consumes the input's `pub`, so it is a **lint**;
   in the derive-plus-flat shape the user's `pub` is real, so it is a
   **guarantee**. A change of enforcement level is defined as breaking.

#### Alternatives (all measured. **None improves on the status quo**)

| Alternative | Measured |
|---|---|
| Put `Repr` in a dedicated module and rely on module privacy | **Worse.** Making it a `pub` type inside a private module suppresses `E0446`, which opens the trait route: an external crate can read every field through a projection and forge one (zero warnings even under `-D warnings`) |
| Make `Repr` `pub` with private fields | **Does not hold the boundary.** It loads, a struct **literal** is `E0451`, `query_as!` is lost, and **it is still forgeable** (`FromRow` assembles it from rows the caller supplies) |
| Put the `Repr` conversion on a trait in `verum` | `E0446` while `Repr` is `pub(crate)`. Making it `pub` opens it, and brings the projection bypass above along with it |
| **A sealed token** | **Not a boundary** (retracted; this was previously written here as "the only surviving candidate"). A token can only be passed **as an argument to a trait the user implements**, so a handler writing a three-line `impl Repository` receives the token from verum (measured). By value it cannot express a multi-row load (`E0382`) and forces `Copy`, and once `Copy` it can be stashed in a static |
| Hand-write `FromRow` / abandon opacity | **Not measured** |

The general form is: **any derive-produced constructor that assembles the struct
inside the defining module is a forging route** — `FromRow`, `Deserialize`, and
any future derive. `E0451` closes one syntactic form only, and **opening `Repr`
enough to be usable opens it enough to be forgeable.**

#### How path 21 is closed (#33, measured — ADR-0010)

**The derive emits the `Repr`, the constructor and the repository into a
derive-owned private module, and re-exports the domain type from it.**

```rust,ignore   // fragment, not a complete item
mod __verum_user {
    pub struct User(UserRepr);
    struct UserRepr { .. }                        // module-private
    impl User { fn from_repr(r: UserRepr) -> Self { .. } }   // no modifier
    pub struct UserRepository;                    // the only legitimate caller
}
pub use __verum_user::{User, UserRepository};
```

| Caller | Probe | Outcome |
|---|---|---|
| The generated repository, inside the module | P28 | loads a real row — the design works |
| A handler elsewhere in the crate | P31 | **`E0624`** |
| A helper written **next to the user's own declaration** | P31 | **`E0624`** |
| The domain declared at the **crate root** | P32 | **`E0624`** |
| `as_repr`, the read half | P34 | **`E0624`** |
| A `Debug` leak through the `Repr` | P35 | **`E0624`** — no accessor hands one out |
| A foreign crate | P27 | **`E0624`** |

**Confining it to the *user's* module instead is not enough**, and that was this
spec's first answer. Two holes, both measured: a helper written beside the user's
own `struct User` forges (P29), and if the domain is declared at the **crate root**
then "no visibility modifier" *is* `pub(crate)`, so the mechanism buys nothing at
all (P33). A derive-owned module has neither property, because its radius is chosen
by the derive rather than by the code being guarded.

**The conversion must stay an inherent method.** A trait method's visibility is the
**trait's**, not the impl module's, so putting the conversion on a public framework
trait defeats all of the above from any crate (P36). This forecloses the obvious
shape of a generic persistence layer; see #39 / #40.

**What this costs.** Everything touching the `Repr` must be generated — a
user-written `impl UserRepository for PgUserRepository` outside the module cannot
reach `from_repr`. The code block at the top of this section still shows that
user-written shape; reconciling the two is #39 / #40's decision.

#### The diagnostic constraint on how it is closed (measured)

**Close it with visibility and the diagnostic can never carry wording.**
`E0603` / `E0615` / `E0609` / `E0616` are field- and path-resolution diagnostics
that do not go through trait resolution, so `#[diagnostic::…]` never reaches them.
That is the cost of the decision above, accepted knowingly.

**The trait-bound escape produces the wording and does not hold.** This section
previously said a trait bound gives `E0277` with Verum's own `message` and `label`,
and that "only the latter satisfies `CLAUDE.md`'s non-negotiable". The first half is
**true and measured** (P37 renders the message and the note). The second does not
follow, because the bound is **forgeable**: `impl verum::RepositoryProof for
MyProof {}` is a foreign trait on a local type, which the orphan rules permit from
the application crate (P24) and from any other crate (P25). Worse, rustc's own
`help:` on that `E0277` points at the user's type and says the trait is not
implemented for it — coaching the two-line bypass.

So the wording is **reachable and worthless**. #33's requirement 1 (nothing outside
may forge) and requirement 2 (the rejection carries Verum's wording) are **jointly**
unsatisfiable; neither is individually impossible.

**Generalised (ADR-0010):** `verum` can never own the constructor's *body*, because
the domain's fields are private to the user's crate. Construction code therefore
always lives there, and only **placement** can restrict which code runs it — a bound
merely gates entry to code that already sits where privacy has granted access.

**ARK-002 is not satisfied, and that is an open risk rather than a solved problem.**
`rustc --explain E0624` names both bypasses by number, and the error emits no
pointer to the generated repository. Recorded in
[`diagnostics.md`](./diagnostics.md).

---

## The trust boundary — the repository implementation

**This decision makes the repository implementation the trust boundary.**

```rust,ignore   // needs a crate or a verum-private module this harness does not carry
impl UserRepository for PgUserRepository {
    async fn set_email(&self, u: &mut User, v: Email) -> Result<()> {
        sqlx::query!(
            "UPDATE users SET email = $1, status = 'verified' WHERE id = $2",
            //                            ^^^^^^^^^^^^^^^^^^^ an undeclared mutation
            v, u.id()
        ).execute(&self.pool).await?;
        Ok(())
    }
}
```

That violation **cannot be detected by Verum**. The capability type check reaches
only as far as whether a method may be called; it does not reach the SQL inside
the method's implementation.

### This is not a defect but a stated boundary

```text
Endpoint / service layer  → guaranteed by types
Repository implementation → trust boundary (subject to review and audit)
DB                        → out of scope
```

> **⚠️ The first line above does not hold today** (measured in T-M1-01 / #13).
> Ledger **path 21** is **narrowed, not closed** (#33 / ADR-0010). With the
> constructor confined to the domain's own module, endpoint and service code can no
> longer forge (`E0624`, P26) and neither can another crate (`E0603`, P27) — but a
> helper written beside the user's own `struct User` still can (P29). The diagram
> describes everything outside that one module.

**It is emitted in the AI Context as `unverified_boundaries`**
([`unverified-boundaries.md`](./unverified-boundaries.md)). What would be a defect
is failing to document where the boundary sits.

---

## Ways to narrow the trust boundary

### 1. Enforce one field = one method

A consequence of [`handler-rules.md`](./handler-rules.md) Rule 1. Each method
touches a single column, so the implementation is a few lines and easy to review.

### 2. Generate the repository implementation from a derive (priority raised)

```rust,ignore   // needs a macro that arrives in M2
#[derive(Repository)]
#[repository(domain = User, table = "users")]
pub struct PgUserRepository { pool: PgPool }
// → set_email / set_name / find generated
```

A generated implementation follows the contract by construction, so **the trust
boundary moves inside Verum.**

> **Measured in #33: this is necessary but not sufficient, and it was written here
> as if it were sufficient.** Generating the repository changes who *should* call
> `from_repr`; it does not change who *can*. Probe P2 is the demonstration — the
> spike's `app/src/repo.rs` is a repository, and a handler forges anyway. What
> makes generation close anything is the second half of ADR-0010: the repository
> must be emitted **into the domain's own module**, so the constructor can carry no
> visibility modifier and still have a legitimate caller.

> **Important**: generating the repository **trait definition** is needed too.
> While `set_<field>` is hand-written per field in both the trait and the impl:
>
> 1. Every new domain costs boilerplate proportional to its field count (the
>    token-efficiency claim collapses on the *writing* side)
> 2. **Writing `user::Name` by mistake in `set_email`'s where clause is not
>    detected** — the claim "rustc does the matching for us" ends up resting on the
>    weak premise that the hand-written boilerplate is correct
>
> This was originally deferred to "later"; because of point 2, **generating the
> trait definition is moved ahead of generating the impl.**

### 3. Make raw SQL an explicit escape hatch

Complex queries go through an escape hatch and are recorded in the contract.

```text
escape_hatch: raw_sql
  reason: "complex aggregation across users and orders"
```

> **Note**: the recording is **self-reported** today. Forget the attribute and
> nothing is recorded. `escape_hatches: []` must not be read as "no escapes". If
> the low-level API requires a ZST proof produced by an attribute macro as an
> argument, a missing record becomes structurally impossible. Where that is not
> achievable, emit `"unknown"`.

---

## Rejected options

### A typed query builder

It would carry the prevention of undeclared mutations all the way into the DB
layer, but it is rejected. SQL generation is building a solved problem, and it
takes time away from the type design. Complex queries tend to fall back on an
escape hatch anyway.

### A static check (lint) on UPDATE statements

An intermediate answer that pierces the boundary with a smaller implementation
than a query builder, but it needs SQL parsing, is powerless against dynamic SQL,
and becomes sqlx-specific. **Starting it before the type design has settled is
premature.** Revisit once the type design is complete.

---

## Open problems

### Transactions

- Is endpoint = one transaction the standard?
- Is the atomicity of several mutations expressed in the contract?
- **Can firing an external effect inside a transaction be forbidden in types?**
  - [`handler-rules.md`](./handler-rules.md) Rule 4 proposes the
    `ctx.after_commit` scope, but it has to be merged with the transaction
    boundary design
- Savepoints and nested transactions
- **The semantics of partial failure** — a contract is an upper bound, so "only a
  subset of the declared effects happened" is not expressible. An AI reading
  `emits: [UserUpdated]` concludes "if it was updated, the event is always
  emitted", but the converse — the event fires and the update does not — happens
  too

### Optimistic and pessimistic locking

- Per-field setters cannot express `WHERE id=? AND version=?` compare-and-swap as
  an atomic operation
- There is no `Lock<Domain>` effect corresponding to `SELECT ... FOR UPDATE`

### Listing, aggregation, JOIN, N+1

Consequences of `Read<Domain, Field>` assuming a single instance. No repository
API other than `find(id)` exists in the specification.

See [`research-questions.md`](./research-questions.md).
