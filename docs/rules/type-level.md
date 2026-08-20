# Verum — type-level programming rules

> Verum's capability system rests on type-level set operations. **The
> constraints here were confirmed by compiling**, and are fixed as rules.
> The canon of the design is
> [`../specs/rust-type-model.md`](../specs/rust-type-model.md).

---

## 1. Effect sets are cons lists

**Do not use a flat tuple `(A, B, C)`.** Membership cannot be implemented for
one.

```rust,compile_fail
// ❌ always E0119 — a coherence violation
impl<A, B> Has<A> for (A, B) {}
impl<A, B> Has<B> for (A, B) {}
```

```rust
// ✅ a cons list
type Mutates = (Mutate<User, user::Name>, (Mutate<User, user::Email>, ()));
```

| Set | Representation |
|---|---|
| Empty | `()` |
| One element | `(A, ())` |
| Two elements | `(A, (B, ()))` |

Users never write these — the derive generates them — but **they appear in error
messages.** §5 mitigates that.

### `ConsList` enforces the shape (T-M0-07)

A flat tuple **appears to work at exactly two elements**: `(A, B)` reads as head
`A` and tail `B`, and only breaks at three. Once, the only recourse was "pin it
with a UI test rather than by reading". **Now it can be a compile error.**

```rust,ignore   // needs a crate or a verum-private module this harness does not carry
pub trait ConsList: private::SealedConsList {}
impl ConsList for () {}
#[diagnostic::do_not_recommend]
impl<H, T: ConsList> ConsList for (H, T) {}
```

`(A, B)` is rejected because `B: ConsList` does not hold, and the error carries
the lesson itself:

```text
error[E0277]: `(Order, Item)` is not a well-formed effect set
   |    ^^^^^^^^^^^^^ effect sets are cons lists, not flat tuples
   = note: write `(A, (B, ()))` — a flat tuple `(A, B)` appears to work at
           exactly two elements and then breaks at three
```

**Put the bound on the impl, not on the trait.** Writing `L: ConsList` on the
trait definition forces every call site to restate it, and combined with the
index parameter (§2) that inflates both generated code and errors. On the impl,
call sites stay at `where S: Has<T, I>`.

> **The cost is in the diagnostic, not in the rejection** (an earlier version of
> this document got that backwards). On the impl, **rejection is exactly as
> strong, but `ConsList`'s message never reaches the reader** — the bound that
> fails at the use site is `Has`, not `ConsList`. Measured:
>
> ```text
> error[E0277]: the trait bound `(A, B): Has<A, _>` is not satisfied
>    | the trait `Has<A, _>` is not implemented for `(A, B)`
> ```
>
> No mention of a flat tuple at all, and the reader is told that `(A, B)` does
> not contain `A` — when `A` is the first element. **This is precisely the
> failure Verum exists to prevent**: an AI adds `A` again and lands on E0283.
>
> So **`Has` in #8 restates the flat-tuple note through `on_unimplemented`**. On
> top of that, having the derive emit
> `const _: () = { fn a<L: ConsList>() {} fn c() { a::<__Mutates>(); } };` per
> declaration makes a shape error surface **at the declaration site** (T-M2-09).

`ConsList` **only checks the shape.** `(A, (A, ()))` passes as well-formed and a
duplicate is still E0283 — **deduplication is 100% the macro's job.** A list
concatenated by nesting instead of using `Append`, such as `((A, ()), (B, ()))`,
is also well-formed and *understates* membership (fail-closed, but undetected).
**`Append` is not a convenience, it is required**, and T-M2-09 must test that.

```rust,ignore   // fragment, not a complete item
impl<H, T: ConsList> Has<H, Here> for (H, T) {}                       // ✅ on the impl
impl<H, X, T: ConsList, I: Index> Has<H, There<I>> for (X, T)         // ✅
    where T: Has<H, I> {}
```

As a defence layer this is a trait bound (layer 3). It is independent of the
derive not emitting flat tuples (layer 1, T-M2-09), and **both are needed** to
catch hand-written code as well as future generated code.

---

## 2. `Has` carries an index parameter

The naive recursive impl is a coherence violation. When `H == X` the two impls
overlap, and the tail's `T: Has<H>` is **satisfiable at that intersection**, so
it does not separate them.

