# Verum — the boundary of the public API

> **This is Verum's most important set of rules.** What is kept off the public
> surface decides both the framework's lifespan and whether the capability system
> holds at all.
> The canon of the design is
> [`../specs/runtime-stack.md`](../specs/runtime-stack.md) (the Dependency Hiding
> Rule) and [`../specs/capability-system.md`](../specs/capability-system.md)
> (sealed traits).

---

## 1. The Dependency Hiding Rule (most important)

> **Not one type from a replaceable dependency appears in the `verum` crate's
> public API.**

Hiding is what buys the freedom to step off later. Hide from the start and
dropping Axum changes nothing in the public API. Expose it and it can never be
dropped.

### Hidden (replaced by a Verum type)

```text
axum::extract::State        ← the most important; see below
axum::Router
axum::response::IntoResponse
axum::Json
axum::extract::Path / Query
axum::handler::Handler
tower::Service / Layer
hyper_util::*
matchit::*
tokio_tungstenite::*
```

### Not hidden — re-exported, in fact

```rust,ignore   // needs a crate or a verum-private module this harness does not carry
pub use http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri};
```

These are not Axum-specific; they are the **stable http 1.x** foundation, and
the hand-written runtime will use exactly the same ones. Hiding them would
achieve nothing.

`http-body` 1.x is equally stable, so parts of the body types may be exposed.

### Why `State` matters most

`axum::extract::State<T>` can produce anything. Exposing it makes "capabilities
you did not declare cannot be obtained" a lie and **breaks the capability system
at the root.**

Axum's `Handler` trait is the same: it accepts any number of arbitrary
extractors. That is not an escape hatch but **an undeclared bypass**
([`../specs/unverified-boundaries.md`](../specs/unverified-boundaries.md)).

### Code touching Axum lives in one module

```text
src/runtime/          ← the only place that touches Axum
    mod.rs
    axum_backend.rs
```

**Do not create a backend trait yet.** A trait with a single implementation gets
in the way later. Split it out when a second one (the hyper backend) is needed —
at that point there are two implementations and the abstraction is justified.

### Verified mechanically in CI (wired in T-M0-04)

`.github/scripts/check-api-boundary.sh` has two modes, run as separate CI jobs.
Both work locally as-is.

```bash
.github/scripts/check-api-boundary.sh imports      # what may be written where in the source
.github/scripts/check-api-boundary.sh public-api   # what has reached the public surface
```

**Neither alone is enough.** Measured with `cargo-public-api` 0.52.0:

| How it is written in source | How it appears in the public API | A naive `axum::` grep |
|---|---|---|
| `pub fn f() -> axum::Router` | `axum::routing::Router` | Detected |
| `pub fn g<T: axum::response::IntoResponse>` | **`axum_core::`**`response::into_response::IntoResponse` | **Missed** |
| `pub use axum::Router;` | `pub use verum::Router` | **Missed — the origin is gone** |
| Any `pub struct` (with hyper-util in the dependency tree) | `impl<A,B,T> `**`hyper_util::`**`...HttpServerConnExec for verum::Thing` | **False positive** |

Four consequences follow.

1. **Write the deny list with real crate names, not façades.** `IntoResponse`
   lives in `axum_core`, and it is one of the two types this section calls "most
   important". A list containing only `axum` lets it straight through.
2. **`--omit blanket-impls,auto-trait-impls,auto-derived-impls` is mandatory.** A
   dependency's blanket impl injects a forbidden crate name into the line of
   *every* public type, so without this the check becomes useless the moment
   hyper or tower enters the tree.
3. **`cargo public-api` cannot see through a re-export.** The source-side grep is
   not an extra; it is the only thing closing that route.
4. `http::` and `http_body::` are permitted by being absent from the deny list.
   No allow-list mechanism is needed.

The grep side has two rules.

- **(a)** Naming anything in the `axum` family outside
  `crates/verum/src/runtime/` is forbidden ([design.md](./design.md) §2, the
  module boundary).
- **(b)** A `pub use` of a forbidden crate is forbidden **everywhere, including
  inside `runtime/`**, and that includes a leading `::`
  (`pub use ::axum::Router;`). Without this, "re-export in `runtime/`, re-export
  again in `lib.rs`" **passes both checks** — by the time it reaches the public
  API the origin is gone.

