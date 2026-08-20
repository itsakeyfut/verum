---
status: accepted
date: 2026-08-18
decision-makers: itsakeyfut
enforcement-level: upper_bound_checked
# `Includes` gates which domains an endpoint may reach, and the check is an upper
# bound on the declared set — reaching an undeclared domain does not compile. What
# this ADR changes is *where the seal lives*, not the level.
ai-context-key: domains
scope: from any crate, for every domain relation — but only while `Endpoint` itself is not forged
voided-by: path 12 (`Endpoint` is forgeable at M2, irreducibly)
---

# `Includes` is a blanket impl, so its seal never becomes derive-facing

## Context and Problem Statement

`unverified-boundaries.md` records that introducing `#[doc(hidden)] pub mod __private`
at M2 is *"the moment the seal weakens"*. It is worse than that on two counts, both
now measured (`spikes/seal-after-m2/`, `bash run.sh` → `5 as specified`):

1. **The seal stops working.** With `derive_facing` exposed as M2 requires, a
   downstream crate writes the seal impl *and* the trait impl, and two undeclared
   domains pass the Architecture Contract (probe **S2**). This is structural:
   proc-macro output is syntactically indistinguishable from hand-written code, so an
   obligation a derive can discharge downstream, a human can discharge downstream.
2. **The ledger's own re-verification procedure is green on that tree.** It prescribes
   *"confirm `impl Includes<未宣言>` is E0277"* — which is probe **S1**, and S1 passes
   while S2 compiles. The procedure models an attacker who does not write the seal.

This has to be decided now because **#41 blocks M2's seal work**: M2 is when
`SealedEndpoint` / `SealedField` / `SealedCondition` arrive in that module.

## Decision Drivers

* **S2 compiles** — measured from a genuinely separate crate, and the forged bound is
  *used*, not merely declared.
* **S1 is green on the same tree** — the fourth instance of "the closure's
  justification does not cover every impl position" (#6 type parameters, #8 recursion,
  #9 a bound on a parameter not in `Self`), and the first where the defect is in a
  **procedure** rather than in an implementation.
* **Coherence does not help.** Probe **S4**: a blanket impl and a competing specific
  impl are judged **disjoint** exactly when the blanket's obligation is unsatisfiable —
  which is exactly the undeclared domain. `CLAUDE.md` already records this from
  T-M0-08: *coherence permits only the harmful side.*
* **The structural seals are genuinely fine.** `private` stays `pub(crate)`
  permanently, so paths 14a–14e survive M2, and `sealed.rs`'s guard 3 pins the split.

## Considered Options

* **Keep one seal impl per domain**, exposed at M2, and rely on a CI inventory check
* **`Includes` as a blanket impl over the endpoint's declared set**
* Decide nothing here and record the four seals as reopening at M2

## Decision Outcome

Chosen: **`Includes` becomes a blanket impl, and its seal moves to `private`.**

```rust,ignore   // fragment, not a complete item
pub trait Includes<D, I>: private::SealedIncludes<D, I> {}

#[diagnostic::do_not_recommend]
impl<E, D, I> private::SealedIncludes<D, I> for E
where E: Endpoint, E::Domains: Has<D, I> {}

#[diagnostic::do_not_recommend]
impl<E, D, I> Includes<D, I> for E
where E: Endpoint, E::Domains: Has<D, I> {}
```

### The mechanism, and the coherence trap next to it

#41's stated reason is *"nothing is left for the derive to emit, so nothing is left
to forge"*. That is right, and the sentence below is the same causal chain one step
more explicit — **not a correction of #41.** An earlier version of this record
attributed "with coherence doing the rejecting" to #41 and then refuted it; review
checked, and the word *coherence* does not appear in #41 at all. The inference was
this record's own.

What is worth keeping is the **trap**, because it is the reading a future reader will
supply for themselves: coherence does *not* do the rejecting.

> With nothing emitted per domain, **the derive never names the seal** — so
> `SealedIncludes` stops being *derive-facing* and becomes **structural**, and
> structural seals stay `pub(crate)` forever.

Probe **S3** is the consequence: the attacker cannot write the seal impl, because the
module is still private, and the blanket seal does not apply for a domain outside the
declared set. `E0277`. **S5** is its control — a declared domain still resolves.

**S4 measures that trap**, and it is worth a row of its own: someone who generalises
"a blanket impl closes forgery because coherence rejects the competing impl" will be
wrong the next time. Measured in both directions on the *sealed* trait as well as on
an unsealed stand-in — where the blanket's obligation is **satisfiable** the competing
impl is `E0119`, and where it is **unsatisfiable** it is admitted and then fails
`E0277` on the seal. So the seal is still load-bearing under the blanket shape, which
is the second and stronger reading of S4.

