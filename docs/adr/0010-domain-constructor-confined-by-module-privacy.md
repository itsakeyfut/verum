---
status: accepted
date: 2026-08-17
decision-makers: itsakeyfut
consulted: "#13 (T-M1-01), #33"
informed: "docs/specs/persistence.md, docs/specs/unverified-boundaries.md, docs/specs/diagnostics.md"
---

# The Domain constructor is confined to a derive-owned module, not to the user's

## Context and Problem Statement

`#[derive(Domain)]` emits a `Repr` and a constructor so a repository can rebuild a
domain value from a database row. The derive expands **inside the user's crate**,
so `pub(crate) fn from_repr` is reachable from every handler in the application:
any endpoint can invent a domain value with no capability, no repository, no SQL
and no `unsafe`. That is ledger **path 21**, measured in #13 and open ever since.

#33 asked how to close it. Its own framing was that *"the only way to make 'the
Repository implementation' a place the type system can name is for Verum to
generate it."*

## Decision Drivers

* The forged value must fail to compile, not merely be discouraged.
* Blocking a route without providing a checked alternative pushes people onto
  unchecked ones (ARK-002).
* **The confinement must not depend on where the user put the domain.** This is
  RK-016's rule for guards, and it turns out to apply to a type-level mechanism
  just as sharply — see option D.
* Whatever is claimed has to be compile-verified.

## Considered Options

* **A. Generate the repository, keep `pub(crate) fn from_repr`**
* **B. Gate the constructor on a capability token value (`RepoToken`)**
* **C. Gate the constructor on a trait bound (`P: RepositoryProof`)**
* **D. Emit the constructor with no visibility modifier into the *domain's own*
  module, and emit the repository beside it**
* **E. Emit the constructor, the `Repr` and the repository into a **derive-owned
  private module**, and re-export the domain type from it**

## Decision Outcome

Chosen option: **E**, because it is the only option measured to reject every
forgery route without depending on the user's module layout.

```rust,ignore   // what the derive emits
mod __verum_account {
    pub struct Account(AccountRepr);
    struct AccountRepr { .. }              // module-private: paths 3/4 shut with it
    impl Account { fn from_repr(r: AccountRepr) -> Self { .. } }   // no modifier
    pub struct AccountRepository;          // the only legitimate caller, inside
}
pub use __verum_account::{Account, AccountRepository};
```

**Option D was chosen first and is dominated by E.** D confines the constructor to
whichever module the user declared the domain in. Two holes were found in review,
both compile-verified:

| Hole | D | E |
|---|---|---|
| A helper the user writes **beside their own `struct`** (P29 / P31) | **compiles** | **`E0624`** |
| The domain declared at the **crate root** — "no modifier" *is* `pub(crate)`, so the module is the crate (P33 / P32) | **compiles** | **`E0624`** |

D's radius is chosen by the code being guarded. E's is chosen by the derive. That
is the same property RK-016 demands of a guard, reappearing as a type-level
mechanism rather than a scanning guard.

### The constraint the decision depends on

**The conversion must be an inherent method. It may never sit on a public trait.**
An inherent method's visibility is its impl block's module; a **trait method's
visibility is the trait's**. Moving the conversion onto `fw::DomainRepr` — which
this spike's own P9 and P14 do, and which any generic runtime (`Repo<D: DomainRepr>`)
would want — makes every wall above evaporate, from the application crate and from
foreign crates alike (P36, `Finished`). Any future proposal for a generic
persistence layer collides with this decision here.

### Consequences

* Good, because handler and service code is rejected (`E0624`, P26/P31), a foreign
  crate is rejected (P27), the read half `as_repr` is rejected too (P34), and the
  generated repository still loads a real row (P28).
* Good, because the module-private `Repr` closes ledger paths 3 and 4 through it:
  no accessor hands a `Repr` out of the module, so there is no value to `Debug`
  (P35). *Note the mechanism: it is the absence of an accessor, not the
  unreachability of the name — `Debug` needs a value, not a name. An earlier draft
  of this ADR stated the wrong mechanism.*
* Good, because the confinement no longer depends on the user's layout.
* **Bad, because all persistence must be generated.** A user-written
  `impl UserRepository for PgUserRepository` outside the generated module cannot
  reach `from_repr`. `docs/specs/persistence.md` still shows exactly that shape.
  **Whether that trade is acceptable is #39 / #40's decision, not this one** — this
  ADR establishes that the trade exists and that D is dominated on containment.
* Bad, because the rejection is a visibility error and cannot be reworded through
  `#[diagnostic::…]` — recorded as a new row in `docs/specs/diagnostics.md`.