```rust,compile_fail
// ❌ error[E0119]: conflicting implementations
pub trait Has<T> {}
impl<H, T> Has<H> for (H, T) {}
impl<H, X, T> Has<H> for (X, T) where T: Has<H> {}
```

The frunk-style index type parameter resolves it.

> ### ⚠️ "Coherence does not consider where clauses" is **wrong** (corrected in T-M0-08)
>
> This document said so for a long time. In fact the overlap check *does* consult
> where clauses, and **treats two impls as disjoint when it can show the
> obligation is unsatisfiable.** Measured on 1.85.0 — identical impl shapes,
> differing only in the tail:
>
> | Downstream `impl Has<Undeclared, There<Here>> for (Decl, TAIL)` | Obligation on `TAIL` | rustc |
> |---|---|---|
> | `TAIL = ()` | Unsatisfiable | **accepted** |
> | `TAIL = (Undeclared, ())` | Satisfiable | E0119 |
>
> So the overlap check lets through exactly the impls **where membership genuinely
> does not hold** — the harmful side. **Coherence is not a defence against a
> forged membership impl. Only the seal is.** This correction is the basis for
> "the seal must not drop the recursion" below; the wrong generalisation is what
> produced T-M0-08's Critical.

**The seal carries `Has`'s type parameters unchanged.** With `private::Sealed`
(taking only `Self`), the moment a derive emits one seal,
`impl Has<ForgedElem, Here> for ...` compiles (measured in T-M0-06,
[api-surface.md](./api-surface.md) §2). The seal's impls must **mirror** `Has`'s
— that shape has been confirmed to keep index inference working.

```rust,compile_fail
// ✅
pub trait Has<T, Idx>: private::SealedHas<T, Idx> {}   // one seal per trait, carrying its parameters

pub struct Here(PhantomData<()>);              // private field: not constructible downstream
pub struct There<I>(PhantomData<fn() -> I>);   // `fn() -> I` so `I`'s auto traits are not inherited

#[diagnostic::do_not_recommend]
impl<H, T: ConsList> Has<H, Here> for (H, T) {}

#[diagnostic::do_not_recommend]
impl<H, X, T: ConsList, I: Index> Has<H, There<I>> for (X, T) where T: Has<H, I> {}
```

### Only diagnostic-only bounds may be dropped from a seal. **Never the recursion** (T-M0-08's Critical)

A seal's impls mirror the trait's. But distinguish two kinds of bound.

```rust,ignore   // needs a crate or a verum-private module this harness does not carry
#[diagnostic::do_not_recommend]
impl<H, T> private::SealedHas<H, Here> for (H, T) {}

#[diagnostic::do_not_recommend]                                  // ✅ the recursion is kept
impl<H, X, T: private::SealedHas<H, I>, I> private::SealedHas<H, There<I>> for (X, T) {}
```

| Bound | Drop it? | Why |
|---|---|---|
| `T: ConsList` / `I: Index` | **Drop, for `Has` only** | Diagnostic-only. Keeping it makes a malformed set fail **on the seal**, producing the unrelated and unactionable "not sealed by Verum". Dropping it makes the failure land on `Has` with the membership message (both were tried). **This licence applies to predicate traits only** — `Has`'s head impl pins the very fact it asserts, so the remaining looseness admits only *true* assertions (`has_forged_membership_on_malformed_set.rs` pins this). It does not extend to a trait with a `type Out` |
| `T: SealedHas<H, I>` (the recursion) | **Never** | This *is* the enforcement |

Dropping the recursion makes `SealedHas<H, There<I>>` **hold unconditionally for
every two-element tuple.** With `H` unconstrained, downstream can forge
membership at any position except the head:

```rust,compile_fail
// while the seal was unconditional, this compiled and satisfied the bound at the use site
impl verum::Has<Undeclared, verum::There<verum::Here>> for (Declared, ()) {}
```

All three defences come off — the orphan rule (a local nominal type is permitted
in a type-argument position), the seal (unconditional), and coherence (which, per
the correction above, was never a defence). And **pasting the `help:` line out of
Verum's own `.stderr` produces exactly this impl** (confirmed by running it).
That is where "an AI responds to a trait-bound error by writing the missing
impl" — demonstrated experimentally — lands.