### The diagnostics improve, and two attributes are load-bearing

| Shape | Forgery site | Use site |
|---|---|---|
| per-domain | ``cannot implement a sealed Verum trait`` / *this trait is sealed* | ``does not declare the domain `Secrets` `` + a `help` listing the domains that *are* implemented |
| **blanket, both annotated** | ``does not declare the domain `Secrets` `` + what to add | ``does not declare the domain `Secrets` `` |

**Two regressions the blanket shape brings, neither of them fatal and both previously
unrecorded here:**

* rustc appends ``help: the following type implements the trait: E`` — the blanket
  impl's *type parameter*, offered as if it were a concrete type. `do_not_recommend`
  does not remove it.
* the seal's inaccessibility note names the private path
  (``fw::private::SealedIncludes``) twice. The per-domain shape has neither line,
  because its seal lives in a *public* module so rustc adds no such note.
* the index leaks into the outermost bound (``Includes<Secrets, There<_>>``), which is
  the shape `do_not_recommend` exists to keep out of messages — and there it has
  nothing to suppress, because the bound being reported *is* `Includes`.

**The recipe is `on_unimplemented` on *both*, and `do_not_recommend` on both.** An
earlier version of this record said "on the seal, **not** on the public trait", which
is true in the forgery position and **wrong in the position users actually hit** —
review measured all four combinations against both positions:

| annotation | forgery site (`impl Includes<Undeclared>`) | **use site** (a handler reaching an undeclared domain) |
|---|---|---|
| seal only | Verum's wording | ``the trait bound `GetUser: Includes<Secrets, Here>` is not satisfied`` |
| public trait only | ``the trait bound `GetUser: private::SealedIncludes<..>` is not satisfied`` | Verum's wording |
| **both** | Verum's wording | Verum's wording |

The use site is the one that matters most, and `crates/verum/src/domain.rs:17-19`
already says so in as many words: *"an unsatisfied `Includes` bound at a use site,
**which is the shape almost every real error takes**. The seal only fires when
someone writes the impl."*
[`api-surface.md`](../rules/api-surface.md) §2 is a section headed *"`on_unimplemented`
goes on both the seal and the trait"*. **Both were already correct and this record
contradicted them** — an implementer following the earlier text literally would have
deleted `domain.rs`'s annotation and made the everyday error a bare trait bound.
Nothing is traded away by annotating both.

`#[diagnostic::do_not_recommend]` goes on **both** blanket impls, and each is
load-bearing for a different position: without it on the seal's impl the forgery site
drills through to the raw `Has<Secrets, Here>` bound; without it on `Includes`'s impl
the use site does. Measured separately.

> **`seal!` cannot express this yet.** The macro hardcodes `message` / `label` /
> `note` and passes only doc comments through, and
> `seals_should_only_be_declared_through_the_macro` rejects a hand-written seal. So
> implementing this recipe in `crates/verum` requires **teaching `seal!` a per-seal
> message first** — the spike hand-writes its seal, which is why it could measure the
> wording at all. An acceptance condition of #60, and not previously recorded here.

### Confirmation

`spikes/seal-after-m2/` (`bash run.sh`), five rows, S5 as S3's §9-14 control.

**Nothing in `crates/verum` implements this yet, and it cannot until `Endpoint`
exists.** `crates/verum/src/` has no `endpoint.rs`; `Endpoint` and its `Domains`
associated type are **#60**'s. So the implementation and its `compile_fail` fixture —
the attacker writing *both* impls, which is #41's first requirement — are relocated
to **#60**, and this record says so rather than implying a fixture exists.

What *did* land in `crates/verum` with this decision is the guard that protects the
split: `sealed.rs`'s guard 3 no longer hardcodes five seal names. It derives the
structural set as `declared_seals()` minus a one-entry allowlist, so a newly declared
seal defaults to "structural" and every exception is a visible diff. Both attacks it
previously missed now fail — *adding* a new structural seal to `derive_facing`, and a
`pub use super::private::*;` glob that names nothing. Measured before and after.

### Consequences

* Good, because path 13 is closed **for real** rather than provisionally, and stays
  closed through M2: `SealedIncludes` leaves the public surface, so `private` can stay
  `pub(crate)`, and the forgery collapses onto the one declaration site an inventory
  check has to inspect anyway.
