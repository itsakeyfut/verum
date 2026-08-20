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

| Cause | Routes | The structural response | What the remedy does not reach |
|---|---|---|---|
| **1. The domain model is exposed as an ordinary Rust struct** | Direct assignment / `into_owned` / a `Debug` leak / interior mutability / **construction and reading through `Repr` (path 21)** | Make the domain opaque, reachable only through capability-checked accessors. **But opacity alone is not enough** — the `Repr` generated for persistence opens a route alongside it (path 21). How it is narrowed is #33 / ADR-0010 | derives the **user** attaches, in a position the attribute cannot see (**26**); the insides of a field's **type** (**28**). Opacity is a property of the Domain, and neither route goes through it |
| **2. Nothing constrains the lifetime or route of a type that can carry a capability** | spawn / the test god-mode / a `when` leak / `dyn Repository` / a `PgPool` on the endpoint | Bind it to the request lifetime with `Ctx<'req, E>` and seal the construction route | the lifetime of what `Ctx` **hands out** (**24**) — containing the `Ctx` was measured not to contain its products. And a named `'req` in the `when` signature (**8**), which no bound rejects |
| **3. Effects happen where no contract is required** | The far side of `emits` / middleware / a repository implementation / free-function constructors / `Condition::holds` | Increase the places that require a contract (in stages) | the user **removing** the requirement rather than escaping it — `dyn` erasure over a public handle (**27**). Adding places that require a contract does not reduce the places that do not |

> **Never leave a cell in the last column blank**, and never write `—`. The rule is
> [api-surface.md](../rules/api-surface.md) §2's, applied to the taxonomy: in #9 a
> blank in the seal table pointed straight at where the hole was. #44 found four
> paths at once, and **every one of them sat in the blind spot of a remedy** — which
> is a property of cutting the causes by *remedy* rather than by boundary.
>
> **A fourth cause was proposed and is not adopted** — *a macro's authority is
> bounded by its expansion form and position*. It is real, and #44's own analysis
> shows it explains paths 21, 22, 13's M2 hole and 26. It is not a *cause* here
> because it does not cover 24 or 27, so adopting it would re-cut the taxonomy by
> mechanism and still leave two paths outside it. Where it lands instead is
> [ADR-0015](../adr/0015-remedies-state-what-they-do-not-reach.md), sharpened by
> what #44 measured: the bound is the attribute's **position**, not only its
> expansion site.

---

## The full ledger

### Cause 1: how the domain model is exposed