The leading `::` in rule (b) was missing initially and was caught in review.
`::axum::Router` is not obfuscation but ordinary Rust, and since rule (a) permits
axum inside `runtime/`, it **passed both rules in plain sight.**

> **A route that remains open**: inside `runtime/`, write
> `use axum::Router as R;` (legal under rule (a)) and then `pub use R;` (no
> `axum::` string, so rule (b) does not match). `cargo public-api` cannot detect
> it either, having lost the origin.
>
> Closing it needs something like restricting `pub use` inside `runtime/` to
> `self::` and `crate::` — but that **builds a false-positive surface against a
> `runtime/` that does not exist yet** (tokio is a permitted crate, so a
> legitimate `pub use tokio::signal;` could be rejected). Going through an alias
> is not the shape an accident takes, so this is revisited when the runtime is
> implemented.
>
> A multi-line `pub use` (with `pub use` and `axum::Router;` on separate lines)
> also escapes rule (b), which is line-based — but **rustfmt has been confirmed
> to fold it onto one line**, and the `fmt` job blocks, so the form cannot exist
> in anything that passes CI.

### This check does not disappear in Phase 12 (removing Axum)

**What disappears is the axum entries and rule (a)'s target; the file stays.**
Phase 12 moves to `hyper-util` plus a hand-written router, and `hyper_util`,
`tower` and `matchit` are **already on the deny list.** The check does not lose
its subject; the subject moves from axum to what replaces it.

This section rests on two grounds, and only one of them lapses.

| Ground | After Phase 12 |
|---|---|
| **Replaceability** — "hiding buys the freedom to step off later" | Lapses for axum. The freedom to swap hyper/tower later remains |
| **Capability completeness** — no `State`-equivalent entry point is public | **Permanent.** `tower::Service` takes any request and returns any response, so it routes around the contract for exactly the same reason `axum::extract::State` does |

| Target | Treatment in Phase 12 |
|---|---|
| `axum` / `axum_core` / `axum_extra` / `axum_macros` | Removed — the dependency is gone and the entries are dead |
| `tower*` / `hyper` / `hyper_util` / `matchit` / `sqlx*` | Kept. `hyper_util` becomes *more* important, being the destination |
| Rule (a)'s target | Replaced, not removed. The rule is really "only `runtime/` touches the HTTP backend", so the target moves to the new crates |
| The `public-api` mode and the guard's own tests | Structurally unchanged; only crate names in the list and the fixtures |

> **Careful**: the script's header says "the check that buys the freedom to drop
> Axum later" and the variable is called `AXUM_ROOTS`. Phase 12's task is named
> "remove Axum", so **there is a real risk of mistaking this for cleanup and
> deleting it wholesale.** Deleting it fails nothing, and the guard for capability
> completeness quietly disappears. The same note is at the top of the script.

### Test the guard itself

`.github/scripts/check-api-boundary-test.sh` pins **both the inputs that must
fail and the inputs that must pass** for `imports` mode, with fixtures. For the
same reason [test.md](./test.md) §2 requires a `pass` to accompany every
`compile_fail`: **with one side only, an implementation that rejects everything
looks healthy.**

Both bugs above — the leading `::` and the empty scan — were real, found in
review, and are kept as test cases. The `boundary` job runs this test before the
guard itself.

### Pinning nightly

`cargo public-api` needs nightly because it consumes rustdoc JSON. **The JSON
format is unstable**, and the range of nightlies a given release understands is
narrow — eight days for 0.46.x. The script pins both the nightly date and the
`cargo-public-api` version, and **the two are always raised together.**

> The escape hatch in §6 is designed to return `&hyper::upgrade::Upgraded`, which
> this check will flag once implemented. That is correct behaviour, but the way
> to permit it has to be decided at implementation time.

---

## 2. Sealed traits

The following traits are **always sealed** — given a private supertrait.