> **The general rule** ([api-surface.md](./api-surface.md) §2 is the canon):
> **a seal's solution set must equal the solution set of the trait it seals. The
> difference between them is the attack surface.**
>
> ⚠️ **This rule originally read "one fixture at the deepest impl position", and
> that did not prevent a third recurrence in #9** — which opened a hole while
> complying with the rule as written. The requirement now is (a) cover **every
> impl position**, (b) annotate each seal impl `SEAL-EXACT` or `SEAL-DIFF`, with
> a justification and a fixture required for any difference (enforced
> mechanically by the tests in `sealed.rs`), and (c) **allow no difference at all
> for a trait with a `type Out`.** The derivation is in §2 of that document.

### `Has` cannot produce the malformed-set diagnostic (measured, T-M0-08)

The cost of putting `ConsList` on the impl (§1) is settled here. **There is no
way to surface `ConsList`'s message through `Has`.**

| Configuration | Result |
|---|---|
| With `do_not_recommend` | `Has`'s message (line counts below) |
| Without it | Still no `ConsList` note, and **the reported type breaks** as well |
| Without `on_unimplemented` | The raw bound error |
| `on(...)` conditional | **Unusable on stable** — a `malformed` warning, which `-D warnings` turns into an error |

`do_not_recommend`'s effect **differs per fixture**. The measure is `wc -l` of the
`.stderr` (re-measured in T-M0-08 — the previous table said "16 lines with, 20
without", but two of those were measured against something else and matched no
fixture).

| Fixture | With | Without |
|---|---|---|
| `has_missing_element` | 16 | **22** |
| `has_empty_set` | 16 | 19 |
| `has_malformed_set_message_is_misleading` | 16 | 19 |
| `has_cannot_be_forged` | 17 | 20 |
| `has_cannot_be_forged_at_depth` | 17 | 21 |
| `has_duplicate_element` | 19 | 19 (E0283 cannot be suppressed by `do_not_recommend`) |

There is a side effect that matters more than the line count. Removing
`do_not_recommend` from `has_missing_element` **collapses the reported type from
the actual set `(A, (B, (C, ())))` down to the trailing `()`** — while the span
still points at the correct set, so the reader is given contradictory
information. That is the documented failure mode of `on_unimplemented` not
replacing the raw error, reproduced here on `Has`.

So **`Has`'s message targets the common case**: an undeclared element. Malformed
sets are asserted **at the declaration site by T-M2-09** (below).
`compile_fail/has_malformed_set_message_is_misleading.rs` **deliberately pins**
the current misleading message, so T-M2-09's improvement will appear as a
`.stderr` diff. The intent is in the filename because the filename is the only
thing that appears in a `TRYBUILD=overwrite` diff.

> ### ⚠️ "The index shape distinguishes the cause" is **wrong** (corrected in T-M0-08)
>
> An earlier version said: undeclared gives `Has<X, There<There<_>>>` (the depth
> walked), a malformed set gives `Has<A, _>` (index unresolved). But **the empty
> set `()` — the flagship `Mutates = ()` case — produces `Has<A, _>`** (measured;
> `has_empty_set.stderr` pins it). The most common case carries the
> "malformed set" signature, so **this cue is unusable.** Distinguishing the cause
> waits for T-M2-09's declaration-site assertion.

### Compile time: negligible at the design point, but **cost is roughly lookups × depth** (re-measured, T-M0-08)

> **⚠️ The previous table measured failed compilations.** It recorded "0.06–0.07 s
> at 25/50/100/200, lost in fixed overhead", but **N ≥ 128 does not compile at
> all** (below). Failures finish quickly, producing a flat and false curve. The
> same artefact led to the first, non-monotonic measurement being misdiagnosed as
> a noise floor.

Direct `rustc` invocation, `+1.85.0`, N membership lookups against an N-element
set, median of three. **Compilation success (exit 0) was confirmed at each N
before measuring.**

| N | Lookups | Median |
|---|---|---|
| 5 | 5 × depth ≤ 5 | 0.03 s |
| 10 | 10 × depth ≤ 10 | 0.04 s |
| 25 | 25 × depth ≤ 25 | 0.05 s |
| 50 | 50 × depth ≤ 50 | 0.09 s |
| 100 | 100 × depth ≤ 100 | **0.42 s** |
| 127 | 127 × depth ≤ 127 | **0.77 s** |
| **128 and above** | — | **`error[E0275]: overflow evaluating the requirement`** |