| # | Route | Response | State |
|---|---|---|---|
| 1 | `user.email = v`, direct assignment | ~~Domain opacity (private fields)~~ → **the field is not private, it is gone from the type**, and the inner field sits in a module `#[domain]` owns ([ADR-0011](../adr/0011-domain-is-an-attribute-macro.md)); below | **Closed in the First PoC** — measured, P18 (`E0615`) / P4 (`E0616`). ⚠️ **The remedy as originally written was insufficient**, and the status was right for a reason the row did not give |
| 2 | `*user = other_user` (fetch two with `find` and swap) | Restricting the construction route does not close it | **Stated** (below) |
| 3 | Escaping a projection with `into_owned()` | **Not provided** | **Closed in the First PoC.** ⚠️ A derive on the `Repr` brings it back — the route's general form is [`../rules/api-surface.md`](../rules/api-surface.md) §8, `Clone` is this path's instance (see path 21). **Confined to the domain's own module** once the `Repr` carries no visibility modifier either (P30, ADR-0010) |
| 4 | A data leak through `Debug` / `Serialize` | A custom implementation emitting declared fields only, derive-generated | **Closed in the First PoC.** ⚠️ The response is **imposed only on the domain side** — a derive on the `Repr` leaks from within the same crate ([`../rules/api-surface.md`](../rules/api-surface.md) §8; `Debug` is this path's instance, see path 21). **Confined to the domain's own module** once the `Repr` carries no visibility modifier: the name is unreachable, so there is no `Debug` to call (P30, ADR-0010) |
| 5 | Mutation through interior mutability (`RefCell` / `Mutex` / `Cell`) | ~~Restrict domain field types (a whitelist until `Freeze` stabilises)~~ — a **name-based** whitelist cannot hold; a **bound-based** one reaches part of it and `Freeze` reaches less than was claimed; see path 28 | ⚠️ **Stated** (was "Closed in the First PoC"; #44). The mutation needs only `&self` — P45, run-verified |
| 21 | **`User::from_repr(UserRepr { .. })` / `as_repr()` reachable from anywhere in the domain's crate** | The derive emits the `Repr`, the constructor **and** the repository into a **macro-owned private module** and re-exports the domain from it (#33 / [ADR-0010](../adr/0010-domain-constructor-confined-by-module-privacy.md)) | **Closed** for all user-written code — handler, service, a helper beside the user's own declaration, the crate-root layout, the read half, and foreign crates are all `E0624` (P31/P32/P34/P35/P27). ⚠️ **Conditional**: closed only while the conversion stays an *inherent* method — on a public trait it reopens completely (P36). Measured in T-M1-01 / #13 and #33; below |
| 26 | **A derive the *user* attaches hands out a Domain, from a foreign crate.** `Default` invents one, `Deserialize` sets every field from a string, `mem::take(&mut u)` reinitialises through a `&mut` alone, `u.clone()` and `Copy` take a copy. **This is not path 21**: it never touches `Repr`, it crosses a crate boundary, and it survives any `Repr` redesign | **`#[domain]` emits the conflicting impl** for the traits verum can name, so the user's derive is `E0119` regardless of position or spelling; `Copy` is `E0204` unless `repr_derive(Copy)`, which is the attribute's own argument list. The layer-1 name check remains for `Deserialize` only ([ADR-0015](../adr/0015-remedies-state-what-they-do-not-reach.md)); below | ⚠️ **Stated, and the coverage is not uniform.** `Default` / `Clone`: closed (P42, P47). `Copy`: closed (P48, P49) — and it **became reachable because closing `Clone` supplied the bound `#[derive(Copy)]` needs**. `Deserialize`: **lint only** (P50) — verum has no serde dependency, so there is no impl to collide with, and the check is blind above the attribute and to any alias. Forged and read from a foreign crate wherever nothing closes it — P43, run-verified |
| 28 | **A field type carries interior mutability and `&self` is enough to use it.** The mutation goes through the getter, so nothing is forged and no `&mut` is needed. A **name** check over field types cannot hold either way — an allow-list turns away the user's own value objects (P51), a deny-list is bypassed by one type alias (P53) | **Partly available, and not `Freeze`.** A whitelist emitted as a **bound** resolves the alias, because rustc does it; that closes `Cell` / `RefCell` today at the cost of also forbidding `Rc`. It does **not** reach `Mutex` / `RwLock` / atomics, nor `Arc<Mutex<..>>` behind an indirection. Below | ⚠️ **Stated.** Compile- and run-verified from a foreign crate through a **correctly loaded** Domain — P45. ⚠️ Because `&self` suffices it is available where `Mutates = ()`, so [`capability-system.md`](./capability-system.md)'s read-only guarantee does not reach it |

> **Numbers are only ever appended, never renumbered.** Paths 12 / 13 / 14 are
> referenced from [`../rules/api-surface.md`](../rules/api-surface.md),
> [`../rules/proc-macro.md`](../rules/proc-macro.md) and the knowledge banks;
> renumbering would silently break every cross-reference. Stability of the numbers
> takes priority over grouping by cause.

#### path 21 — `Repr` cannot be confined to "the repository implementation" as written, but can be confined to the domain's module (compile-verified)

`#[domain]` **expands in the user's crate**, so the visibility of the
generated `pub(crate) struct UserRepr` and `pub(crate) fn from_repr` / `as_repr`
is **the whole application crate.** The derive cannot emit `pub(in ...)` because it
does not know which module the repository is written in. That leaves two readings
of "the repository implementation", and **neither works** — plus a third that does,
found in #33 by dropping the assumption that the repository is written by the user
at all.

| Where the repository lives | Measured |
|---|---|
| The same crate as the domain (a single-crate application) | Any handler in the crate can write `User::from_repr(UserRepr { email: anything, .. })`. No capability, no repository, no SQL, no `unsafe` |
| A separate crate | `Repr` is entirely invisible (`E0603`). The design does not function |
| Generated into **the domain's own module** | **Not enough** (#33). Handlers are rejected (`E0624`, P26), but a helper beside the user's own `struct User` forges (P29), and if the domain sits at the **crate root** the module *is* the crate, so nothing is confined at all (P33) |
| **Generated into a macro-owned private module** (#33, [ADR-0010](../adr/0010-domain-constructor-confined-by-module-privacy.md)) | **The one that holds.** The radius is chosen by the derive, not by the user's layout: P31, P32, P34, P35 and P27 are all `E0624`, while the generated repository inside still loads a real row (P28) |

**The `pub(in ...)` sentence above is true and does not imply what it was used to
imply.** The derive does not need `pub(in ...)`; it needs *no modifier*, plus a
legitimate caller inside the resulting scope — which is why generating the
repository is load-bearing. **Generating it is not sufficient on its own**: probe
P2 forges from a handler while `app/src/repo.rs` sits in the same crate. It is the
placement that closes the path, not the generation.

**What the confinement's radius must be, measured.** Confining to *the user's*
module leaves two holes: a helper beside the user's own declaration forges (P29),
and at the **crate root** "no modifier" is `pub(crate)`, so the mechanism is vacuous
(P33). Both close when the derive owns the module (P31, P32). The lesson is
RK-016's — a guard must not depend on placement — applied to a type-level mechanism.

**What keeps path 21 in the ledger.** The closure is **conditional on the conversion
staying an inherent method**. A trait method's visibility is the trait's, so moving
the conversion onto a public framework trait — the obvious shape for a generic
`Repo<D: DomainRepr>` — reopens it from every crate (P36, `Finished`). Path 21 keeps
its AI Context entry for that reason, and because ARK-002 is **not** satisfied: the
`E0624` carries no pointer to the generated repository, and `rustc --explain E0624`
names both bypasses by number.

**No trait-bound gate can replace this, and the reason generalises.** A gate whose
guard is a trait needs the trait sealed, and a seal cannot be satisfied by code the
derive expands **inside the user's crate** — whatever the expansion writes, a
hand-written impl writes too. Measured against both candidates: a token passed by
value is stolen through the user's own `impl Repository` (P23), and an
`impl RepositoryProof for MyProof {}` is a foreign trait on a local type, allowed
from the application crate (P24) and from any other crate (P25). Consequence for
diagnostics: the rejection here is a **visibility error and cannot be reworded**,
the same constraint already recorded for `E0615` / `E0609` below.

**The comparison with path 2 splits by axis. It is not strictly worse.**

| Axis | Which is worse |
|---|---|
| Freedom of values | ~~**21.**~~ **Neither, once path 26 is on the table.** This row said path 2 "can only insert values `find` actually returned". With `Default` derived, `mem::take(&mut u)` puts a Domain **nobody built** where a loaded one was, and with `Deserialize` derived the values are the attacker's — from a foreign crate, measured (P43). Since the same table classifies path 2 as **permanent**, the understatement was on the permanent side, and it was being used to prioritise how path 21 gets closed. What remains true is that 21's *preconditions* are lighter: no capability and no `find` result |
| Reach and permanence | **2.** Path 2 holds across a crate boundary given a `&mut D`, and **survives closing 21** (it is classified below under what cannot be closed in principle). 21 is crate-local and disappears once closed |

A forged `User`'s getters were confirmed to work at run time (probe P8 of the
spike). But **it was not compared against a loaded `User`**, so "indistinguishable"
is an observation about getter behaviour.

> **⚠️ No longer true under `#[domain]`** ([ADR-0011](../adr/0011-domain-is-an-attribute-macro.md)).
> The attribute expands into a module that is a **child** of the user's module, so a
> helper written beside the declaration cannot reach the **constructor** —
> `E0624`, measured. What it *can* still reach is the `Repr`, if the `Repr`
> carries any visibility modifier; with none (as `#[domain]` emits, and as the
> ledger states as paths 3/4's closing condition) that is `E0603` too.
> An earlier version of this note said `E0616` and said the helper was outside
> the radius outright: `E0616` is the *field* error (`u.0.email`), which is a
> different measurement, and the unqualified claim was false for the `Repr`.
> The residue described below is the **derive** form's.

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

**The route's general form is stated once**, in
[`../rules/api-surface.md`](../rules/api-surface.md) §8 — including why an
enumerated ban list is the wrong shape for it. This paragraph used to restate it
and to enumerate the derives; #35 reduced it to this pointer.

What was asserted here without evidence is worth naming: "`FromRow` and
`Deserialize` are the same mechanism". **P41 measures it** (#35) — a foreign crate
forges from a JSON string alone, no database. §8 carries both instances.

Reproduction and the probe table: `spikes/domain-opacity-sqlx/`
(`bash run.sh`). The specification-side account is in
[`persistence.md`](./persistence.md) §Verdict. **Not deciding how to close it in
the spike was deliberate** — this route is the domain's exposure form itself, so
the choice determines the shape of M2's derive tasks (ARK-002: blocking without an
alternative pushes people onto unchecked routes).

#### paths 26 and 28 — what closes them is placement, and it is not a check (compile- and run-verified)

Both are cause 1's, both are outside its remedy, and neither is path 21. Reproduce:
`spikes/domain-opacity-sqlx/` (`bash run.sh`), probes **P42**–**P46**.

**Path 26 — the derives the user attaches.**

`read-contract.md` records the remedy as "forbid `Deserialize` on a domain". #44
measured that a derive cannot see its sibling derives and an attribute can, and
concluded that the attribute form can enforce it. **That is true of one position.**

| Where the derive is written | What happens |
|---|---|
| **below** `#[domain]` | the attribute sees it in its own token stream and rejects it — **P46** / **P50**, layer 1, verum's own wording |
| **above** `#[domain]` | **the attribute never receives it.** rustc expands an item's outer attributes in **source order**, and the first active attribute macro consumes the rest; a derive listed above is therefore expanded *first* and applies to whatever the attribute emitted. **P42** |

> **The mechanism sentence above was wrong in an earlier version**, which said the
> attribute is expanded first and that "dropping it from the re-emitted item does not
> suppress it". The attribute never has it, so there is nothing to drop — and only the
> accurate version predicts P42's `E0560`, because it explains why the derive's
> generated code names the *original* fields.

**And the check is blind to spelling as well as position.** `r#Default` yielded
`"r#Default"` from `to_string()` and matched nothing; an aliased import
(`use core::clone::Clone as Dup;`) matches nothing in principle, because a proc
macro resolves no names. **This is the same argument this file uses against path 5's
name-based field whitelist**, and it applies here too — so the name check cannot be
the guarantee for any trait.

**What is the guarantee: the conflicting impl.** The attribute does not need to see
the derive; it needs to occupy the coherence slot. `#[domain]` emits its own
`impl Default` and `impl Clone`, so the user's derive is `E0119` with the span on
their own derive, in **every** position and under **every** spelling — coherence
reads neither. **P42** / **P47**.

| Derive | Closed by | Reaches every position? | Every spelling? |
|---|---|---|---|
| `Default`, `Clone` | the emitted impl → `E0119` | **yes** | **yes** |
| `Copy` | `E0204` (the `Repr` carries no derive), plus a check on `repr_derive(..)` — the attribute's **own** argument list | **yes** | **yes** |
| `Deserialize` | the layer-1 name check only | no | no |

> **ARK-002 is not satisfied for `Clone`.** Duplicating a Domain is a legitimate
> need — two call sites, a before/after comparison — and the checked alternative for
> it is a `Projection`, which path 3 records as **"Not provided"**. So `Clone` is now
> blocked (`E0119`) with nothing to point at, which is the shape this file warns
> about: the type wall is high, so the AI walks around it. The emitted message names
> the repository, which is the alternative for *obtaining* a Domain and not for
> copying one. Until `Projection` exists, this is a known cost rather than a closed
> route.

**`Copy` is on that table because closing the other two opened it.**
`#[derive(Copy)]` requires `Self: Clone`; that was unsatisfied, so `Copy` failed
with `E0277` — an *incidental* barrier that emitting `Clone` removed. A bit-copy
duplicates the Domain without calling the emitted `clone`, so the
`unimplemented!()` body is no defence. Recorded because a reader would otherwise
take the `E0119` row as strictly better with no cost.

**What placement does and does not reach**, measured by isolating shape from
position:

| Emitted shape | Emitted where | Result |
|---|---|---|
| newtype (what `#[domain]` emits) | the user's module | rejected, **`E0560`** — but only because the derive's generated code names fields the attribute deleted. An *accidental* rejection, and it disappears the moment `#[domain]` preserves the field names |
| field names preserved | the user's module | **compiles, and the forgery runs from a foreign crate** — `serde_json::from_str` with the attacker's values, `clone`, `mem::take`. **P43** |
| field names preserved | a **macro-owned child module** | rejected, **`E0616`** (`Clone` reads the field). With `Default` alone the same source is **`E0451`** (construction). **P44** |

**Placement is the mechanism — for a derive whose generated code names a field.**
`#[derive(Clone, Copy)]` above the attribute **compiles under the confined shape**:
with `Copy` present, `Clone`'s derive emits `*self` and names nothing, so it meets
neither the newtype mismatch nor the field privacy. That is **P49**, and it is why
the unqualified form of this sentence was withdrawn — the qualification is what
makes `Copy` need its own remedy.

Placement still matters, because it is what ADR-0010 chose for path 21 and it
happens to cover the field-naming derives: **if the confinement radius is ever
relaxed, two paths reopen**, and until this entry existed only one of them said so.
Both rows name the dependency now, and so does
[ADR-0015](../adr/0015-remedies-state-what-they-do-not-reach.md).

**Path 28 — the field type's insides, and what a remedy can actually reach.**

Path 5's remedy restricts the *field types*, and **both horns of it are now
measured** — P45 measures only the `&self` escalation, so the remedy's failure was
desk analysis until probes P51–P54 existed:

| Form of the remedy | Result |
|---|---|
| an **allow-list** of permitted field-type names | **P51**, rejected: it turns away `Email`, the user's own value object. "Too narrow" is not a risk, it is immediate |
| a **deny-list** of interior-mutable names | **P53**, accepted: `type Audit = RefCell<Vec<String>>` passes, because the macro compares the token `Audit`. **P54** is the control — written out, the same check rejects it |
| the same predicate emitted as a **bound** | **P52**, `E0277`: the macro passes the tokens into a bound position and **rustc resolves the alias for it** |

So "a derive sees only tokens" kills the *name* form and not the remedy. `struct
Money(Cell<i64>)` remains unmeasured and is a user-defined type rather than an
alias, so it is the same class as P53 with a different shape.

The escalation is that the mutation goes through `&self`:

```rust,ignore   // fragment, not a complete item
// from another crate, on a Domain built through its legitimate route
let readonly: &Order = &order;
readonly.audit().borrow_mut().push("written by a GET".to_owned());
```

No `&mut`, no capability, and **nothing forged** — this is a correctly loaded
Domain whose contents change through a shared reference. `&self` is what puts it
inside a GET, where `Mutates = ()` and
[`capability-system.md`](./capability-system.md) describes the read-only guarantee.
That guarantee is about the *setter's* where clause, and this route never reaches a
setter.

**The child-module confinement that closes 26 does not reach 28** — P45's `Order`
is emitted into the confined module and the mutation still lands. Opacity is a
property of the Domain; the insides of its field types are a different boundary.

**What a remedy reaches, measured (#44's review). Not `Freeze`.**

The remedy column said "the closing mechanism is `Freeze`, which is unstable", and
that was wrong in both directions.

| Mechanism | Reaches | Does not reach |
|---|---|---|
| a **name** check, either horn | nothing usable — an allow-list turns away the user's own types (**P51**), a deny-list is bypassed by one alias (**P53**, control **P54**) | both horns |
| the same predicate emitted as a **bound** (`fn assert<T: Sync>()`, or a sealed `DomainField`) | **`Cell`, `RefCell`, and the alias**, today on stable — **P52** | `Mutex` / `RwLock` / `Atomic*` / `OnceLock`, which are `Sync`. And it **forbids `Rc`** — the "too narrow" horn, now priced rather than hypothetical |
| `Freeze` (unstable) | `Mutex` and atomics **held directly** | **`Arc<Mutex<..>>`, `Rc<RefCell<..>>`, `&'static Mutex<..>`** — `Freeze` is about an `UnsafeCell` reachable *without* indirection, so the idiomatic shape passes it. Compile-verified on nightly, and the `Arc<Mutex<..>>` route was **run** through a real getter |

So `Freeze` is not "the" mechanism and not sufficient; the bound form is available
now and is partial. **A sealed `DomainField` bound is the closest thing to a
closure**, and it inherits path 13's problem — a user can write
`impl verum::DomainField for MyLocalType`, so it needs sealing, and the seal must
survive being derive-facing. That makes path 28 *conditionally* closable in the same
sense path 13 is, which is a very different statement from "impossible until
`Freeze`". None of it is adopted here; what changes is that the enumeration is no
longer one item long ([SRK-009](../../dev/spec/review-knowledge.md)).

### Cause 2: the lifetime and route of a capability

| # | Route | Response | State |
|---|---|---|---|
| 6 | Carrying `Ctx` out with `tokio::spawn` | `Ctx<'req, E>` (not `'static`; `Send` preserved). The promised alternative `ctx.spawn::<Job>` is **provided and measured** — #40 / F4, a payload in and the job's context built inside the task, so `'job` is a borrow and nothing capability-carrying is `'static` (F5/F6 are the controls). The shape originally specified — the child borrowing the parent — does **not** compile (F1) and was replaced rather than kept | **Closed in the First PoC** — measured, T-M1-02 probe C1 (`E0521`) |
| 7 | Handing it to a `static Sender<Ctx<E>>` | Same, and the same missing alternative | **Closed in the First PoC** — measured, probe C3 (`E0521`) |
| 8 | Leaking out of a `when` scope with `Ok(ctx)` | **Not the return type** — the higher-ranked `Ctx` closes the specified signature; see below | **Closed for the specified signature. ⚠️ OPEN for a named `'req`** — reachable from an ordinary handler today (measured) |
| 9 | `Ctx::for_test()` as a god-mode constructor | Require a sealed `Runtime` token ([ADR-0006](../adr/0006-runtime-sealed-token.md), still `proposed`); testing goes through an API with a fixed endpoint type | ⚠️ **Stated, not closed.** What T-M1-02 measured is **visibility** — `Ctx::new` is `pub(crate)`, so `app` gets `E0624`. **No seal was measured**, and visibility alone was measured to leak (note below) |
| 10 | A `PgPool` on the endpoint struct | `#[endpoint]` rejects anything but a unit struct | **Closed in the First PoC** |
| 11 | Passing a `dyn Repository` to a service (the type parameters vanish) | ~~Do not expose `dyn Repository`; parameterise the service by capabilities too~~ — **verum exposes no `dyn` anything and the remedy still cannot be enforced**, because the user supplies the trait; see path 27 | ⚠️ **Stated** (was "Closed in the First PoC"; #44, compile-verified against the shipped `Repo`). Probes G1–G3 |
| 12 | A hand-written `impl Endpoint` declaring arbitrary capabilities | Seal `Endpoint` | **Closed in the First PoC** (nobody can satisfy the seal — no macros exist yet). ⚠️ **Reopens at M2 under the assumed emission shape** (#41, measured): if `#[endpoint]` emits `impl Endpoint for X` then `SealedEndpoint` must be nameable downstream, and proc-macro output is syntactically indistinguishable from hand-written code. A forged `Endpoint` declares any `Domains` — **and, once they exist, any `Reads` / `Mutates` / `Creates` / `Deletes` / `Emits` / `Calls`, so it dominates forging `Includes`**. Bounded two ways, both measured: `impl Endpoint for other_crate::X` is `E0117` (confined to the crate owning the endpoint type) and widening a set the derive already emitted is `E0119` (an attacker must introduce a *new* local type). ⚠️ **"Cannot be closed by types" was asserted and then refuted in review** — emitting a *type* rather than an impl (`pub type X = EndpointOf<Tag, Domains>`) leaves `derive_facing` empty and makes a downstream `impl Endpoint` `E0277`; compiled, though unchecked against `Reads`/`Mutates`/routing. **The emission shape is #60's to decide and is open, not closed.** AI Context: `forged_endpoint`, `permanent: true`; per ARK-005 an inventory check is the fallback *if* the impl-emitting shape is taken. [ADR-0013](../adr/0013-includes-is-a-blanket-impl.md) |
| 13 | `impl Includes<Order> for User` (a local type, so it passes the orphan rule) | ~~Seal `Includes`~~ → **make `Includes` a blanket impl**, so the derive never names its seal and the seal stays `pub(crate)` for good ([ADR-0013](../adr/0013-includes-is-a-blanket-impl.md), #41) | ✅ **Closed, and it survives M2** — measured downstream: `spikes/seal-after-m2` S3 rejects the forgery with Verum's own wording, S5 confirms a declared domain still resolves. ⚠️ **The seal alone did not survive**: with one impl per domain, exposing `derive_facing` as M2 requires lets an attacker write the seal impl too and two undeclared domains pass (S2). Not implemented yet — needs `Endpoint` (#60). Conditional on path 12: a forged `Endpoint` declares any `Domains`, and the blanket faithfully reports it |
| 14 | Forging `impl Field<...>` (forging `Field::NAME` forges the column name in generated SQL) | Seal it | **Closed in the First PoC** (`Field` unimplemented). ⚠️ **Reopens at M2** (#41): `Field::NAME` is per-field *data*, so the derive must emit one impl per field and must name the seal. **The blanket trick that saved path 13 does not apply** — there is nothing to derive the value from. Forging `NAME` forges a column name in generated SQL, so this is the most damaging of the four. AI Context: `forged_field`, `permanent: true` |
| 14a | `impl Has<Elem, Idx> for <set>` — forging membership itself. **The head position (`Here`) and non-head positions (`There<_>`) are separate routes** | Seal `Has`, **and make the seal's recursive impl conditional too** | ✅ **Closed** (T-M0-08 / #8; `has_cannot_be_forged.rs` + `has_cannot_be_forged_at_depth.rs`) — read the note below |
| 14b | `impl ConsList for MyType` — forging the shape proof, making a malformed set look well-formed | Seal `ConsList` | ✅ **Closed** (T-M0-07 / #7; `cons_list_cannot_be_forged.rs`). The tuple shape is also closed by the orphan rule (E0117) (re-checked in T-M0-08) |
| 14c | `impl Index for MyIdx` — forging the position of membership | Seal `Index` | ✅ **Closed** (T-M0-07 / #7; `index_cannot_be_forged.rs`). `There<MyIdx>` and `There<There<MyIdx>>` are closed by the orphan rule too (re-checked in T-M0-08) |
| 14d | `impl Append<B> for <set>` — forging the concatenation result. It has a `type Out`, so **the composed capability set itself can be named** | Make `Append`'s seal **match** the trait (including the base impl's `B: ConsList`) | ✅ **Closed** (T-M0-09 / #9; `append_cannot_be_forged_at_base.rs` + `_at_depth.rs`). **It had once been closed on a miss** — note below |
| 14e | `impl Lookup<K, Idx> for <map>` — forging "the entry for this key is this", swapping a conditional scope arbitrarily | Make `Lookup`'s seal **match** the trait (including the head impl's `T: ConsList`) | ✅ **Closed** (T-M0-09 / #9; `lookup_cannot_be_forged_at_head.rs` + `_at_depth.rs`). **It had once been closed on a miss** — note below |
| 14f | `impl Has<H, Idx> for (H, <non-cons-list>)` — **passing a malformed set through the capability check.** **The head and deep positions alike** (`impl Has<Other, There<There<Here>>> for (Decl, (Elem, (Other, Junk)))` compiles). Membership itself is true, so no capability is gained | None (`Has`'s seal deliberately drops `ConsList` for diagnostics — `SEAL-DIFF`) | ⚠️ **Stated** (until T-M2-09). `ConsList`'s "a malformed set fails closed" can be defeated downstream. But **only when the element is a bare local type** — a real effect type `Mutate<User, Email>` wraps the local type in verum's generics, so the orphan rule (E0117) closes it (measured). So it is **unreachable for effect sets and limited to domain-shaped elements.** `has_forged_membership_on_malformed_set.rs` (head) and `has_forged_membership_at_depth_on_malformed_set.rs` (depth) pin the side where *false* membership is rejected, at both positions. **It is not "permanent"** — once T-M2-09 asserts the shape at the declaration site, or conditional `on_unimplemented` stabilises, the `SEAL-DIFF` justification lapses and the bound can be restored |
| 24 | **A capability handle outlives the request that granted it.** `ctx.users()` returned `Repo<D, R, M>` with **no lifetime**, so it owned its access and was `'static` — the `Ctx` was contained and everything it handed out was not | Give the handle the request lifetime: `Repo<'req, D, R, M>` (#39, [ADR-0005](../adr/0005-repo-handle-shape.md)) | ✅ **Closed in the First PoC** — the parameter and its two fixtures exist; there is no `Ctx` and no `ctx.users()` yet, so nothing can *obtain* a handle either. The checked route for work that outlives the response is `ctx.spawn::<Job>`, **provided as of #40** (F4) — so unlike when this row was written, closing this path no longer removes the last workaround for that need (ARK-002 satisfied). ⚠️ **The producer side is unpinned** — `'req` is enforced on users of the handle, but with a `PhantomData` field an emitter can mint any lifetime (measured); #40. Measured **at run time** before the fix — probe **E1**: the response returned and the store read `escaped@example.com` 150 ms later. Both candidate shapes reject the escape (`E0521`, E2 / E4a) and both still serve an ordinary handler (E3 / E4b). Fixture: `repo_handle_cannot_outlive_its_request.rs`, **mutation-tested** — reverting `Repo` to its pre-#39 shape turns it green. ⚠️ **This was scope escape, not capability forgery**: field-granular checking survives it, which is why every one of #14's acceptance criteria passed while it was open. `!Send` was measured as an extra restriction and **rejected** (E5 breaks ordinary use, E5b shows it closes nothing — RK-017's shape). Which field carries `'req` is **#40's** |

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
> **⚠️ Why it was provisional, and what the provisional closure got wrong**: path 13
> was closed because **nobody could satisfy `SealedIncludes<D>`** — `verum-macros`
> emits no macros at all. And as [`../rules/api-surface.md`](../rules/api-surface.md)
> §2 records, **a proc macro's output resolves in the calling crate and so cannot
> reach `pub(crate) mod private`** (E0603, measured). M2 is forced to introduce
> `#[doc(hidden)] pub mod __private`, and §2 says that is the moment the seal's
> strength drops. It is worse: the seal **stops working**, and #41 measured it (S2).
>
> ### The re-verification procedure recorded here was itself defective (#41)
>
> It read: *"after `__private` is introduced, with the derive emitting one domain's
> seal, confirm in both directions that `impl Includes<undeclared>` is E0277 and that
> a declared one compiles."*
>
> **Run verbatim on a tree where the forgery compiles, it is green.** That is probe
> **S1**, and probe **S2** — the same tree — compiles four lines that pass two
> undeclared domains through the Architecture Contract. The procedure models an
> attacker who does **not** write the seal impl. Once the seal is public, the attacker
> writes the seal too, because proc-macro output is syntactically indistinguishable
> from hand-written code.
>
> **The attacker model any seal procedure must assume**, stated so it is not
> re-derived wrongly: *the attacker can write every impl the derive can write.* A
> procedure that only exercises the trait impl tests nothing about a public seal.
>
> This is the **fourth instance** of one class — #6 (type parameters), #8 (recursion),
> #9 (a bound on a parameter not in `Self`) — and the first where the defect is in a
> **verification procedure** rather than in an implementation. `api-surface.md` §2's
> rule that a closure's justification must cover every impl position now applies to
> procedures as well as to seals.
>
> **The replacement is not a better procedure but a different mechanism**: path 13 is
> closed by making `Includes` a blanket impl, so the derive never names the seal and
> the seal never becomes public. See [ADR-0013](../adr/0013-includes-is-a-blanket-impl.md),
> and `spikes/seal-after-m2/` for S1–S5 including **S4, which refutes the reason #41
> originally gave** (coherence does *not* reject the competing impl — it judges the two
> disjoint exactly where the blanket does not apply).
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
| 20 | ~~`Condition::holds` unlocks everything by returning `true`~~ → **it unlocks the effects declared under that condition** — and it is **arbitrary user code the framework calls on every request**, taking `&Domain` and `&Request`; below | **Impossible in principle.** "Require purity by convention" is not a check — `holds` lives in another impl, which is path 26's reason | **Permanently stated.** ⚠️ **Its blast radius is bounded by `E::Conditional` being derived from the declaration, and that bound depends on path 12**, which reopens at M2 |
| 23 | **`Debug` / `Serialize` / a free function reads a field the endpoint did not declare in `reads`** — `kind: uncapped_read` | Capability-check the getters (measured to work) and accept that these routes are outside them. A `Projection`'s **own** derived `Debug` does narrow to the declared set (#15, P4), but the `Domain` value still exists and its `Debug` and any free function taking `&Domain` reach every field | **Stated** (measured, #15) |
| 22 | **The `syntactically_present` scan is neither complete nor sound.** *(a)* It cannot leave the item it is attached to — a free associated function taking `&ctx`, a helper in a sibling `impl`, and **an effect produced by a `macro_rules!` expansion** (the last is unreachable even with cross-item analysis, since the macro may come from another crate). *(b)* Within the item it matches by **spelling** — the handler parameter named anything but `ctx` voids every key at once; `let repo = ctx.users()` and UFCS are missed. *(c)* It runs **before cfg-stripping**, so it reports effects from code that is never compiled | (a) annotate every effect-carrying item and take the transitive closure at build time (a future form); (b) is closable in the scanner and at layer 1; (c) has no fix — the tokens are all there is. **`scope: "ctx_spelled_same_item"` overstates (a) and says nothing about (b) or (c)** and needs replacing | **Stated** (Q-A / 2026-08-15; **rewritten by T-M1-07 / #37**, compile-verified. Enumeration is of *observed* classes, not a census) |
| 27 | **The user erases the capability parameters themselves.** They define their own object-safe trait, implement it for `Repo<'req, D, R, M>` — **a blanket impl covers every capability shape from one line** — and pass `&dyn`. `D`, `R` and `M` vanish from every signature downstream, so no transitive closure can be taken over the service | **None available.** verum exposes no `dyn` anything; `Repo` is public because it has to be, a local trait with a foreign `Self` passes the orphan rule, and no set of exports forbids it at all — a blanket impl need not name `Repo`, and a closure needs no trait | ⚠️ **Stated, and the remedy is unenforceable** — G1/G2/G3, compile-verified against the shipped `verum::Repo` from a crate that only depends on it. ⚠️ **It does not defeat `'req`** (G4): this is capability erasure, path 24 was scope escape, and the two are kept apart for that reason |
| 29 | **The generated repository is a `pub` unit struct, so any crate can mint one.** [ADR-0010](../adr/0010-domain-constructor-confined-by-module-privacy.md)'s listing re-exports `Account` **and `AccountRepository`**; the repository holds no state, so a foreign crate writes `AccountRepository.find(&its_own_pool, 1)` and receives a Domain whose every field it chose. No `Ctx`, no `Includes`, no capability, no `Repr`, no derive | **Not provided.** Acquisition has to be bound to the `Ctx` — the repository reachable only as something `ctx.users()` hands out, or its constructor requiring a `'req` token — and that is #60 / ADR-0006's shape to decide, not decided here | ⚠️ **Stated.** Compile- and **run**-verified from the foreign crate the path names (`VERUM_MINT=attacker@example.com`, `spikes/domain-opacity-sqlx`). ⚠️ **This is path 21's checked alternative** — the route ARK-002 required in exchange for closing 21 — so it is not a defect *beside* that closure but its cost, and `persistence.md`'s foreign-crate row read `E0624` because P27 measures `from_repr` and nothing measured the repository |
| 25 | **An external effect fires before the commit** — `handler-rules.md` Rule 4's ordering ("send the mail *after* the transaction") is a convention, not a type | **Not provided in the First PoC.** The designed mechanism — issue the effect capability only inside `ctx.after_commit`, scoped the way `when` is — needs a transaction boundary, and that is not designed ([`research-questions.md`](./research-questions.md)) | ⚠️ **Stated, not enforced.** `'req` (path 24) stops a handle leaving the request; it does not order effects *within* it. Rule 4's sample code is written in the correct order because Verum supplies the template an AI imitates — **that is the whole of the mechanism today**, and this row is what stops it reading as a guarantee. Recorded by #39, which closed path 24 and found Rule 4 resting on it |

#### path 27 — the remedy is unenforceable because the user supplies the trait (compile-verified)

Path 11's remedy read "do not expose `dyn Repository`; parameterise the service by
capabilities too", and its status read "Closed in the First PoC". **verum does not
expose `dyn` anything**, and the path is open anyway. Reproduce:
`spikes/ctx-lifetime-rpitit/` (`bash run.sh`), probes **G1**–**G4**, against the
shipped `verum::Repo` through a crate whose only dependency is verum.

```rust,ignore   // fragment, not a complete item
// the user's crate, and every line of it is ordinary safe Rust
pub trait AnyService { fn touch(&self); }
impl<D, R, M> AnyService for verum::Repo<'_, D, R, M> { fn touch(&self) {} }   // G2

pub fn service(handle: &dyn AnyService) { handle.touch(); }                    // G3
```

Three things make it unclosable rather than merely open:

* **`Repo` has to be public.** It is what `ctx.users()` returns.
* **A local trait with a foreign `Self` passes the orphan rule.** This is the same
  fact that made sealing necessary everywhere else (path 13, RK-009), and here
  there is nothing to seal: the trait is the *user's*.
* **One blanket impl covers every capability shape**, present and future,
  including sets no endpoint declared.
* **No set of exports forbids it at all**, and "unless `Repo` is unnameable" was
  too weak a way to say so. `impl<T> AuditAny for T {}` does not mention `Repo`, and
  `&Repo` coerces to `&dyn AuditAny` from it; a `&dyn Fn() -> _` closure erases the
  parameters with **no trait and no impl of the user's at all**. Both compile against
  the shipped crate. Making `Repo` unnameable — an opaque return plus a sealed trait
  — is also defeated, by a blanket impl over the *bound* rather than the type. The
  verdict is unchanged and its reason is stronger.

**What it costs and what it does not.** Downstream of the `&dyn`, `D`, `R` and `M`
are gone from every signature, so the transitive closure
[`effect-inference.md`](./effect-inference.md) would need over a service cannot be
taken — the service's *type* says nothing about which domain or fields it touches.
**It does not defeat `'req`** (**G4**, `lifetime may not live long enough`): `dyn`
erases type arguments and still carries a lifetime bound, so a handle borrowed for
the request cannot be laundered into a `&'static dyn`.

> **Path 27 is capability erasure; path 24 was scope escape.** Keeping them apart
> matters because the fix for one is not the fix for the other, and because #39's
> closure of 24 is not weakened by this. It is the same distinction the path-24 row
> draws to explain why every one of #14's acceptance criteria passed while 24 was
> open.

Field-granular checking is unaffected *inside* the impl — the user's method body
still cannot touch an undeclared field. What is lost is that anything **calling
through the `&dyn`** has no declaration to check against, and no AI Context key can
describe it.

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

> ### This path was described wrongly in **both** directions (#44)
>
> **Overstated: it does not unlock "everything".** What `holds` returning `true`
> unlocks is the effects **declared under that condition** — the declaration is
> still the ceiling, and [`mutation-contract.md`](./mutation-contract.md) already
> publishes the union of the conditional and unconditional sets. The lie is about
> *conditionality*, never about the effect set.
> [`conditional-effects.md`](./conditional-effects.md) words this correctly, and
> the row above did not.
>
> **Understated: `holds` is not a predicate with a hole in it.** It is **arbitrary
> user code that the framework itself calls on every request**, receiving
> `&Domain` and `&Request`, free to open a socket, read a clock, or write a file —
> none of which appears anywhere in the contract. That is why the path is filed
> under cause 3 (effects where no contract is required) and not merely as an
> unverifiable boolean.
>
> **And its remedy cannot be checked**, for path 26's reason: "require purity by
> convention" is a claim about the body of an impl the macro does not expand. A
> derive on the `Condition` type cannot see it — the same wall that makes the
> forbidden-derive check a lint.
>
> **The bound on the blast radius depends on a path that reopens** — *desk
> analysis, not compiled*: neither `Endpoint` nor `Condition` exists in
> `crates/verum` yet, so there is nothing to measure it against. What keeps a
> true-returning `holds` from unlocking the whole contract is that
> `E::Conditional` is **derived from the declaration**. A forged `Endpoint`
> declares any sets it likes (**path 12**, `permanent: true` since #41, and that
> half *is* measured — `spikes/seal-after-m2` S2), so path 20's containment rests
> on path 12 holding, and path 12 reopens at M2. Stated here because a reader of
> path 20 alone would take the containment as unconditional.

The response:

- Always emit `condition_verified: false` in the AI Context
- Make it a convention that a `Condition` implementation is a pure function (no
  external I/O, clock or randomness) — **a convention, and nothing checks it**
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
        "detail": "a domain's Repr and constructor are confined to a macro-owned private module (ADR-0010); no user-written code can build one or read a field without a capability. This holds ONLY while the conversion is an inherent method \u2014 on a public trait it is reachable from every crate (path 21)",
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
        "kind": "forged_endpoint",
        "detail": "at M2 the derive must emit `impl Endpoint for X`, so its seal is nameable downstream and proc-macro output is indistinguishable from hand-written code. A forged `Endpoint` declares any `Domains` \u2014 and, once they exist, any `Reads` / `Mutates` / `Creates` / `Deletes` / `Emits` / `Calls`, so it dominates forging `Includes` itself. Confined to the crate that owns the endpoint type (E0117) and cannot widen an endpoint the derive already emitted (E0119); a blanket `impl<D> Endpoint for Any<D>` covers unboundedly many types from one impl (path 12)",
        "permanent": true
      },
      {
        "kind": "forged_field",
        "detail": "`Field::NAME` is per-field data, so the derive must emit one impl per field and must name the seal \u2014 the blanket shape that closes path 13 cannot apply, because there is nothing to derive the value from. Forging `NAME` forges a column name in generated SQL (path 14)",
        "permanent": true
      },
      {
        "kind": "forged_derive",
        "detail": "a derive the USER attaches hands out a Domain with no capability: Default invents one, Deserialize sets every field from a string, mem::take reinitialises through a &mut alone, Clone and Copy take a copy. Default and Clone are closed because #[domain] emits the conflicting impl (E0119, any position, any spelling); Copy is E0204 unless repr_derive(Copy), which is the attribute's own argument list. Deserialize is a LINT only \u2014 verum has no serde dependency, so there is no impl to collide with, and a name check is blind above the attribute and to any alias (path 26)",
        "location": "src/domain/user.rs",
        "permanent": false
      },
      {
        "kind": "mintable_repository",
        "detail": "the repository ADR-0010 generates beside the Domain is a `pub` unit struct that carries no state, so any crate constructs one and calls it with its own pool: the Domain that comes back is legitimately built and every field is the caller's. No Ctx, no Includes, no capability, no Repr, no derive. This is the checked alternative path 21's closure was paid for, so closing 21 without binding acquisition to the Ctx moves the route rather than removing it (path 29)",
        "location": "src/domain/user.rs",
        "permanent": false
      },
      {
        "kind": "dyn_erasure",
        "detail": "the user defines their own object-safe trait, implements it for Repo<'req, D, R, M> \u2014 one blanket impl covers every capability shape \u2014 and passes &dyn, so D/R/M vanish from every signature downstream and no transitive closure can be taken over the service. verum exposes no dyn anything; the trait is the user's, so there is nothing to seal and no export to withhold. It does not defeat 'req (path 27)",
        "permanent": true
      },
      {
        "kind": "aliased_interior_mutability",
        "detail": "an interior-mutable field type is written through the &self getter, so it is available where Mutates = () \u2014 a GET \u2014 and nothing is forged: a correctly loaded Domain changes through a shared reference. A NAME-based whitelist over field types cannot see it; the same whitelist emitted as a BOUND resolves the alias and closes Cell/RefCell today, at the cost of forbidding Rc, and does not reach Mutex/atomics. Freeze does NOT close it either: Arc<Mutex<..>> and Rc<RefCell<..>> satisfy Freeze (path 28)",
        "location": "src/domain/order.rs",
        "permanent": false
      },
      {
        "kind": "unscanned_effect",
        "detail": "the syntactically_present scan is neither complete nor sound: it cannot leave its own item (free functions, sibling impls, macro expansions), it matches receivers by spelling (a renamed ctx parameter voids every key), and it runs before cfg-stripping so it reports effects from code that is never compiled (path 22)",
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
First PoC:  6 permanent + 12 non-permanent
Full PoC:   6 permanent + 10 non-permanent (middleware and events handled)
Later:      6 permanent +  0 non-permanent
```

> **These three lines are the third copy of the two numbers, and they are now
> checked** — `check_json.py` parses this block, compares it to the emitted
> entries, and derives the `Full PoC` line by removing exactly the two kinds its
> own parenthesis names. It was `6 + 8` until #44's review: 11 − 2 = 9, and the
> pre-#44 value (`9 → 6`) carried the same off-by-one, so raising both by two
> preserved it. Eleven lines below, the counting rule asserted that there was **no
> third place for these numbers to disagree from**. There was, and this was it.

`permanent` never reaches zero. Not hiding that is this file's purpose. And the
count is a floor, not a total — `completeness: "best_effort"` says the list is
what has been found, so a *rising* count is a review working, not a regression.

> **The counting rule** (stated explicitly because a review noted "the number
> differs every time it is counted"): count **one-to-one with the entries emitted
> in the AI Context's `unverified_boundaries.entries`.** permanent 6 =
> `condition_body` (20) / `row_scope` (row-level permissions) / `domain_swap` (2) /
> **`forged_endpoint` (12)** / **`forged_field` (14)** / **`dyn_erasure` (27)**.
> non-permanent 12 = `middleware` (16) / `event_subscriber` (15) /
> `repository_impl` (17) / `constructor_body` (18) / `upsert_granularity` (19) /
> `domain_repr` (21) / `malformed_set` (14f) / `unscanned_effect` (22) /
> `uncapped_read` (23) / `forged_derive` (26) /
> `aliased_interior_mutability` (28) / `mintable_repository` (29).
>
> **The two numbers above are the only ones stated, and both are checked.** An
> earlier version of this note also said "these twelve entries" in prose while the
> enumeration held fourteen — a third copy of a count, in a sentence, with nothing
> comparing it to either of the other two. `check_json.py` now parses this note and
> asserts that each stated number equals the names beside it, that the union equals
> what the sample emits, and that each name's side matches the `permanent` flag in
> the JSON. So the numbers here can no longer disagree with the entries, and there
> is no third place for them to disagree from.
>
> **Paths 12 and 14 went from 3 to 5 permanent in #41.** Both were "Closed in the
> First PoC" only because `verum-macros` emits nothing yet; #41 measured that they
> reopen at M2 and cannot be closed by types, which makes them `permanent: true`.
> The first version of that change flipped their *ledger* rows and added **no**
> entries here — leaving two paths open and unemitted, which is the one state this
> file exists to make impossible ("An unchecked boundary is **always** emitted in
> the AI Context", above). Caught in review. `check_json.py` could not see it,
> because nothing was added for it to disagree with.
>
> **And the entries it did add went into one sample only.** `forged_endpoint` and
> `forged_field` reached this file's sample and never
> [`ai-context.md`](./ai-context.md)'s, so the two disagreed for five PRs while the
> paragraph below asserted they must agree — the ledger emitted 14 kinds and
> `ai-context.md` 12. Found in #44, by writing the check the paragraph below
> claimed already existed.
>
> **Paths 26, 27 and 28 were added in #44**, taking permanent to **6** and
> non-permanent to **11**. `dyn_erasure` (27) is the permanent one: verum exposes
> no `dyn` anything and the remedy is still unenforceable, because the trait is the
> user's. `forged_derive` (26) is non-permanent for an uncomfortable reason — it is
> closed today by a placement chosen for path 21, not by anything aimed at it.
>
> **Paths 18 and 19 were previously uncounted** because neither had a `kind` name
> decided, and this definition excluded 19 while counting 18 — so what the
> definition counted and what was emitted disagreed (#43 item 8). Both are now
> named and emitted, which #38 forced: `enforcement.voided_by` may only name a
> `kind` that exists, and both paths void `mutates`.
>
> The entries must **agree as a set** with both this file's sample and
> [`ai-context.md`](./ai-context.md)'s. Three places holding different values is
> why this note was rewritten, and `spikes/doc-code-blocks/check_json.py` now
> **does** make that agreement mechanical: it compares the two samples, parses the
> enumeration above, and checks each name's side against the JSON's `permanent`.
> Each of those was planted and confirmed red before being relied on.
>
> **What it still does not check, stated so this paragraph does not overreach
> again**: that every emitted `kind` is *named* by some `enforcement.voided_by`.
> These are not — `condition_body`, `row_scope`, `uncapped_read`,
> `forged_endpoint`, `forged_field`, `dyn_erasure` — so for the keys they void, a
> reader who stops at `enforcement` comes away believing the guarantee is
> unconditional, which is the exact failure the join is described below as
> preventing. Which keys each of them voids is a design question, not a copy edit
> (`dyn_erasure` voids the effects of a *service*, and there is no key for a
> service), and asserting the join here with an exemption list of exactly those
> names would be an assertion that cannot fail. **No count is given on purpose** —
> a number here would be a third copy of something nothing compares.
>
> Of #44's three new kinds, two do have an owner and were joined:
> `forged_derive` and `aliased_interior_mutability` both void `mutates`, beside
> `domain_repr` and `domain_swap`, which are the other two routes by which a
> Domain's values change outside the declaration.

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

**And it is narrower than the handler scope, too** (path 28, #44). `Mutates = ()`
is enforced through the *setter's* where clause, so a mutation that never calls a
setter is outside it: an interior-mutable field type is written through `&self`,
which a GET has. That happens **inside** the handler, so widening the scope to the
request does not reach it and neither does closing `middleware`. It leaves
`voided_by` only when `Freeze` stabilises.

> **There is no longer a `scope_of_readonly_guarantee` key.** It said exactly what
> those three keys now say — "all three empty, checked only inside the handler" —
> and a value derivable from its neighbours is a value that can disagree with them.
> It also overstated: a GET may still cause Logging, Metrics, Tracing, CacheRead
> and CacheWrite ([`effect-system.md`](./effect-system.md)), so nothing about it
> was read-*only*. See
> [ADR-0008](../adr/0008-guarantees-carry-scope-and-voiding-paths.md).

**Naming a guarantee's scope accurately matters as much as making the guarantee
stronger.**