| Trait | What happens if it is not sealed |
|---|---|
| `Endpoint` | A hand-written impl can declare any capability |
| `Has` | Membership can be forged |
| `Includes` | `impl Includes<Order> for User` passes the orphan rule and the Architecture Contract disappears |
| `Field` | Forging `Field::NAME` fakes the column name in generated SQL |
| `Condition` | The condition trait itself can be forged |
| `ConsList` | The shape proof can be forged, and a malformed set passes as well-formed (sealed in T-M0-07) |
| `Index` | The position can be forged (sealed in T-M0-07) |
| `Append` | **The concatenation result can be forged. Having a `type Out`, it names the composed capability set itself** (sealed in T-M0-09; dropping the base impl's bound actually opened a hole) |
| `Lookup` | **"This is the entry for that key" can be forged, substituting a conditional scope** (sealed in T-M0-09) |

### The implementation pattern

```rust,ignore   // fragment, not a complete item
// src/sealed.rs — seals are declared by a macro (see "one seal per trait" below)
pub(crate) mod private {
    macro_rules! seal { /* generates a pub trait, always with on_unimplemented */ }

    // structural seals — verum implements these on tuples itself; no derive involved
    seal! { SealedConsList }
    seal! { SealedIndex }
    seal! { SealedHas<T, Idx> }
    seal! { SealedAppend<B> }
    seal! { SealedLookup<K, Idx> }
}

// seals a derive has to satisfy go in a **separate module**. M2 exposes only this
// one (see "split the seals into two modules" below)
pub(crate) mod derive_facing {
    seal! { SealedEndpoint }
    seal! { SealedIncludes<D> }
}

// src/endpoint.rs — a derive satisfies this seal, so it lives in `derive_facing`
pub trait Endpoint: derive_facing::SealedEndpoint {
    type Method;
    // ...
}

// src/domain.rs — a trait with type parameters gets a seal carrying them
pub trait Includes<D>: derive_facing::SealedIncludes<D> {}
```

The derive emits `impl derive_facing::SealedEndpoint for UserEndpoint {}`. Users
cannot reach the `private` module, so they cannot write the impl by hand.

> **Unresolved (found in T-M0-06)**: a proc macro's output is **resolved in the
> calling crate**, so it cannot reach `pub(crate) mod private`. A derive was
> written and **E0603** confirmed. "The derive emits it", above, **cannot be
> implemented as things stand.** M2 will need a `#[doc(hidden)] pub mod __private`
> re-export, and **that is the moment the seals get weaker**, so settle its shape
> together with the type-parameter question below.

### Why this is mandatory

By design, Verum **shows an AI a great many trait-bound errors.** And the first
fix an AI tries for a trait-bound error is **writing the missing impl.**

```rust,compile_fail
// the "fix" an AI might write — this compiled before sealing
impl Includes<Order> for User {}   // ← the Architecture Contract disappears
```

`User` is a local type, so the orphan rule permits it, `cargo build` succeeds and
the AI Context reports nothing. **One line used to void the guarantee.** Today
`Includes` is sealed and this is rejected with `E0277` (verified on every run by
`spikes/doc-code-blocks`); the block records the pre-sealing state.

**Sealing takes a few lines, but applying it later is a breaking change.** It is
mandatory for the First PoC.

### The module is required (measured)

Writing `pub(crate) trait Sealed` **without a module** fails the moment it
becomes a public trait's supertrait.

```text
error: trait `Sealed` is more private than the item `Includes`   // private_bounds
```

**The nesting is what makes the visibility work**:
`pub(crate) mod private { pub trait Sealed {} }`. It is not a matter of style.

### `Sealed` alone does not pass the lints (measured)

With no `pub trait X: private::Sealed<..>` using it, two errors appear.

```text
error: trait `Sealed` is never used          // dead_code
error: unreachable `pub` item                // unreachable_pub
```

One such trait makes both disappear — the `pub` becomes reachable through the
supertrait. **`Sealed` cannot be laid down on its own as "groundwork to apply
later".**

### A seal must carry the sealed trait's type parameters (most important, measured)

**Sealing `Self` alone is not enough.** A bare `Sealed` supertrait constrains
only *which types* may implement, not **with which arguments.**

```rust,compile_fail
// ❌ guards Self only
pub trait Includes<D>: private::Sealed {}
```

The instant a derive emits one `impl Sealed for GetUser`, **forging an
undeclared domain compiles** (measured).

```rust,ignore   // needs a crate or a verum-private module this harness does not carry
impl verum::__private::Sealed for GetUser {}   // emitted by the derive (legitimate)
impl verum::Includes<Order>   for GetUser {}   // declared
impl verum::Includes<Secrets> for GetUser {}   // forged — and it compiles
```

`User` is a local type, so the orphan rule permits it. **The attack this section
calls "one line voids the guarantee" recurs, delayed by exactly one derive.**

```rust,ignore   // needs a crate or a verum-private module this harness does not carry
// ✅ seal the relation itself
pub trait Includes<D>: derive_facing::SealedIncludes<D> {}
```

The derive emits `Sealed<Order>` **only for declared domains**, so
`Includes<Secrets>` has no `Sealed<Secrets>` and is `E0277` (confirmed in both
directions).

> **Rules for any future sealed trait**
> - The seal carries the trait's type parameters unchanged. `Has<T, Idx>` gets
>   `SealedHas<T, Idx>`, not `SealedHas`.
> - **One seal per sealed trait** (below). Sharing lets one trait's derive unlock
>   another.
> - A trait without type parameters (`Endpoint` and similar) gets a
>   parameterless seal (`SealedEndpoint`).
> - **A seal's impl set must never be looser than the trait's** (below; a second
>   recurrence in T-M0-08).

### A seal's impls must not be looser than the trait's — mirror the recursion too (most important, measured)

Carrying the type parameters is not enough. **The impl structure has to be
mirrored as well.** The §"type parameters" above came from #6; this came from #8
— **the same failure, a second time.**

In T-M0-08 the seal's recursive impl for `Has` was unconditional.

```rust,compile_fail
impl<H, X, T, I> private::SealedHas<H, There<I>> for (X, T) {}          // ❌ H and the recursion both unconstrained
```

```rust,ignore   // fragment, not a complete item
impl<H, X, T: private::SealedHas<H, I>, I> private::SealedHas<H, There<I>> for (X, T) {}  // ✅
```

The ❌ form **holds for every two-element tuple**, so downstream could forge
membership at any position but the head (measured — pasting the `help:` line out
of the `.stderr` was enough).

### ⚠️ The rule above **failed to prevent a third recurrence in #9**. Here is the principle, re-derived

#6 (type parameters), #8 (the recursion) and #9 (a bound on a parameter that does
not appear in `Self`) shipped the same class three times. **The mistake was
widening the rule each time to cover the previous instance**; #9 opened a hole
while complying with §2 as it then stood. Derive it from the principle instead.