* **Bad, and ARK-002 is not satisfied.** `rustc --explain E0624` names both
  bypasses: *"1. Only use the item in the scope it has been defined"* (write the
  code inside the module) and *"2. Make the item public"*. Under E the first is
  unavailable to the user — the module is derive-owned — but the error still emits
  no pointer to `AccountRepository`, and its only navigational span points into the
  generated module. **This is an open risk, not a solved one**; see
  `docs/specs/diagnostics.md`.

### Confirmation

`spikes/domain-opacity-sqlx/run.sh`. **Only probes that fail when the decision is
violated count as confirmation** (CLAUDE.md's definition):

| Probe | Fails if… |
|---|---|
| **P31** (`E0624`) | the constructor leaves the derive-owned module — mutation-verified: `pub(crate)` turns it red |
| **P32** (`E0624`) | the crate-root layout is not covered |
| **P34** (`E0624`) | `as_repr` is not confined with `from_repr` |
| **P35** (`E0624`) | the `Repr` escapes, reopening paths 3/4 |
| **P26**, **P30** | option D's weaker form still holds where it holds |

**P27, P28 and P29 are *not* confirmation of this decision.** P27 restates P5's
wall (the `Repr`'s visibility), and P28/P29 compile unchanged if the decision is
reverted. An earlier draft listed all five as Confirmation; three of them could not
fail. P33 and P36 are the counter-evidence rows — they must **pass**.

## Pros and Cons of the Options

### A. Generate the repository, keep `pub(crate) fn from_repr`

* Good, because generation is needed regardless — it is what puts a legitimate
  caller inside the confinement D and E rely on.
* Bad, because it is **not sufficient on its own**, which is where #33's premise
  was wrong. Generating the repository changes who *should* call the constructor;
  it does not change who *can*.
* Evidence, stated exactly: probe **P2** shows a handler forging while a repository
  (`app/src/repo.rs`) exists in the crate. That repository is **hand-written**, so
  P2 measures "a repository existing in the crate does not restrict a `pub(crate)`
  constructor". Getting from there to "generating it is not sufficient" needs the
  further premise that generation leaves visibility unchanged — true, but reasoning,
  not measurement.

### B. Gate the constructor on a capability token value

* Bad, because the rejection is `E0061` — an arity error carrying no wording Verum
  wrote (P22).
* Bad, and decisively: the token is **stealable** (P23). `fw` can only hand a token
  to an implementor of its repository trait, and the user can write that impl.

### C. Gate the constructor on a trait bound

* Good, because it **does** produce `E0277` carrying Verum's
  `#[diagnostic::on_unimplemented]` message and note — **measured, P37**:
  ```
  error[E0277]: `NoProof` cannot authorise constructing a Domain value
    = note: Only the Repository generated by `#[derive(Domain)]` may build this
      Domain from its Repr. Load it through that repository ...
  ```
* Bad, because it is **unenforceable**: the trait is forgeable from the application
  crate (P24) and from any other crate (P25) — `impl verum::RepositoryProof for
  MyProof {}` is a foreign trait on a local type, which the orphan rules permit.
* Bad, because rustc's own `help:` line on that `E0277` points at the user's type
  and says the trait is not implemented for it — coaching the two-line bypass.

**Correction.** An earlier draft of this ADR said requirement 2 of #33 (`E0277`
with Verum-authored wording) was *"not merely unmet but unreachable"*. That is
false, and was refuted by compiling it. The wording is **reachable and worthless**:
requirement 1 (a handler and a foreign crate both fail to forge) and requirement 2
are **jointly** unsatisfiable, which is a narrower and different claim.

The generalisation that survives is about closure, not about diagnostics: **`fw`
can never own the constructor's body**, because the domain's fields are private to
the user's crate. Construction code therefore always lives in the user's crate, and
only *placement* can restrict which code there runs it — a trait bound can merely
gate entry to code that already sits where privacy has already granted access.
This is consistent with `docs/rules/api-surface.md` §2 and ledger path 13, which
record derive-facing seals as **weakened with a tracked re-verification condition**,
not as futile.

## More Information

* `docs/specs/persistence.md` §How path 21 is closed
* `docs/specs/unverified-boundaries.md` path 21
* `docs/specs/diagnostics.md` §Rustc-native diagnostics Verum cannot reword
* `docs/rules/api-surface.md` §2 and ledger path 13 — the derive-cannot-reach-`private`
  result this decision rests on, with the opposite (correct) disposition
* [ADR-0009](./0009-observed-is-not-a-lower-bound.md) separately notes that path 21
  is not closed by generating the **Contract** from token scanning, and that
  `from_repr` at least becomes *visible* there. That is a different mechanism from
  repository generation; an earlier draft cited it as "two independent
  measurements, one result", which over-read it.
* `spikes/domain-opacity-sqlx/README.md`
