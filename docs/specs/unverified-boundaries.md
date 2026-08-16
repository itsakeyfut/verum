# Unverified boundaries

The ledger of every route the type check does not reach. **The file that exists
so nothing is left unrecorded.**

Related: [`capability-system.md`](./capability-system.md),
[`ai-context.md`](./ai-context.md),
[`persistence.md`](./persistence.md).

---

## Why this file exists

Verum's central risk is not that the types are weak.

> **It is that next to a route closed by types sits an easier, unchecked one.**

The higher the type wall, the more an AI **walks around** it rather than over it.
An AI stuck on a compile error always has a third option: relax the contract, do
it in the service layer, throw it somewhere else through an event, write raw SQL.

So the goal is not "close everything in types". It is **to decide, for every
route, whether it is closed or stated.**

```text
an unknown gap    → dangerous. Both AI and human believe it is guaranteed
a stated boundary → manageable. It can be identified as a review subject
```

This file enumerates every route and tracks its state.

---

## The three structural causes

Closing routes individually is whack-a-mole. The causes reduce to three.

| Cause | Routes | The structural response |
|---|---|---|
| **1. The domain model is exposed as an ordinary Rust struct** | Direct assignment / `into_owned` / a `Debug` leak / interior mutability / **construction and reading through `Repr` (path 21)** | Make the domain opaque, reachable only through capability-checked accessors. **But opacity alone is not enough** — the `Repr` generated for persistence opens a route alongside it (path 21). How it is closed is #18 |
| **2. Nothing constrains the lifetime or route of a type that can carry a capability** | spawn / the test god-mode / a `when` leak / `dyn Repository` / a `PgPool` on the endpoint | Bind it to the request lifetime with `Ctx<'req, E>` and seal the construction route |
| **3. Effects happen where no contract is required** | The far side of `emits` / middleware / a repository implementation / free-function constructors / `Condition::holds` | Increase the places that require a contract (in stages) |

---

## The full ledger

### Cause 1: how the domain model is exposed

| # | Route | Response | State |
|---|---|---|---|
| 1 | `user.email = v`, direct assignment | Domain opacity (private fields) | **Closed in the First PoC** |
| 2 | `*user = other_user` (fetch two with `find` and swap) | Restricting the construction route does not close it | **Stated** (below) |
| 3 | Escaping a projection with `into_owned()` | **Not provided** | **Closed in the First PoC.** ⚠️ But deriving `Clone` on `Repr` brings it back (see path 21) |
| 4 | A data leak through `Debug` / `Serialize` | A custom implementation emitting declared fields only, derive-generated | **Closed in the First PoC.** ⚠️ The response is **imposed only on the domain side** — deriving `Debug` on `Repr` leaks from within the same crate (see path 21) |
| 5 | Mutation through interior mutability (`RefCell` / `Mutex` / `Cell`) | Restrict domain field types (a whitelist until `Freeze` stabilises) | **Closed in the First PoC** |
| 21 | **`User::from_repr(UserRepr { .. })` / `as_repr()` reachable from anywhere in the domain's crate** | Undecided (settled in #17 / #18) | ⚠️ **Open** (measured in T-M1-01 / #13; below) |

> **Numbers are only ever appended, never renumbered.** Paths 12 / 13 / 14 are
> referenced from [`../rules/api-surface.md`](../rules/api-surface.md),
> [`../rules/proc-macro.md`](../rules/proc-macro.md) and the knowledge banks;
> renumbering would silently break every cross-reference. Stability of the numbers
> takes priority over grouping by cause.

#### path 21 — `Repr` cannot be confined to "the repository implementation only" (compile-verified)

`#[derive(Domain)]` **expands in the user's crate**, so the visibility of the
generated `pub(crate) struct UserRepr` and `pub(crate) fn from_repr` / `as_repr`
is **the whole application crate.** The derive cannot emit `pub(in ...)` because it
does not know which module the repository is written in. That leaves two readings,
and **neither works.**

| Where the repository lives | Measured |
|---|---|
| The same crate as the domain (a single-crate application) | Any handler in the crate can write `User::from_repr(UserRepr { email: anything, .. })`. No capability, no repository, no SQL, no `unsafe` |
| A separate crate | `Repr` is entirely invisible (`E0603`). The design does not function |

**The comparison with path 2 splits by axis. It is not strictly worse.**