Control: a **single** lookup against an N=127 set takes 0.04 s. So **depth itself
is free and the cost is lookups × depth.** The table above grows both together,
making it roughly quadratic — "lost in fixed overhead" was wrong.

**A limit that had not been recorded**: at the default `recursion_limit` the
maximum is **127 elements**; 128 and above is E0275. That is far from the design
point (three or four elements per category,
[`../specs/effect-system.md`](../specs/effect-system.md)), and
`#![recursion_limit]` would raise it — but the limit's existence is worth
recording.

The conclusion is unchanged: **even at six times the design point (N=25) it is
+0.02 s over fixed overhead**, which is negligible. What changed is the shape of
the evidence — quadratic rather than linear, and bounded.

> **This only refutes "cost as a function of set length".** The real cost drivers
> are endpoints × sets × lookups and the volume of derive output, measured
> separately as the kill criteria in
> [`../specs/evaluation.md`](../specs/evaluation.md).

### The cost: every method carries an `I`

```rust,ignore   // fragment, not a complete item
fn set_email<I>(&self, u: &mut User, v: Email) -> Result<()>
where M: Has<Mutate<User, user::Email>, I>;
```

The derive generates it so users never write it, but **hand-written code inside
the framework always takes this form.**

### Duplicates are rejected by the macro

The index approach assumes each element appears exactly once. A duplicate leaves
`I` ambiguous and produces the unrelated E0283, "type annotations needed".

```text
error[E0283]: type annotations needed
note: multiple `impl`s satisfying `(Mn, (Mn, ())): Has<Mn, _>` found
```

There are **three** routes to it.

1. A user writes `mutates = [User::email, User::email]`.
2. **Appending for a `when` scope produces a duplicate** — `emits = [X]` together
   with `when(C) => { emits = [X] }`.
3. **A duplicate in `domains`** — `domains = [User, User]`. This route is new with
   [ADR-0013](../adr/0013-includes-is-a-blanket-impl.md): `Includes<D>` had no index
   parameter, so a duplicate produced a duplicated `impl Includes<User> for GetUser`
   and rustc rejected it **at the declaration** with `E0119`. Under the blanket
   `Includes<D, I>` the declaration is fine and **every use site** gets `E0283`
   instead — measured, with the raw `Has` impls leaking into the note, which
   `do_not_recommend` cannot suppress.

The second is a legitimate contract, so **the derive deduplicates before
appending.** The first and third are rejected by the macro
([`proc-macro.md`](./proc-macro.md)).

> **Route 3 is why the rejection has to be at the declaration.** Routes 1 and 2 are
> about effect sets, where a duplicate was always a macro-layer concern. Route 3
> moved a *declaration-site* `E0119` into a *use-site* `E0283`, so the error now
> lands on `ctx.users()` rather than on the contract — the exact swap `do_not_recommend`
> exists to prevent, in a position where it has nothing to suppress.

---

## 3. Which type-level operations are allowed

| Operation | Allowed? | Used for |
|---|---|---|
| `Has<Set, Elem, Idx>` — membership of one element | **Yes** (linear in element count) | Capability checking |
| `Append<A, B>` — concatenating cons lists | **Yes** (no coherence problem) | Composing capabilities for a `when` scope |
| `Lookup<Set, Key, Idx>` — type-level map lookup | **Yes** (the indexed form only) | Retrieving a condition from `Conditional` |
| `Subset<A, B>` — set containment | **Forbidden** (combinatorial explosion) | — |
| `Filter<Set, Pred>` — type-level filter | **Forbidden** (the catch-all impl always conflicts) | — |
| Negative reasoning (`NotHas`) | **Impossible** — no language feature | Use `Mutates = ()` instead |

### `Append` and `Lookup`, measured (T-M0-09)

**Only `Lookup` needs an index parameter.** `Append`'s two impls target `()` and
`(H, T)`, which are structurally disjoint and cannot overlap. An index is needed
only where **both impls target `(_, _)`**, as in `Has` and `Lookup`.

```rust,ignore   // verum-internal: legal only inside the crate that owns the trait or type
// ✅ Append — no index
impl<B: ConsList> Append<B> for () { type Out = B; }
impl<H, T: Append<B>, B> Append<B> for (H, T) { type Out = (H, <T as Append<B>>::Out); }
```