#### Premise: only three things constrain a downstream impl

| Mechanism | Constrains downstream? |
|---|---|
| A bound on the trait declaration (`pub trait Append<B: ConsList>`) | ✅ Yes. But **call sites end up restating the bound** (measured — a generic consumer becomes E0277), which collides with [type-level.md](./type-level.md) §1 |
| **The seal** (a supertrait obligation) | ✅ Yes, **at zero cost to use sites** |
| The orphan rule | ✅ Yes, but one local type anywhere is enough to pass |
| **A bound on verum's own impl** | ❌ No — **and worse, it works against safety** (below) |

**The last row is #9's cause.** The `B: ConsList` in
`impl<B: ConsList> Append<B> for ()` was being read as a guarantee, and it is not
imposed on a foreign impl at all. The comment said "removing it stops verum
building, so it is load-bearing" — **true, and irrelevant to safety.** Conflating
the two leaves a hole while feeling verified.

> ### ⚠️ A stronger correction: that bound is not neutral — it **opens the window**
>
> The first version of this table read as though "does not constrain" meant
> neutral. Measured, it is **the opposite.** Removing the bound from the upstream
> impl makes downstream forgery fall to coherence:
>
> | Upstream impl | Downstream `impl Op<Local> for ()` |
> |---|---|
> | `impl<B: Shape> Op<B> for ()` | **compiles** |
> | `impl<B> Op<B> for ()` | **rejected, E0119** |
>
> With the bound, the obligation at the intersection becomes unsatisfiable, the
> overlap check declares the impls disjoint and **stands aside.** So `B: ConsList`
> is needed for verum's own correctness and **opens a forgery window at the same
> time.** That is why the same bound must appear on the seal — #9 was not
> forgetting to add something neutral, it was **forgetting to close a window that
> had been opened.**

#### The principle: a seal's solution set equals the trait's

> If any type satisfies `Sealed` and not `Trait`, **downstream can fill in
> precisely that difference.** In other words:
>
> **the difference set is the attack surface.**

So every bound that carries enforcement goes **on the seal** — a bound on the
trait declaration costs the call site. A difference may be left deliberately only
when **the difference set can be proved harmless and that proof exists as a
test.**

**No difference is allowed for a trait with a `type Out`.** A predicate trait's
head impl (`Has`) pins the very fact it asserts — `H` must be `Self`'s head — so
the remaining looseness admits only *true* assertions. A trait with an associated
type pins nothing: **the forger chooses the output.** #9 left exactly this
difference and it was exploited in one line.