| Axis | Which is worse |
|---|---|
| Freedom of values | **21.** Path 2 (`*user = other_user`) can only insert values `find` actually returned, whereas 21 can **invent** them. Its preconditions are lighter too (no capability, no `find` result) |
| Reach and permanence | **2.** Path 2 holds across a crate boundary given a `&mut D`, and **survives closing 21** (it is classified below under what cannot be closed in principle). 21 is crate-local and disappears once closed |

A forged `User`'s getters were confirmed to work at run time (probe P8 of the
spike). But **it was not compared against a loaded `User`**, so "indistinguishable"
is an observation about getter behaviour.

**Making the fields private does work, but the guarantee is "from outside the
defining module", not a type boundary** (measured). From the defining module and
its children `u.0.email = v` compiles, and **the macro expands in the same module
as the user's `struct User`**, so an `impl` or helper the user writes next to it
stands on the permissive side. The shortest workaround for an AI stuck on `E0616`
is "move that code into the domain definition file" — the textbook case for
ARK-002.

The error code varies with the shape (measured; this line originally gave `E0616`
for both, because no probe existed for `u.email = v`).

| Shape | The code actually emitted |
|---|---|
| Newtype with an `email()` getter (**the shape of the real design**) | **`E0615`** (attempted to take the value of a method) |
| Newtype without the getter | `E0609` (no such field) |
| A flat private named field, from **outside the module** | `E0616` (field is private) |
| A newtype's `u.0.email`, from outside the module | `E0616` |
| The inner field made `pub(crate)` | **It compiles** — which is why the derive must emit private |

**`E0615` and `E0609` cannot have their wording replaced with `#[diagnostic::…]`**
(they sit outside the three defence layers). So however path 21 is closed, no
guidance toward the contract can be emitted for this class of mistake. How it is
closed has to be decided together with how it is diagnosed.

**`Repr` opens paths 3 and 4 as well as 21.** Deriving `Debug` on `Repr` prints
every field, including undeclared ones, via `format!("{:?}")` (path 4's response
is imposed only on the domain side), and deriving `Clone` yields a fully owned
copy — the equivalent of path 3's `into_owned`. **In the specified shape this is
confined to the same crate** — an external crate cannot reach `as_repr` and gets
`E0624` (measured; this originally said "leaks from another crate too", which was
wrong).

The constraints on generated code gain "do not derive `Debug` / `Clone` /
`Serialize` / **`Deserialize`** on `Repr`". **The general form to remember: any
derive-produced constructor that assembles the struct inside the defining module
is a forging route.** `FromRow` and `Deserialize` are the same mechanism, and an
enumerated ban list opens a hole the moment one more derive is added.

Reproduction and the probe table: `spikes/domain-opacity-sqlx/`
(`bash run.sh`). The specification-side account is in
[`persistence.md`](./persistence.md) §Verdict. **Not deciding how to close it in
the spike was deliberate** — this route is the domain's exposure form itself, so
the choice determines the shape of M2's derive tasks (ARK-002: blocking without an
alternative pushes people onto unchecked routes).

### Cause 2: the lifetime and route of a capability