#### Put the shape bound on the impl where the recursion bottoms out

This was T-M0-09's most useful finding. Attaching `ConsList` to every impl is
**redundant**; one on the impl where recursion terminates enforces it throughout.

| Trait | Where the shape bound goes | The recursive impl |
|---|---|---|
| `Append` | `B: ConsList` on `impl Append<B> for ()` (**remove it and verum itself stops building**) | Not needed |
| `Lookup` | `T: ConsList` on `impl Lookup<K, Here> for ((K, V), T)` | Not needed — `T: Lookup<K, I>` already forces a two-element tuple, and its resolution terminates at the head impl |

`I: Index` is redundant on the recursive side for the same reason: impls exist
only for `Here` and `There<_>`, so a non-index cannot satisfy the recursive
premise. **All measured** — adding or removing them changes neither behaviour nor
`.stderr`.

> `Has` keeps `I: Index` on its recursive impl. It is equally redundant there, but
> `has_duplicate_element.stderr` reproduces the impl signature verbatim, so
> removing it would show up as a diff. Left alone as out of scope for T-M0-09.

#### `Out: ConsList` guarantees the composed result is well-formed

```rust,ignore   // needs a crate or a verum-private module this harness does not carry
pub trait Append<B>: private::SealedAppend<B> { type Out: ConsList; }
```

Call sites get this guarantee without restating a bound (measured). **`Append`
can never be the origin of a malformed set** — a property `Has` cannot have,
since it has no output.

#### The seal mirrors the recursion, and `type Out` makes it more dangerous than `Has`

`Append` and `Lookup` have a `type Out`, so a forged impl does not merely
*assert* something — it **chooses the resulting type.** Dropping the seal's
recursion lets it through from downstream (measured, in both directions).

> **⚠️ Correction (#9's review)**: an earlier version said "coherence partly
> covers `Append`, so forging onto a well-formed list is E0119". **Wrong.**
> Downstream rejection splits by shape into **E0117 (orphan rule), E0277 (seal)
> or E0119 (coherence)**. **E0119 does fire across crates** — "only within the
> crate" was wrong, and it was a second-order error written inside a sentence
> correcting a different one (the measured table is in
> [api-surface.md](./api-surface.md) §2). In other words, an in-crate measurement
> was being reported as a downstream defence.
>
> A more important correction: the route that was actually open was not "the tail
> is not a cons list" but **`B` being a local type** — `Self` was perfectly
> well-formed. `impl Append<Local> for ()` compiled, and **because every `Append`
> chain bottoms out at `for ()`, that one line rewrote every concatenation
> result.**
>
> The root cause fits in a sentence: **a bound on verum's impl constrains verum
> only.** The `B: ConsList` in `impl<B: ConsList> Append<B> for ()` is not imposed
> on a foreign impl, so if the seal lacks it there is no defence at all.
> "Removing it stops verum building" is true and irrelevant to safety. The detail
> and the re-derived principle are in [api-surface.md](./api-surface.md) §2.

#### The cost: a malformed operand produces two errors

Passing a malformed set to `Append` produces **the unrelated seal error, "cannot
implement a sealed Verum trait", alongside `Append`'s good message.** (The
equivalent case on `Has` produces one.)