* **But "the exposure drops from N domain relations to one `Endpoint` declaration" is
  a line count, not a reduction in attainable privilege.** Measured: one forged
  `Endpoint` with a three-domain `Domains` satisfies all three `Includes` bounds with
  no `Includes` impl written, and `impl<D> Endpoint for Any<D>` covers unboundedly many
  types from a single impl. Once `Endpoint` carries `Reads` / `Mutates` / `Emits` /
  `Calls`, forging it is **strictly greater** than forging N `Includes` relations. The
  change does not *move* the hole — path 12 pre-exists and is untouched — but the
  benefit is the one in the bullet above, not a smaller attack surface.
* Two real boundaries on the residual, both measured and worth more than the "N → 1"
  framing: `impl Endpoint for other_crate::GetUser` is **`E0117`**, so a forgery is
  confined to the crate owning the endpoint type; and a second
  `impl Endpoint for GetUser` widening a set the derive already emitted is
  **`E0119`**. So an attacker cannot widen an existing endpoint — they must introduce a
  new local type and get it routed, which is exactly what an inventory check can see.
* Good, because the error names the domain instead of saying "sealed".
* **`Includes` gains an index parameter** (`Includes<D, I>`), which is a breaking
  change to a public trait's arity. Pre-1.0 and unpublished, so the cost is a version
  bump per `bump.md`.
* **The index parameter imports RK-011's duplicate hazard onto the Architecture
  Contract, and that is new.** `domains = [User, User]` used to be a duplicated
  `impl Includes<User> for GetUser` and therefore `E0119` **at the declaration**;
  under the blanket shape the declaration is fine and **every use site** gets
  `E0283` with the raw `Has` impls in the note, which `do_not_recommend` cannot
  suppress (measured). So macro-side dedup of `domains` becomes load-bearing for
  `Includes`, not only for effect sets — recorded as route 3 in
  [`type-level.md`](../rules/type-level.md) §Duplicates are rejected by the macro,
  and an acceptance condition of the declaration-site assert.
* Bad, because **`Endpoint` remains forgeable under the emission shape assumed
  here** — and that is weaker than "irreducible", which is what an earlier version of
  this record said. If `#[endpoint]` emits an `impl Endpoint for X`, its seal must be
  nameable downstream, so a forged `Endpoint` declares any `Domains` and the blanket
  `Includes` faithfully reports the attacker's set. Path 12 gains a
  `forged_endpoint` AI Context entry (`permanent: true`) for that reason; per ARK-005
  what remains is an inventory check comparing emitted declarations against existing
  impls.

  > **⚠️ "Cannot be closed by types" was an assertion, and review built a
  > counterexample.** Have `#[endpoint]` emit a **type**, not an impl —
  > `pub type GetUser = verum::EndpointOf<GetUserTag, (Order, ())>;` with
  > `impl<Tag, D> Endpoint for EndpointOf<Tag, D>` living inside `verum`. Then
  > `derive_facing` is empty, `SealedEndpoint` stays in `private`, a downstream
  > `impl Endpoint for Evil` is `E0277`, the undeclared-domain error keeps Verum's
  > wording, and `private` is `E0603`. All compiled. The residual "forgery" becomes
  > writing the alias by hand — which **is** the declaration, so the
  > declared-vs-implemented divergence the inventory check exists to catch cannot
  > arise, and ARK-005's remedy dissolves with it.
  >
  > That shape is **not** adopted here: it is unchecked against `Reads` / `Mutates` /
  > `Effects`, routing and the handler bounds. The finding is narrower and sharper —
  > this record promoted an **unADR'd emission convention** (`proc-macro.md` writes
  > `impl Endpoint` without an ADR deciding it) into a measured impossibility in the
  > ledger, and [ADR-0011](./0011-domain-is-an-attribute-macro.md) already established
  > that an attribute can emit shapes a derive cannot. Deciding the emission shape is
  > **#60's**, and it is now an open question rather than a closed one.
* `Field` and `Condition` are **not** fixed by this. `Field::NAME` is per-field data,
  so it must be emitted; `Condition::holds` is user-written by design. Paths 14 and
  the `Condition` route keep their true post-M2 status, recorded in the ledger.

## More Information

* #41 — the issue; #65 — the guard's hardcoded list, fixed here and closed
* `spikes/seal-after-m2/README.md` — probes S1–S5 and the refutation of S4
* `docs/rules/api-surface.md` §2 — the seal-exactness rule, which now also applies to
  verification procedures
* `docs/specs/unverified-boundaries.md` paths 12, 13, 14, 14a–14e · RK-009, RK-015,
  RK-016, ARK-005