| # | Route | Response | State |
|---|---|---|---|
| 6 | Carrying `Ctx` out with `tokio::spawn` | `Ctx<'req, E>` (not `'static`; `Send` preserved). ⚠️ The promised alternative `ctx.spawn::<Job>` **does not compile** (F1) | **Closed in the First PoC** — measured, T-M1-02 probe C1 (`E0521`) |
| 7 | Handing it to a `static Sender<Ctx<E>>` | Same, and the same missing alternative | **Closed in the First PoC** — measured, probe C3 (`E0521`) |
| 8 | Leaking out of a `when` scope with `Ok(ctx)` | **Not the return type** — the higher-ranked `Ctx` closes the specified signature; see below | **Closed for the specified signature. ⚠️ OPEN for a named `'req`** — reachable from an ordinary handler today (measured) |
| 9 | `Ctx::for_test()` as a god-mode constructor | Require a sealed `Runtime` token ([ADR-0006](../adr/0006-runtime-sealed-token.md), still `proposed`); testing goes through an API with a fixed endpoint type | ⚠️ **Stated, not closed.** What T-M1-02 measured is **visibility** — `Ctx::new` is `pub(crate)`, so `app` gets `E0624`. **No seal was measured**, and visibility alone was measured to leak (note below) |
| 10 | A `PgPool` on the endpoint struct | `#[endpoint]` rejects anything but a unit struct | **Closed in the First PoC** |
| 11 | Passing a `dyn Repository` to a service (the type parameters vanish) | Do not expose `dyn Repository`; parameterise the service by capabilities too | **Closed in the First PoC** |
| 12 | A hand-written `impl Endpoint` declaring arbitrary capabilities | Seal `Endpoint` | **Closed in the First PoC** |
| 13 | `impl Includes<Order> for User` (a local type, so it passes the orphan rule) | Seal `Includes` | ⚠️ **Provisionally closed** (T-M0-06 / #6; with the re-verification condition below) |
| 14 | Forging `impl Field<...>` (forging `Field::NAME` forges the column name in generated SQL) | Seal it | **Closed in the First PoC** (`Field` unimplemented) |
| 14a | `impl Has<Elem, Idx> for <set>` — forging membership itself. **The head position (`Here`) and non-head positions (`There<_>`) are separate routes** | Seal `Has`, **and make the seal's recursive impl conditional too** | ✅ **Closed** (T-M0-08 / #8; `has_cannot_be_forged.rs` + `has_cannot_be_forged_at_depth.rs`) — read the note below |
| 14b | `impl ConsList for MyType` — forging the shape proof, making a malformed set look well-formed | Seal `ConsList` | ✅ **Closed** (T-M0-07 / #7; `cons_list_cannot_be_forged.rs`). The tuple shape is also closed by the orphan rule (E0117) (re-checked in T-M0-08) |
| 14c | `impl Index for MyIdx` — forging the position of membership | Seal `Index` | ✅ **Closed** (T-M0-07 / #7; `index_cannot_be_forged.rs`). `There<MyIdx>` and `There<There<MyIdx>>` are closed by the orphan rule too (re-checked in T-M0-08) |
| 14d | `impl Append<B> for <set>` — forging the concatenation result. It has a `type Out`, so **the composed capability set itself can be named** | Make `Append`'s seal **match** the trait (including the base impl's `B: ConsList`) | ✅ **Closed** (T-M0-09 / #9; `append_cannot_be_forged_at_base.rs` + `_at_depth.rs`). **It had once been closed on a miss** — note below |
| 14e | `impl Lookup<K, Idx> for <map>` — forging "the entry for this key is this", swapping a conditional scope arbitrarily | Make `Lookup`'s seal **match** the trait (including the head impl's `T: ConsList`) | ✅ **Closed** (T-M0-09 / #9; `lookup_cannot_be_forged_at_head.rs` + `_at_depth.rs`). **It had once been closed on a miss** — note below |
| 14f | `impl Has<H, Idx> for (H, <non-cons-list>)` — **passing a malformed set through the capability check.** **The head and deep positions alike** (`impl Has<Other, There<There<Here>>> for (Decl, (Elem, (Other, Junk)))` compiles). Membership itself is true, so no capability is gained | None (`Has`'s seal deliberately drops `ConsList` for diagnostics — `SEAL-DIFF`) | ⚠️ **Stated** (until T-M2-09). `ConsList`'s "a malformed set fails closed" can be defeated downstream. But **only when the element is a bare local type** — a real effect type `Mutate<User, Email>` wraps the local type in verum's generics, so the orphan rule (E0117) closes it (measured). So it is **unreachable for effect sets and limited to domain-shaped elements.** `has_forged_membership_on_malformed_set.rs` (head) and `has_forged_membership_at_depth_on_malformed_set.rs` (depth) pin the side where *false* membership is rejected, at both positions. **It is not "permanent"** — once T-M2-09 asserts the shape at the declaration site, or conditional `on_unimplemented` stabilises, the `SEAL-DIFF` justification lapses and the bound can be restored |

> ### ⚠️ 14a–14e **could reopen at M2 — but will not, because the seal was split** (found in #9's review)
>
> The path-13 note below says "M2 will be forced to introduce
> `#[doc(hidden)] pub mod __private`" and "that is the moment the seal's strength
> drops". **That warning was attached only to 13.** With every seal in one module,
> that single change makes **every seal nameable**, reopening 14a–14e at once — and
> making that change did indeed let a forged membership compile downstream.
>
> In response the seal was **split into two modules** (#9). `private` holds the
> **structural** seals (`SealedConsList` / `SealedIndex` / `SealedHas` /
> `SealedAppend` / `SealedLookup`); verum implements them on tuples itself with no
> derive involvement, so it stays **`pub(crate)` permanently.** `derive_facing`
> holds only the seals a derive must satisfy (today, `SealedIncludes`), and only
> that one is exposed at M2.
>
> So **14a–14e's ✅ survives M2**, and 13's ⚠️ provisional closure remains a
> `derive_facing` matter.
> `compile_fail/sealed_derive_facing_module_is_private.rs` pins the current state,
> and the moment M2 opens it, it surfaces as a `.stderr` diff.

> ### ⚠️ 14d and 14e had also been closed on a miss (found in #9's review, the second time after 14a)
>
> Following 14a's lesson, #9 added a **deepest-position** fixture to both. What was
> open was the **shallowest** — `Append`'s `for ()` (the base) and `Lookup`'s head.
> `Append`'s base is **the floor every concatenation bottoms out on**, so one line
> of `impl Append<Local> for ()` **rewrote every concatenation result in the
> program.**
>
> The cause was the seal dropping the shape bound. Reading it as "verum's impl has
> `B: ConsList`, so it is protected" was the mistake: **a bound on verum's impl is
> not imposed on a foreign impl.**
>
> As a ledger practice: **the basis for closure is "every impl position is
> covered", and neither the deepest nor the shallowest alone is enough.** Allow no
> blank (`—`) in [api-surface.md](../rules/api-surface.md) §2's table — in #9 that
> blank pointed straight at where the hole was.

> ### ⚠️ 14a had once been **closed on a miss** (found in T-M0-08's review)
>
> #8 originally recorded 14a as closed on the strength of
> `has_cannot_be_forged.rs` alone (`Here` plus a non-tuple `Self`). **Both were
> routes the seal's head impl already closed**, and the genuinely open `There<_>`
> route was not covered — the ledger row itself wrote only the shallow side,
> `impl Has<Elem, Here> for MyList`, so the fixture matched the row and the row
> matched the fixture while both missed the real hole.
>
> The lesson concerns the ledger's practice: **the basis for closure is not "there
> is a fixture" but "every impl position of that trait is covered".** A trait with
> a recursive impl pins both the shallowest and the deepest (made a rule in
> [api-surface.md](../rules/api-surface.md) §2).
>
> **The more type arguments a sealed trait has, the larger its exposure.** 14b and
> 14c turned out to be intact because `ConsList` and `Index` have no type
> arguments, leaving `Self` as the only position a local type can occupy (a tuple
> or `There<_>` is not a local type in `Self`, so the orphan rule rejects it
> first). `Has<T, Idx>` allows a local element type at `T`, so it passes the orphan
> rule and the seal is the only defence. **When reviewing a new sealed trait, look
> at its type-argument count first.**

> **Path 14 was split into three.** `Has` and `Field` were originally one row, but
> the basis for closure is **that the trait carries the seal as a supertrait**, and
> that lands at different times per trait. Left combined, closing `Has` would read
> as closing `Field` too. The same reason `ConsList` / `Index` were split out in #7.

> Note on #14: for `impl Has<Mutate<User, Password>> for ()`, `Has` and `()` are
> both foreign, and `Mutate<User, ..>` merely contains a local type as a type
> argument rather than being local, so **the orphan rule most likely prevents it**
> (originally unverified). #13, by contrast, definitely passes because `User` is
> local. Sealing helps in both cases, so it is applied without distinction.
>
> **Measured in T-M0-06** (the guess above is right *for this shape* and wrong as a
> general rule):
> ```text
> impl verum::Includes<Order>      for ()  ->  E0277 (the orphan rule passes; only the seal stops it)
> impl verum::Includes<Vec<Order>> for ()  ->  E0117 (wrapped in foreign generics, it is not treated as local)
> ```
> **If a local type appears directly as a trait's type argument, the orphan rule
> passes.** Inside foreign generics, as with `Mutate<User, ..>`, it does not. So
> #14's guess is correct for the `Has<Mutate<..>>` shape specifically, but
> generalising it to "merely containing a local type as a type argument is
> prevented" is wrong. **Either way the orphan rule must not be relied on, and the
> conclusion that the seal is the only defence is unchanged.**

> **On #13's closure (T-M0-06 / #6)**: the seal foundation plus sealing
> `Includes<D>: SealedIncludes<D>` landed, and a UI test pinned the failure of
> `impl verum::Includes<Order> for User {}` down to its `.stderr`.
>
> **The basis for closure is not "a `Sealed` exists" but "that trait carries
> `Sealed` as a supertrait"**, so #12 (`Endpoint`) and #14 (`Field`) stay open
> until the trait in question is implemented at M2 (`Has` was split out as 14a and
> closed in T-M0-08). Do not treat the foundation landing as closing them all.
>
> **⚠️ Why it is provisional (discovered in T-M0-07's review)**: path 13 is closed
> today because **nobody can satisfy `SealedIncludes<D>`** — `verum-macros` emits no
> macros at all. And as [`../rules/api-surface.md`](../rules/api-surface.md) §2
> records, **a proc macro's output resolves in the calling crate and so cannot reach
> `pub(crate) mod private`** (E0603, measured). M2 will be forced to introduce
> `#[doc(hidden)] pub mod __private`, and §2 itself says that is the moment the
> seal's strength drops.
>
> **So today's green is not evidence of M2's green.** The re-verification
> condition: after `__private` is introduced, with the derive emitting one domain's
> seal, confirm in both directions that `impl Includes<undeclared>` **is E0277** and
> that a declared one **compiles** (the same procedure used in T-M0-06).
>
> **That it stays closed once the derive lands** is the point of this closure. The
> seal was originally written as `Sealed` (over `Self` only), and a Tier-2 review
> demonstrated by measurement that the moment a derive generates one `Sealed`,
> `impl Includes<undeclared>` compiles. Changing the seal to `SealedIncludes<D>`
> **seals the relation itself**, and both directions were confirmed: forgery is
> E0277 and a declared one compiles. Detail in
> [`../rules/api-surface.md`](../rules/api-surface.md) §2, "a seal must carry the
> target trait's type arguments".

#### path 8 — the recorded remedy is not the mechanism, and a named `'req` is open (compile-verified)

**Four** documents recorded the remedy as "the closure's return type is fixed to
`Result<()>`" — `conditional-effects.md`, `capability-system.md`,
`handler-rules.md`, and this row itself. T-M1-02 measured it:

| Probe | Return type | `Ctx` lifetime | Result |
|---|---|---|---|
| D3 | `Result<()>` | higher-ranked | `E0308` |
| D4 | free | higher-ranked | rejected anyway |
| D5a | `Result<()>` | **named** — no leak attempted | `not general enough` under `+ Send` |
| D5c | `Result<()>` | **named** — leaking | **compiles** |

**What closes the specified signature is the higher-ranked `Ctx`.** D3 and D4
each reject `Ok(ctx)` on their own, so the return type is redundant — real, but
not the mechanism.

**Nothing closes a named `'req`.** D5c is that form and it type-checks, and it is
**reachable from an ordinary handler today**: `Handler::handle` is
`fn .. -> impl Future + Send`, not `async fn`, so the bound constrains only what
the *returned future* holds across awaits. A handler body is synchronous and
already holds `Ctx<'req, Self>` with `'req` named; it can drive the leaking
future to completion before it ever constructs the future it returns.

**Probe D5e** (`spikes/ctx-lifetime-rpitit/`, `bash run.sh`): compiles, runs
against the real multi-thread hyper server, and **mutates the store through a
`Ctx` that outlived its `when` scope** — safe Rust, no added dependency, no
god-mode constructor, no relaxed `Send`. Found in Tier-2 review by two
independent agents; made a standing probe in #48, so it re-runs rather than
resting on a review that has ended.

> **`+ Send` is not a containment bound.** An earlier version of this entry said
> it was what closed this path. That was wrong and is withdrawn. `+ Send`
> constrains values held across awaits; `.await` is the only thing that propagates
> the obligation, and a synchronous body can construct and consume anything on
> either side of one. Recorded as RK-017 so it is not re-derived.

**The remedy is a constraint on the signature, not a bound.** `when` must be
generated with the elided (higher-ranked) form and never with a named `'req`.
Because `when` is macro-generated, that is enforceable at **defence layer 1**
([`diagnostics.md`](./diagnostics.md)) — the macro emits the signature, so the
macro can refuse to emit the broken one. Nothing enforces it today.

Two consequences the taxonomy does not express:

* **The status word is wrong for the named variant.** The row says
  "Closed in the First PoC"; that is true of the specified signature and false of
  the variant an implementer reaches for. **#44 owns the status taxonomy** and this
  entry supplies only the measurement.
* **#44 was right without qualification.** It recorded this path as leaking,
  "compiled and run". An earlier version of this entry reconciled that away as
  "#44 measured the construction, the spike measured reachability". The leak is
  reachable; there was nothing to reconcile.

#### path 9 — A0 measures less than the status claims

A0 confirms `app` cannot *construct* a `Ctx` (`Ctx::new` is `pub(crate)`;
`E0624`). It does **not** confirm the path is closed: in the spike
`ErasedHandler::call` is a public trait method, so a `Runtime` can be `Box::leak`ed
and a handler driven with `'req = 'static` — no `Ctx` construction required. The
sealed-token design is [ADR-0006](../adr/0006-runtime-sealed-token.md), still
`proposed`, and visibility alone was measured to leak.

#### path 23 — `reads` is enforced with `mutates`' scope, not more (compile-verified)

#15 measured that a capability-checked getter rejects an undeclared read
(`E0277`). It also measured what never goes through a getter:

| Probe | Route | Result |
|---|---|---|
| P1 | `Debug` on the domain | prints every field, no capability |
| P2 | a free function taking `&Domain` | reads whatever it likes |
| **P4** | a `Projection<D, F>`'s `Debug` | **narrows to the declared set** — `Projection { email: "e@x" }`, `secret` never printed |

> **P4 replaces an earlier P3 row that said the opposite.** The first version of
> #15 recorded "the same — `F` is a type parameter, so no derive can enumerate
> it", and that was withdrawn under review: the derive emits one impl **per field
> of the Domain**, which it can see, and a fixed recursive walk resolves `F` at
> monomorphisation. So a projection **does** narrow its own `Debug`; what it
> cannot do is stop the `Domain` value's, which is P1 and is what keeps this path
> open.

**This is not an argument against the getters.** It is the boundary of what
"enforced" means for `reads`, and it is the boundary `mutates` already has:
`handle_via_ctx`. Recorded so `reads` is not read as narrower or broader than it
is once its level promotes.

**Path 4 does not cover this.** Path 4 is a `Debug` leaking fields the *Domain*
does not declare, and its remedy is a derive emitting the Domain's declared
fields. Every one of those is still outside the *endpoint's* `reads`. The two
paths share a mechanism and differ in which declaration they are measured
against.

Reproduce: `spikes/reads-getter-enforcement/` (`bash run.sh`). Decision in
[ADR-0004](../adr/0004-reads-enforcement-level.md).

### Cause 3: effects that happen outside the contract

| # | Route | Response | State |
|---|---|---|---|
| 15 | A subscriber to `emits` causes arbitrary effects | Require a contract on the subscriber + emit the transitive closure in the AI Context | **Deferred (stated)** |
| 16 | Middleware effects do not appear in the contract | Require a contract on middleware + have the router compose them | **Deferred (stated)** |
| 17 | Raw SQL inside a repository implementation | Move the boundary by generating the implementation / an SQL lint | **Deferred (stated)** |
| 18 | Side effects inside a free-function constructor (`AuditLog::user_updated()` and the like) — `kind: constructor_body` | Generate the constructors and remove the room for hand-writing | **Deferred (stated)** |
| 19 | Bypassing field granularity with `creates` + `deletes` (an upsert) — `kind: upsert_granularity` | The derive rejects declaring both for one domain / `create` takes new IDs only | **Deferred (stated)** |
| 20 | `Condition::holds` unlocks everything by returning `true` | **Impossible in principle** | **Permanently stated** |
| 23 | **`Debug` / `Serialize` / a free function reads a field the endpoint did not declare in `reads`** — `kind: uncapped_read` | Capability-check the getters (measured to work) and accept that these routes are outside them. A `Projection`'s **own** derived `Debug` does narrow to the declared set (#15, P4), but the `Domain` value still exists and its `Debug` and any free function taking `&Domain` reach every field | **Stated** (measured, #15) |
| 22 | **The `observed_effects` scan does not reach a service body** (a consequence of the Q-A decision to scan `handle` only) | Annotate every effect-carrying item and take the transitive closure at build time (a future form). For now, state it with `scope: "handle_only"` and `deferred` | **Stated** (Q-A / 2026-08-15) |

---

## What cannot be closed in principle

### The body of `Condition::holds`

```rust
impl Condition<User, UpdateUserRequest> for EmailChanged {
    const NAME: &'static str = "EmailChanged";
    fn holds(user: &User, req: &UpdateUserRequest) -> bool {
        true    // ← this makes every conditional effect unconditional
    }
}
```

A boolean a user wrote cannot be verified in types. And because **the AI Context
still emits `"conditional": [...]`, the metadata actively lies.**

The response:

- Always emit `condition_verified: false` in the AI Context
- Make it a convention that a `Condition` implementation is a pure function (no
  external I/O, clock or randomness)
- Require a condition to be defined once as a named type, so it can be identified
  as a subject for review and testing

### Row-level permissions (IDOR)

`Mutate<User, user::Email>` means "the email column of the User type may be
written", not "**this** user may be written".

```rust,ignore   // fragment, not a complete item
let victim = ctx.users().find(attacker_supplied_id).await?;
ctx.users().set_email(&mut victim, attacker_email)?;   // the capability is satisfied
```

Updating one row and updating every row look identical in the contract.
**Authorisation is always required separately**, and a capability is not a
substitute for it. See [`capability-system.md`](./capability-system.md), "a
capability is not authorisation".

### `*user = other_user`

Even with an opaque domain, holding a `&mut User` permits wholesale replacement.

```rust,ignore   // fragment, not a complete item
let mut a = ctx.users().find(id_a).await?;
let b = ctx.users().find(id_b).await?;
*a = b;    // it type-checks
```

Branding `find`'s return value by ID type would prevent it, but the ergonomic cost
is large. For now it is only stated.

---

## Contract-relaxation bias — a problem types do not solve

Faced with a compile error, an AI **widens the contract by one line rather than
fixing the implementation.** That is an economically rational choice, and types
cannot prevent it.

```text
error: undeclared mutation `User::status`
  help: add `User::status` to the contract, or remove this call
        ↑ the AI picks this one           ↑ often the correct one
```

[`diagnostics.md`](./diagnostics.md)'s "a help always shows both directions" is a
wording-level countermeasure and cannot constrain the choice itself.

The response, outside the types:

| Means | Contents |
|---|---|
| CI | Detect diffs that **widen** `mutates` / `reads` / `domains`, and require a separate label and extra review |
| Commit convention | A change that relaxes a contract states the reason in at least one line |
| Instructions for AI | State in the equivalent of a CLAUDE.md that relaxing a contract is a last resort |

Recognise that **this is an operational problem, not a type-system one**, and do
not try to solve it with types.

---

## Emitting it in the AI Context

An unchecked boundary is **always** emitted in the AI Context.

```json
{
  "endpoint": "UpdateUser",
  "unverified_boundaries": {
    "completeness": "best_effort",
    "entries": [
      {
        "kind": "condition_body",
        "detail": "EmailChanged::holds cannot be verified in types",
        "location": "src/conditions/user.rs:12",
        "permanent": true
      },
      {
        "kind": "middleware",
        "detail": "the effects of the applied middleware are undeclared",
        "permanent": false
      },
      {
        "kind": "event_subscriber",
        "detail": "effects on the subscriber side of UserUpdated are unchecked",
        "permanent": false
      },
      {
        "kind": "repository_impl",
        "detail": "SQL inside a repository implementation is unchecked",
        "location": "src/repositories/user.rs",
        "permanent": false
      },
      {
        "kind": "constructor_body",
        "detail": "a free-function constructor such as AuditLog::user_updated may cause effects; its purity is convention, not a check (path 18)",
        "permanent": false
      },
      {
        "kind": "upsert_granularity",
        "detail": "creates plus deletes on one domain changes field values with no Mutate capability (path 19)",
        "permanent": false
      },
      {
        "kind": "row_scope",
        "detail": "row-level permissions are outside the type check; authorisation is separate",
        "permanent": true
      },
      {
        "kind": "domain_swap",
        "detail": "*user = other_user holds given a &mut D and cannot be closed (path 2)",
        "permanent": true
      },
      {
        "kind": "domain_repr",
        "detail": "a domain's Repr is reachable from anywhere in the same crate; it can be constructed and every field read without a capability (path 21)",
        "location": "src/domain/user.rs",
        "permanent": false
      },
      {
        "kind": "malformed_set",
        "detail": "a malformed effect set can be passed through the capability check (path 14f), limited to bare local types as elements",
        "permanent": false
      },
      {
        "kind": "uncapped_read",
        "detail": "a Domain's Debug and free functions taking &Domain read fields the endpoint did not declare in reads; no getter shape reaches them, and a Projection narrows only its own Debug (path 23)",
        "permanent": false
      },
      {
        "kind": "service_body",
        "detail": "the observed_effects scan covers only the inside of handle; effects in a service body do not appear in the lower bound (path 22)",
        "permanent": false
      }
    ]
  }
}
```

`permanent: true` marks what cannot be closed in principle; `false` marks what
disappears once the contract is widened.

**`completeness` is `best_effort`, never `exhaustive`.** "We listed every path" is
not a checkable claim: a path was recorded as closed while it was open **three
times** (#6 / #8 / #9, all recorded in this file), and a single review added four
more. The same reasoning that makes `escape_hatches` emit `"unknown"` rather than
`[]` applies here — an unqualified list reads as a proof of exhaustiveness that
nobody can supply.

**Each `kind` here is what `enforcement.voided_by` names elsewhere in the same
output.** That join is the point: a key stating a guarantee points at the entries
that void it, so a reader who stops at `enforcement` cannot come away believing
the guarantee is unconditional ([`ai-context.md`](./ai-context.md) §1).

**This output mechanism is implemented from the First PoC.** Added later, it would
mean every AI Context up to that point had been lying.

---

## How progress is measured

Widening the contract reduces the entries in `unverified_boundaries`. That count
is the progress metric.

```text
First PoC:  3 permanent + 9 non-permanent
Full PoC:   3 permanent + 6 non-permanent (middleware and events handled)
Later:      3 permanent + 0 non-permanent
```

`permanent` never reaches zero. Not hiding that is this file's purpose. And the
count is a floor, not a total — `completeness: "best_effort"` says the list is
what has been found, so a *rising* count is a review working, not a regression.

> **The counting rule** (stated explicitly because a review noted "the number
> differs every time it is counted"): count **one-to-one with the entries emitted
> in the AI Context's `unverified_boundaries.entries`.** permanent 3 =
> `condition_body` (20) / `row_scope` (row-level permissions) / `domain_swap` (2).
> non-permanent 9 = `middleware` (16) / `event_subscriber` (15) /
> `repository_impl` (17) / `constructor_body` (18) / `upsert_granularity` (19) /
> `domain_repr` (21) / `malformed_set` (14f) / `service_body` (22) /
> `uncapped_read` (23).
>
> **Paths 18 and 19 were previously uncounted** because neither had a `kind` name
> decided, and this definition excluded 19 while counting 18 — so what the
> definition counted and what was emitted disagreed (#43 item 8). Both are now
> named and emitted, which #38 forced: `enforcement.voided_by` may only name a
> `kind` that exists, and both paths void `mutates`.
>
> These twelve entries must **agree as a set** with both this file's sample and
> [`ai-context.md`](./ai-context.md)'s. Three places holding different values is
> why this note was rewritten; `spikes/doc-code-blocks/check_json.py` now makes
> the agreement mechanical rather than a promise.

---

## The exact scope of "a GET is read-only"

Unless middleware carries a contract, this guarantee is **confined to the handler
scope.**

```rust,ignore   // fragment, not a complete item
// if auth middleware updates last_login_at
GET /users/{id}
  handler scope : Mutates = () → read-only (true)
  request scope : User.last_login_at is updated (false)
```

This is stated per key rather than once globally: `mutates`, `creates` and
`deletes` each carry `enforcement.scope: "handle_via_ctx"` and list `middleware`
under `voided_by`. When middleware contracts arrive, `middleware` leaves
`voided_by` and the scope widens.

> **There is no longer a `scope_of_readonly_guarantee` key.** It said exactly what
> those three keys now say — "all three empty, checked only inside the handler" —
> and a value derivable from its neighbours is a value that can disagree with them.
> It also overstated: a GET may still cause Logging, Metrics, Tracing, CacheRead
> and CacheWrite ([`effect-system.md`](./effect-system.md)), so nothing about it
> was read-*only*. See
> [ADR-0008](../adr/0008-guarantees-carry-scope-and-voiding-paths.md).

**Naming a guarantee's scope accurately matters as much as making the guarantee
stronger.**