> This is a **policy**, not a claim about the compiler. A difference the orphan
> rule cannot reach can be harmless even with a `type Out` (measured: with a trait
> whose difference is only `{u8}`, the downstream impl is E0117). Read it as an
> operational rule — **"that justification will not be accepted"** — because
> asserting an unverified fact about the compiler is exactly the failure this
> section is fixing.

#### Split the seals into two modules by "must a derive satisfy it?" (found in #9's review)

This section says above that M2 "will need a `#[doc(hidden)] pub mod __private`"
and that "this is the moment the seals get weaker". **That warning was attached
only to path 13.** With every seal in one module, that single change makes
**every seal nameable**, reopening ledger paths 14a–14e at once (the change was
made and forged membership was confirmed to compile).

| Module | Contents | Visibility |
|---|---|---|
| `private` | **Structural** seals — `SealedConsList` / `SealedIndex` / `SealedHas` / `SealedAppend` / `SealedLookup`. verum implements them on `()` and tuples itself, and **a derive never writes one** (confirmed against M2's task definitions) | **Permanently** `pub(crate)`. M2 does not affect it |
| `derive_facing` | Seals a derive satisfies per declaration. Currently `SealedIncludes` only; M2 adds `SealedEndpoint` / `SealedField` / `SealedCondition` | M2 has no choice but to expose it as `__private` |

`compile_fail/sealed_derive_facing_module_is_private.rs` pins the current
non-exposure, so M2 opening it appears as a `.stderr` diff. **Splitting now costs
two modules. After M2 it is a breaking change to whatever `__private` exposed.**

#### Enforce it mechanically (the rule was a convention, which is why it did not hold)

Every seal impl in `typelevel.rs` carries **`SEAL-EXACT`** or **`SEAL-DIFF`**
(with a justification and a `fixture:` name) in the comment directly above it.
`every_seal_impl_should_declare_whether_it_mirrors_its_trait` in `sealed.rs`
scans every seal impl and fails on a missing annotation or a nonexistent fixture
(broken deliberately in both directions to confirm).

This does not prove the annotations are **correct** — no cheap test can. What it
closes is the failure that actually happened: **borrowing another trait's safety
argument to drop a bound, with nobody noticing the claim was made at all.**

**This guard itself had a hole** (found in review). Its first version read only
`include_str!("typelevel.rs")` — **reproducing exactly the instance-shaped
thinking it was meant to replace, by looking only where the last bug was.**
Adding an unannotated, loose seal impl to `domain.rs` left the test green, and
that impl did in fact permit a downstream `Includes` forgery (measured). It now
scans all of `src/`, joins multi-line impl headers folded by rustfmt, and catches
forms where a `use` has dropped the `private::` prefix. **All three routes were
broken deliberately to confirm.**

**A remaining limit, stated plainly**: a false `SEAL-EXACT` passes — the bounds
are not actually compared. What guarantees a difference is harmless is **the
fixture pair**; the annotation only forces the claim to be written down.

#### Coherence's precise role (corrected twice in #9's review)

The first version said "coherence half-protects `Append`", and the second said
"E0119 only fires within the crate". **Both are wrong.** Measured downstream, in
a separate crate, against the real verum:

| Form | What rejected it |
|---|---|
| `Lookup`, correct key, well-formed map, forged `Out` | **E0119** |
| The same, at depth | **E0119** |
| `Has`, true membership, well-formed set | **E0119** |
| `Has`, true membership, set malformed **at depth 2** | **E0119** |
| `Has`, true membership, set malformed **at depth 1** | Compiles — the seal decides |

**E0119 does fire across crates, and in the region where the seal necessarily
holds it is the entire defence** (verum's own impl applies, so the seal is
satisfied). "The difference set is the attack surface" is only a *complete*
statement because coherence and the orphan rule close everything outside the
difference.

But **it cannot be used as a design basis.** The last two rows have the same
trait and the same seal, differ only in the **depth** at which the `ConsList`
obligation breaks, and are rejected by opposite mechanisms. A guarantee cannot
rest on something unpredictable — which is the real reason to keep seals exact,
and a stronger reason than the one first written here.

> The route to the second error is worth recording. **"E0119 only fires within
> the crate" was a review agent's claim adopted without verification**, and it was
> written *inside a sentence correcting a different error*. **A correction is a
> new claim and needs as much verification as the thing it replaces.**

#### Every new sealed trait gets a deepest-position forgery fixture

This is the only mechanical way to prevent the gap that occurred.
`has_cannot_be_forged.rs` covered `Here` (the **shallowest** impl position) and a
non-tuple `Self` — both routes the seal's head impl already closed — so the open
route passed straight through.

| Sealed trait | Shallowest-position fixture | **Deepest-position fixture** |
|---|---|---|
| `Has<T, Idx>` | `has_cannot_be_forged.rs` | `has_cannot_be_forged_at_depth.rs` |
| `Append<B>` | **`append_cannot_be_forged_at_base.rs`** | `append_cannot_be_forged_at_depth.rs` |
| `Lookup<K, Idx>` | **`lookup_cannot_be_forged_at_head.rs`** | `lookup_cannot_be_forged_at_depth.rs` |

| Any future trait with a recursive impl | **Required** | **Required** |

> **The dashes in this table were the audit record of #9's hole.** Two cells in
> the shallowest column were blank under a heading that said "required", and what
> was actually open was `Append`'s base (`for ()`) and `Lookup`'s head. **Allow no
> blank cell** — if one cannot be filled, write down why that position is closed.

#### A trait with a `type Out` is a step more dangerous — the forgery chooses the result

`Append` and `Lookup` have associated types. A forged impl does not merely assert
"this is declared"; it **names the composed capability set itself.** Forging
`Has` makes one predicate false; forging `Lookup` returns any scope you like for
a given condition.

```rust,compile_fail
// this compiled downstream while the seal's recursion was dropped (measured in T-M0-09).
// The seal now matches, so it is rejected with E0277 — this check confirms that on every run.
impl verum::Lookup<IsPaid, verum::There<verum::Here>> for ((X, X), ()) { type Out = ForgedScope; }
```

**So for a sealed trait with an associated type, the deepest-position fixture is
required, not optional.**

**Do not count on coherence — `Append` is especially misleading.** Because
`Append` has a base impl `for ()`, the obligation at the intersection becomes
satisfiable and forgery onto a well-formed list is rejected with E0119 — **half
protected.** When the tail is not a cons list the obligation is unsatisfiable,
coherence stands aside, and only the seal is left. Do not conclude "I tried it
and got E0119, so it is safe."

> **Why this was promoted from a knowledge-bank entry to a rule**: "an AI responds
> to a trait-bound error by writing the missing impl" recurred **twice** as a
> Critical, in #6 and #8. Both times the cause was the same — the seal did not
> mirror the sealed trait's structure — and a passive entry in a knowledge bank
> did not stop it. The motivation (the AI behaviour, demonstrated experimentally)
> is still recorded there; **this section is the canon for the remedy.**

### One seal per sealed trait (measured)

The original design used a single `Sealed<Args>` distinguished by its arguments.
**It was discarded after measuring.**

**rustc's sealed-trait help lists every impl of the seal, regardless of `Args`.**
So an error about one sealed trait lists the implementing types of *other* sealed
traits.

```text
# the misdirection produced for `impl Includes<Order> for User`
= help: the following types implement the trait:
          ()            ← from ConsList
          (H, T)        ← from ConsList
          verum::Here   ← from Index
          verum::There<I>
```

Cons-list and index types are offered as candidate fixes. **The list grows with
every sealed trait added** (seven by M3). T-M0-06's error had inflated from 14
lines to 19.

Splitting the seals makes each error show only its own implementing types (back
to 14 lines, measured).

### Seals are declared through the `seal!` macro

The cost of splitting is that `on_unimplemented` must be written per seal — and
can be forgotten. **A seal without it leaks a raw trait-bound error**, which is
what having a single "floor" in T-M0-06 was for.

So `macro_rules! seal!` inside `private` generates the trait **always with
`on_unimplemented` attached.** The `$attr` slot accepts doc comments only, so a
caller cannot override the mandated message either.

> **A macro makes it automatic, not mandatory (the first version claimed
> otherwise and was wrong).** Adding a hand-written `pub trait SealedX {}` inside
> `private` **passes cleanly, lint table included** (measured).
>
> What makes it mandatory is the unit test
> `seals_should_only_be_declared_through_the_macro` in `sealed.rs`, which **reads
> its own file and rejects any `pub trait` not produced by the macro template.**
> Confirmed by injecting a hand-written seal and watching it fail.

```rust,ignore   // fragment, not a complete item
seal! {
    /// Seals [`crate::Includes`].
    SealedIncludes<D>
}
```

Trait-specific guidance is attached separately to the sealed trait — the two
coexist because different bounds fire.

### `on_unimplemented` goes on both the seal and the trait

```rust,ignore   // fragment, not a complete item
pub(crate) mod private {
    #[diagnostic::on_unimplemented(
        message = "`{Self}` cannot implement a sealed Verum trait",
        label = "this type was not produced by a Verum derive",
        note = "..."
    )]
    pub trait Sealed {}
}
```

**The seal's annotation is the floor.** It applies automatically to every trait
sealed from now on, removing the route where one is forgotten and a raw error
leaks.

**But the floor alone is not enough.** It fires only when `Sealed` itself is
unsatisfied — that is, **only when a user wrote a hand-written impl.** Most real
errors are unsatisfied bounds at a **use site** (the `ctx.orders()` shape), where
`Sealed` never appears.

```text
# what happens with no annotation on Includes (measured)
error[E0277]: the trait bound `MyEndpoint: Includes<Order>` is not satisfied
   |    ^^^^^^^^^^ the trait `Includes<Order>` is not implemented for `MyEndpoint`
```

Neither Verum's message nor rustc's sealed explanation appears. **The requirement
"never show a raw bound failure" breaks on this route.**
[type-level.md](./type-level.md) §5's "every trait with type parameters carries
one" points at the same thing.

So **annotate both.** Different errors fire, so they do not collide, and adding
one leaves the other's `.stderr` unchanged (confirmed).

```rust,ignore   // needs a crate or a verum-private module this harness does not carry
#[diagnostic::on_unimplemented(
    message = "`{Self}` does not declare the domain `{D}`",
    label = "reaching `{D}` requires declaring it",
    note = "either add `{D}` ... or use a domain it already declares — do not implement `Includes` by hand, it is sealed"
)]
pub trait Includes<D>: derive_facing::SealedIncludes<D> {}
```

The note pointing **both ways** — widen the contract, or use something already
declared — is required by [diagnostics.md](../specs/diagnostics.md), and
corresponds to "when you block a path, offer a checked alternative".

> **rustc recognises a sealed trait and emits its own note** (measured).
> ```text
> = note: `Includes` is a "sealed trait", because to implement it you also need to
>   implement `verum::sealed::private::Sealed`, which is not accessible; ...
> ```
> It **coexists** with `on_unimplemented` — message, label and note are replaced
> and the built-in note remains. So the "raw bound error" state never arises.

**Going forward**: trait-specific guidance ("use `#[derive(Endpoint)]`" and
similar) is added **individually** once there are more traits. It coexists with
the seal's generic text because different errors fire. Keep the generic text as
the floor and specialise only where needed.

Note that the path appearing in real errors is
`verum::sealed::private::Sealed` (this document's `private::Sealed` is the
in-crate form). Both "sealed" and "private" reach the reader, so it is left as
it is.

---

## 3. The prelude

Users have to `use` the extension traits — `CtxUsers`, `UserRepo` and so on, per
domain. An inherent impl cannot be written (E0116 —
[`type-level.md`](./type-level.md)).

Forgetting the `use` produces the **unrelated** error "no method named `users`".

Both mitigations are implemented.

```rust,ignore   // needs a macro that arrives in M2
// 1. provide verum::prelude
pub mod prelude {
    pub use crate::{Ctx, Endpoint, Handler, Result};
    pub use verum_macros::{contract, endpoint, Domain, Event, Request, View};
    pub use http::{Method, StatusCode};
}

// 2. the derive emits a `pub use` for the extension traits
// #[derive(Domain)] on User generates:
//   pub use self::__verum_user_ext::{CtxUsers, UserRepo};
```

---

## 4. Encapsulation and API stability

### A public type does not expose its internals

```rust,compile_fail
// ❌ making an internal field pub
pub struct Ctx<'req, E> { pub repos: RepoRegistry, ... }
```

```rust,ignore   // verum-internal: legal only inside the crate that owns the trait or type
// ✅ through accessors
impl<'req, E: Endpoint> Ctx<'req, E> { /* only via extension traits */ }
```

### Public enums and structs that will gain values are `#[non_exhaustive]`

```rust
#[non_exhaustive]
pub enum VerumError { /* ... */ }

#[non_exhaustive]
pub struct ServerOptions { /* ... */ }
```

Adding a variant or a field stays non-breaking. It also prevents exhaustive
matches and struct-literal construction, steering users to constructors and
builders.

### `Ctx`'s constructor demands a sealed token

```rust,ignore   // fragment, not a complete item
impl<'req, E: Endpoint> Ctx<'req, E> {
    pub(crate) fn new(rt: &'req Runtime<Sealed>, ...) -> Self;
}
```

A user cannot construct `Runtime<Sealed>`, so they cannot build a `Ctx` for an
endpoint type of their choosing. **Do not expose a god-mode constructor for
tests** ([`test.md`](./test.md)).

> What `Runtime` and `Sealed` actually are is undecided, and visibility alone was
> measured to leak — see [`../adr/0006-runtime-sealed-token.md`](../adr/0006-runtime-sealed-token.md).

### Capabilities are not exposed as values

A capability is expressed through `Ctx<'req, E>`'s type parameters and never
materialised as a value. Do not expose an API taking a `Cap<T>`
([`../specs/rust-type-model.md`](../specs/rust-type-model.md)).

---

## 5. `Ctx` is not `'static`

```rust,ignore   // declaration shown with its body elided
pub struct Ctx<'req, E> { /* ... */ }   // not 'static; Send is kept
```

- **Not `'static`** → it cannot be passed to `tokio::spawn`, closing the route by
  which a capability crosses the request boundary.
- **`Send` is kept** → the handler's future must be `Send` to load onto hyper's
  multi-thread runtime.

`ctx.spawn::<Job>(..)` is provided instead, requiring a `Spawn<Job>` effect to be
declared. **Block a path and provide a checked alternative in the same change**
([`../specs/unverified-boundaries.md`](../specs/unverified-boundaries.md)).

---

## 6. Escape hatches are recorded with a proof token

Low-level APIs are exposed ([`../concepts.md`](../concepts.md), principle 21),
but the possibility of forgetting to record one is removed structurally.

```rust,compile_fail
// ❌ forget the attribute and nothing is recorded
pub fn raw_connection(&self) -> &hyper::upgrade::Upgraded;
```

```rust,ignore   // fragment, not a complete item
// ✅ demand a ZST proof that only the attribute macro can produce
pub fn raw_connection(&self, proof: EscapeHatchProof) -> &hyper::upgrade::Upgraded;
```

Only `#[escape_hatch(reason = "...")]` can produce an `EscapeHatchProof`. Without
the attribute the function cannot be called, so it necessarily appears in the AI
Context's `escape_hatches`.

**Until this is implemented, the AI Context emits `"unknown"` for
`escape_hatches`, not `[]`.**

---

## 7. MSRV and edition

| Item | Value |
|---|---|
| edition | **2024** |
| MSRV | **1.85+** |

Required for `#[diagnostic::do_not_recommend]`, stabilised in 1.85, and for
edition 2024. Async closures (`AsyncFnOnce`), used by the `when` scope, arrived
in the same release.

In a workspace these go in `[workspace.package]`, and **each crate inherits them
explicitly.**

```toml
# root Cargo.toml
[workspace.package]
edition = "2024"
rust-version = "1.85"
```

```toml
# crates/*/Cargo.toml — the inheritance must be declared
[package]
edition.workspace = true
rust-version.workspace = true
```

> **Forgetting `rust-version.workspace = true` is a silent no-op.**
> `[workspace.package]` is only a template and never reaches a package unless the
> member opts in. The manifest looks right, the build succeeds, and cargo enforces
> no MSRV at all. Verify that `rust_version` in `cargo metadata` is not `null`.

CI verifies the MSRV build. Raising it is a deliberate decision, and this
document is updated in the same change.

---

## Never do this

- ❌ Expose an `axum::` / `tower::` / `hyper_util::` / `matchit::` type in the
  public API.
- ❌ Expose a `State`-equivalent entry point that can produce anything.
- ❌ Publish `Endpoint` / `Has` / `Includes` / `Field` / `Condition` unsealed.
- ❌ Make `Ctx` `'static`, or give it a public constructor.
- ❌ Create a backend trait with a single implementation.
- ❌ Expose an API taking a `Cap<T>`.
- ❌ Expose an escape hatch without a proof token — recording it would be
  forgotten.