> **⚠️ Correction (#9's review)**: an earlier version said this was "unavoidable,
> because a malformed operand and a forgery target have the same shape".
> **That argument misidentifies the cause.** Measured — same operand, same seal:
>
> | Form | Errors |
> |---|---|
> | `fn f<X: Append<Y>, Y>()` (a bound only) | **1** |
> | `<Malformed as Append<(C, ())>>::Out` (a projection) | **2** |
>
> The second comes from **projecting an associated type**, which reports the
> supertrait obligation separately. It is not the seal being strict.

The conclusion stands but the reason changes: a projection always produces two,
so writing `::Out` makes it unavoidable. It is acceptable because the useful
error comes **first** and both disappear with the same fix. Once T-M2-09 asserts
the shape at the declaration site, generated code will not hit it.

#### Compile time and the recursion limit (measured after confirming exit 0)

| N | `Append(N, N)` | `Lookup` (N key lookups) |
|---|---|---|
| 10 | 0.04 s | 0.03 s |
| 25 | 0.04 s | 0.05 s |
| 50 | 0.04 s | 0.11 s |
| 60 | 0.04 s | 0.15 s |
| 100 | **E0275** — 200 elements after concatenation exceeds the limit. *Failed*, not unmeasured | 0.58 s |

`Append` is **one resolution at depth N**, so it is flat in N. `Lookup` is N keys
× depth N, roughly quadratic — the same shape as `Has`.

**The recursion limit is set by the length after concatenation; how it is split
is irrelevant** (measured: `100+26` and `1+125` pass, **`100+27` and `1+126` fail
when consumed under a bound**, and `100+28` fails either way). **Always cite an
example that includes 127** — it is the only place 126 and 127 differ, and the
previous version cited only examples that skipped it.

> **⚠️ Correction (#9's review)**: an earlier version gave the limit as "127, the
> same constant as `Has`" and cited `100+27` and `1+126` as passing. **The limit
> depends on the harness, and those fail in the form that matters in practice.**
>
> | How `Out` is used | Limit |
> |---|---|
> | Just parked in a `PhantomData<Out>` | 127 |
> | **Consumed under a bound** (`Out: ConsList`, or passed to `Has`) | **126** |

> ⚠️ **Choosing the harness matters, and this point is easy to misread.** "An
> unused projection is not normalised, so it exits 0 past the limit" is true
> **only for a bare `type X = <A as Append<B>>::Out;` alias.** Putting
> `PhantomData<Out>` in a struct field or a function signature, as in the table
> above, **is normalised** (measured: E0275 at 128, 300 and 600). Choosing a
> `PhantomData` harness in the belief that it is on the non-normalised side gives
> the opposite of what the warning suggests.
>
> Consuming it costs one extra frame. **M8 feeds `Out` into a `Has` bound, so 126
> is the side that matters.** Stating the limit as a property of `Append` is the
> error — it shifts by one depending on what you do with the result. Cite 126, the
> safe side.

### Design constraints that avoid `Filter`

**`Conditional` is generated split by category.**

```rust
pub struct When<C, CondMutates, CondEmits, CondCalls>(PhantomData<(C, CondMutates, CondEmits, CondCalls)>);
```

Mixing them means needing `Filter` to "extract only the mutations from
`Conditional`". **This is a required design constraint, and the derive's output
shape must not change.**

### Express absence as `= ()`

Negative trait bounds (`!Trait`) are unstable, and a wildcard in a type parameter
(`NotHas<Mutate<_, _>>`) cannot be written.

```rust
// ✅ associated type equality bounds are stable
trait ReadOnly: Endpoint<Mutates = (), Creates = (), Deletes = ()> {}
```

The error is clear, too:

```text
expected unit type `()` found tuple `(Mutate<User, user::Email>, ())`
```

---

## 4. Do not enforce the method with a blanket impl

```rust,compile_fail
// ❌ the logic does not hold
impl<E: Endpoint<Method = Get>> ReadOnly for E {}
// error[E0271]: type mismatch resolving `<E as Endpoint>::Deletes == ()`
```

Since `ReadOnly` has `Mutates = ()` as a supertrait, the blanket impl has to
require it as well — and then **it enforces nothing about the method.**

The derive generates a compile-time assertion instead.

```rust
const _: () = {
    fn assert_readonly<E: Endpoint<Method = Get> + ReadOnly>() {}
    fn check() { assert_readonly::<GetUser>(); }
};
```

The macro also rejects it at expansion time
([`proc-macro.md`](./proc-macro.md)). **Implement both.**

### `Method` is a type-level marker

```rust
type Method = Get;              // ✅
```

```rust,compile_fail
const METHOD: Method = ...;     // ❌ associated const equality bounds are unstable
```

---

## 5. Limit what reaches the error message

A cons list and `There<There<...>>` appearing in full drives up an AI's iteration
count.

### Every recursive impl carries `#[diagnostic::do_not_recommend]` (1.85+)

Without it, the `help: the following other types implement trait` chain runs to
roughly twenty lines. With it, ten — and **the failing type is shown as the
actual contract tuple rather than the trailing `()`.**

### Every trait with type parameters carries `#[diagnostic::on_unimplemented]`

Do not expose a raw trait-resolution error.

```rust,ignore   // needs a macro that arrives in M2
#[diagnostic::on_unimplemented(
    message = "undeclared mutation `{Domain}::{Field}`",
    label = "not declared in this endpoint's contract",
    note = "add it to #[contract(mutates = [...])] or remove this call"
)]
pub trait CanMutate<Domain, Field> {}
```

> `{Field}` expands to `Email`, not `user::Email` — the path qualification is
> lost. That does not match the form written in the contract (`User::email`), so
> arrange for the message to embed a string derived from `Field::NAME`.

### Put the where clause on the method

On the impl it becomes E0599 (no such method) and **`on_unimplemented` is
discarded.**

```rust,compile_fail
// ❌ where on the impl
impl<'req, E: Endpoint> CtxUsers for Ctx<'req, E> where E: Includes<User> { }
// error[E0599]: the method `users` exists ... but its trait bounds were not satisfied
```

```rust
// ✅ where on the method
trait CtxUsers {
    type R;
    type M;
    // `Owner` is the endpoint type. `Includes` is implemented on the endpoint,
    // and a trait method's where clause cannot name `E`, so the trait exposes it
    // as an associated type. See ../adr/0001 and ../adr/0002.
    type Owner;

    fn users(&self) -> Repo<'_, User, Self::R, Self::M>
    where Self::Owner: Includes<User>;
}
```

**Fix this in the derive's generated template.**

### Shorten types with aliases

The derive generates a type alias per endpoint so the type names in errors stay
short.

```rust
type __VerumUpdateUserMutates = (Mutate<User, user::Name>, ());
```

---

## 6. Use extension traits for framework types

`Ctx`, `Repo` and `Projection` are framework-side types. **An inherent impl can
only be written in the crate that defines the type** (E0116). A derive runs in
the user's crate, so an inherent impl is structurally impossible.

```rust,compile_fail
// ❌ cannot be written in a user's crate
impl<E: Endpoint> Ctx<E> { fn users(&self) -> Repo<User, ...> { } }
// (pre-#39 shape on purpose: this block is the ❌ counter-example for E0116, and
//  the `...` makes no arity claim. Do not "fix" it to `Repo<'_, ..>` — the ✅ half
//  below is what carries the current shape.)
// error[E0116]: cannot define inherent `impl` for a type outside of the crate
```

```rust,ignore   // declaration shown with its body elided
// ✅ generate a local trait
pub trait CtxUsers { type R; type M; fn users(&self) -> Repo<'_, User, Self::R, Self::M>; }
impl<'req, E: Endpoint> CtxUsers for Ctx<'req, E> { }
```

Forgetting the `use` produces an unrelated error, so the derive emits a
`pub use` ([`api-surface.md`](./api-surface.md), the prelude).

---

## 7. Capabilities are ZSTs

They have no runtime representation ([`perf.md`](./perf.md)).

```rust
pub struct Mutate<D, F>(PhantomData<(D, F)>);
pub struct Read<D, F>(PhantomData<(D, F)>);
```

- Not constructible as values — no public constructor.
- No `Copy` — no route to duplicating one.
- Expressed only through type parameters.

---

## 8. Measure compile time

Type-level computation feeds straight into compile time. **Make measuring it a
habit every time an endpoint is added.**

```bash
cargo build --timings
```

| What to watch | Sign of trouble |
|---|---|
| Number of `Has` resolutions | Worse than linear in endpoints × effects |
| Amount of derive output | Generated lines per endpoint |
| Depth of trait resolution | `There<There<There<...>>>` — the depth is the element's position |

If it exceeds 2× Axum, narrow the scope of the type-level computation
([`../specs/evaluation.md`](../specs/evaluation.md), kill criteria).

---

## Never do this

- ❌ Represent an effect set as a flat tuple.
- ❌ Define `Has` without an index parameter.
- ❌ Forget `#[diagnostic::do_not_recommend]` on a recursive impl.
- ❌ Write `Subset`, `Filter` or negative reasoning — a signal to revisit the
  design.
- ❌ Generate `Conditional` without splitting it by category.
- ❌ Put the where clause on the impl — `on_unimplemented` stops working.
- ❌ Try to write an inherent impl on a framework type (E0116).
- ❌ Make a capability constructible as a value.
