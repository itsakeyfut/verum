# T-M1-01 / #13 — Domain opacity × sqlx, extended by #33

```
rustc 1.97.1 · sqlx 0.9.0 · SQLite · 37 probes · 148 packages (4 members + 144 deps)
```

**Read the probe table. Everything else here is one sentence per row.**

That instruction is not modesty. This document has been reviewed twice; **the probe
table was correct both times and the prose was wrong both times** — six claims across
the two rounds, every one of them a generalisation written *past* what a probe
established. So the prose is now cut to what the probes say, and anything beyond that
is an explicitly open question rather than a conclusion. `bash run.sh` is the
authority; if it disagrees with a sentence below, the sentence is wrong.

## Verdict

1. **sqlx interoperability holds.** The `pub(crate)` `Repr` shape compiles and runs.
2. **The trust boundary is not the Repository implementation.** `pub(crate)` means the
   whole app crate, and the derive cannot emit `pub(in …)` because it does not know
   where the Repository will be written. → ledger **path 21**.
3. **The generated shape is undecided.** A derive cannot emit an item named after its
   input, so `pub struct User(UserRepr)` is not something `#[derive(Domain)]` can
   produce.

Everything the alternatives were measured to do is in `docs/specs/persistence.md`
§Verdict. **No alternative measured here improves on the status quo.**

### #33 — how path 21 is narrowed (P22–P30)

4. **Generating the Repository is necessary but not sufficient**, which is the
   premise #33 opened with. Generation changes who *should* call `from_repr`, not
   who *can*: `app/src/repo.rs` already is a Repository, and **P2 forges anyway**.
5. **No trait-bound gate closes it.** A token by value is stolen through the user's
   own `impl Repository` (P23); `impl RepositoryProof for MyProof {}` is a foreign
   trait on a local type and is allowed from the app crate (P24) and any other
   crate (P25).
   **The `E0277` wording itself is reachable, and this README said otherwise.**
   P37 compiles it: the bound yields `E0277` carrying Verum's `message` and `note`
   verbatim. It is rejected for being *unenforceable*, not unavailable — so #33's
   requirements 1 and 2 are **jointly** unsatisfiable rather than 2 being
   individually impossible. The generalisation that survives is about closure:
   `verum` can never own the constructor's **body**, because the domain's fields are
   private to the user's crate, so only *placement* can restrict which code runs it.
6. **What works is placement, and it needs nothing new.** Emit the `Repr`, the
   constructor **and** the Repository into a **derive-owned private module**, and
   re-export the domain from it. Handlers, a helper beside the user's own
   declaration, the crate-root layout, the read half and foreign crates are all
   `E0624` (P31/P32/P34/P35/P27), while the generated Repository still loads a real
   row (P28).
7. **Confining to the *user's* module is not enough**, which is what this spike
   concluded first. A helper written beside the user's `struct User` forges (P29),
   and if the domain is declared at the **crate root** then "no modifier" *is*
   `pub(crate)` and the mechanism buys nothing at all (P33). A derive-owned module
   has neither hole, because its radius is chosen by the derive rather than by the
   code being guarded — RK-016's rule, applied to a type-level mechanism.
8. **The closure is conditional.** It holds only while the conversion is an
   *inherent* method. A trait method's visibility is the **trait's**, so moving the
   conversion onto `fw::DomainRepr` — which P9 and P14 here already do, and which a
   generic `Repo<D: DomainRepr>` would want — reopens everything from every crate
   (**P36**). Path 21 keeps its AI Context entry for that reason.
9. **Everything touching the `Repr` must therefore be generated.** A user-written
   `impl UserRepository for PgUserRepository` outside the module cannot reach
   `from_repr`. That is a real cost and it is #39 / #40's call, not this spike's.

Recorded as [ADR-0010](../../docs/adr/0010-domain-constructor-confined-by-module-privacy.md).

## The probe table

| # | Probe | Expect | Result |
|---|---|---|---|
| P1 | the design as specified: `query_as!` + `FromRow` + `query_as::<_,Repr>` + `pub(crate)` conversions | pass | pass |
| P2 | handler forges a `User` via `from_repr` with arbitrary fields | pass | pass |
| P3 | handler reads every field via `as_repr` | pass | pass |
| P4 | `u.0.email = v` from another module, private inner field | fail | `E0616` |
| P4b | the same from the **defining module and a child module** | pass | pass |
| P13 | the same with a `pub(crate)` inner field | pass | pass |
| P18 | `u.email = v` on the newtype (which has an `email()` getter) | fail | `E0615` |
| P5 | a foreign crate names `UserRepr` | fail | `E0603` (+ `E0624` on `from_repr`) |
| P6 | flat `User` with `as_repr` returning a temporary | fail | `E0515` |
| P19 | flat `User` **owning a cached `Repr`**, same signature | pass | pass |
| P7 | `query_as!` where the Repr's fields are **fully private** | fail | `E0451` |
| P9 | the conversion on a `verum` trait, `Repr` `pub(crate)` | fail | `E0446` |
| P10 | `Repr` public + fields private: foreign crate loads via `FromRow` | pass | pass |
| P11 | …and forges with a struct **literal** | fail | `E0451` |
| P12 | …and tries `query_as!` | fail | `E0451` |
| P15 | …and forges from literal `SELECT` columns instead | pass | pass |
| P14 | `Repr` `pub` in a **private module** + the trait: foreign crate uses projection | pass | pass |
| P20 | `Debug` / `Clone` derived on the `Repr` | pass | pass |
| P16 | a derive emitting `pub struct User(UserRepr)` — same name as its input | fail | `E0428` |
| P17 | control: the same macro emitting `XRepr` **and** `XWrapper` | pass | pass |
| P8 | runtime round trip, both directions | pass | pass |
| **#33 — the candidate gates** | | | |
| P22 | token gate: handler calls the constructor with no token | fail | `E0061` (an **arity** error — no Verum wording) |
| P23 | …the user writes their own `impl TokenRepository` and is handed the token | pass | pass |
| P24 | bound gate: handler supplies its own `impl RepositoryProof` | pass | pass |
| P25 | …the same from a **foreign** crate | pass | pass |
| **#33 — the mechanism** | | | |
| P26 | **`from_repr` with no modifier, called from another module in the crate** | fail | **`E0624`** |
| P27 | …called from a foreign crate | fail | `E0603` (+ `E0624`) |
| P28 | the generated Repository beside the domain loads a real row | pass | pass |
| P29 | the residue of the *user's-module* form: a helper beside the domain forges | pass | **pass** |
| P30 | the `Repr` module-private: a handler cannot **name** it | fail | `E0603` |
| **#33 round 2 — the user's module is the wrong radius** | | | |
| **P31** | **a helper beside the domain, under a derive-owned module** | fail | **`E0624`** |
| P33 | the *user's-module* form with the domain at the **crate root** | pass | **pass — the hole** |
| **P32** | the derive-owned form at the same crate root | fail | **`E0624`** |
| P34 | `as_repr`, the read half ledger path 21 also names | fail | `E0624` |
| P35 | a `Debug` leak through the `Repr`, via a real value | fail | `E0624` |
| P36 | the conversion moved onto the public `fw::DomainRepr` trait | pass | **pass — reopens it** |
| P37 | the bound gate's `E0277`, with Verum's `message` and `note` | fail | `E0277` |

`bash run.sh` → `result: 37 as specified, 0 unexpected`, on 1.97.1 and on nightly 1.99.

**Run it on an otherwise idle checkout.** Review hit a spurious `UNEXPECTED` row
twice, from two independent reviewers, when a second `cargo` (an IDE checker) held
the build lock. The `find … touch` at the top defeats caching *once*, not per
probe, so a concurrent build can still replay a diagnostic. The verdict survived —
`E0624` was reproduced four other ways — but a single red run is not evidence the
verdict moved. An earlier version of this line claimed "reproduced cold and warm";
that was true of the runs I did and not of the runs review did.

P29 is the row that keeps this table honest. Without it P26–P28 read as "closed",
and a probe set that only demonstrates the win is how #15's verdict came to be
refuted three separate ways. P24/P25 do the same job for the `E0277` sub-verdict:
"no trait-bound gate works" is a claim, so the forgery is compiled rather than
argued.

**Mutation-verified**, not merely observed: `pub(crate)` on `from_repr` turns P26
red, on `SecretRepr` turns P30 red, and on the derive-owned constructor turns
**P31** red. Review independently killed all nine of round 1's probes.

**Three of round 1's probes could not fail and were cited as confirmation anyway.**
P27 restates P5's wall (the `Repr`'s visibility); P28 and P29 compile unchanged if
the mechanism is reverted. P27's needle is now `E0624`, which does discriminate.
P29 carries a `const _:` pin, because review measured that emptying its body left it
green — and P29 was the sole evidence for round 1's central qualifier. P2, P23, P24
and P28 had the same defect and now carry pins or a value-bearing needle.

## What each result means — one sentence each

- **P2 / P3** — ordinary handler code in the Domain's crate can build a `User` with
  arbitrary field values and read every field. This is path 21.
- **P4 / P4b / P13** — field privacy is a **module** boundary: it holds outside the
  defining module, not inside it or its children, and not at all if the inner field is
  `pub(crate)`.
- **P18** — the error for `u.email = v` is `E0615` here, `E0609` without a getter of
  that name, `E0616` only for a flat private *named* field from outside its module.
- **P5** — a Repository in its own crate cannot see the `Repr`.
- **P6 / P19** — `as_repr(&self) -> &Repr` requires the Domain to **own** something
  borrowable; a newtype is one way, a cached field is another.
- **P7** — `query_as!` expands into a struct literal at the call site, so **fully
  private** fields fail. `pub(crate)` fields are enough for the in-crate case.
- **P9** — a public trait's associated type cannot be bound to a crate-private type.
  That is all; it does not foreclose anything else.
- **P10 / P11 / P12 / P15** — "`Repr` public, fields private" can load, rejects a
  struct literal, loses `query_as!`, and **is forged anyway** from a row the caller
  supplies.
- **P14** — a `pub` `Repr` in a private module plus the trait lets a foreign crate
  denote it by projection, read every field, and forge.
- **P20** — a `Debug` / `Clone` on the `Repr` lets **handler code in the same crate**
  read every field and take an owned copy (ledger paths 4 and 3). Not reachable from a
  foreign crate in the specified shape: `as_repr` is `pub(crate)`, so that is `E0624`.
- **P16 / P17** — a derive **can** emit a newtype (`XWrapper`); it cannot emit one
  named after its input, because that name is taken.

## Open, not concluded

- **Which macro form.** `as_repr(&self) -> &Repr` is satisfiable by a derive in more
  than one way — a user-written newtype with the derive emitting only the `Repr`, or a
  `Repr` that is a type alias for the Domain — so "the signature and the derive are
  incompatible" is **not** what was measured. What was measured is P16. **#34 decides** (this said #18, which is closed and did not).
- **Who emits `#[derive(sqlx::FromRow)]`.** A user cannot add a derive to a generated
  item. A pass-through attribute is one option; whether Verum emits sqlx paths itself
  is a Dependency-Hiding question.
- **Whether anything closes path 21.** A sealed token was written here as a surviving
  candidate in an earlier version; **that was retracted** — the token can only reach
  user code through a user-implementable trait, so a handler that writes its own
  `impl Repository` receives one. Not probed here; see `docs/specs/persistence.md`.

## Running it

```bash
bash run.sh
```

`setup-db.py` creates `spike.db` first: `query_as!` verifies SQL against a real
database at compile time. Python's stdlib `sqlite3` is used because the CLI is absent
here and requiring `sqlx-cli` to re-check a verdict is friction that stops anyone
re-checking it.

An `UNEXPECTED` row means this toolchain or sqlx version no longer matches the table
— read the diff before changing any spec. `Cargo.lock` is not committed, so a
divergence surfaces as a red row rather than staying pinned; `run.sh` prints the
versions it measured. `[workspace.package]` deliberately carries **no `rust-version``**
— adding one would let the MSRV-aware resolver quietly pick an older sqlx.

## What the harness protects against

Each mechanism was verified by breaking it and confirming the pristine tree stays
green. Rules and rationale: `docs/rules/test.md` §9.

| Mechanism | Broken how | Result |
|---|---|---|
| rejections carry their expected error code | make P4 fail for an unrelated reason | `MISSING("E0616")`, exit 1 |
| acceptances do not depend on the build cache | run twice warm | `Finished` matches both times; `Checking` does not, so it is not used |
| probe code is actually compiled | mistype a cfg in each crate, **including the two integration-test crates and `mac`** | `[lints.rust] unexpected_cfgs = "deny"` per package makes it fatal; an inner `#![deny]` in `lib.rs` did **not** reach test crates |
| the baseline ran | break the schema without touching a source file | `find … -exec touch` forces recompilation; the gate covers `app` **and** `separate-repo` |
| the runtime probe ran | delete `roundtrip.rs` | `MISSING("test result: ok. 2 passed")`, exit 1 |
| the round trip tests both directions | make `save` ignore `as_repr()` | assertion fails |
| a pass-probe still exercises its mechanism | gut P17's macro, delete P19's `as_repr` | `E0412` / `E0425` — the assertions live at the call site, not inside the macro output |

**Known limit**: gutting a pass-probe's function *body* is not always caught. P20's
derives are pinned as a type-level fact, but its handler body is not.

## Why this is not a workspace member, and not in CI

sqlx 0.9.0 declares `rust-version = 1.94.0` and `cargo +1.85.0 check` refuses it
outright, against Verum's MSRV of 1.85. As a member it would break the MSRV job, force
`deny.toml`'s allow-list open (`foldhash` is `Zlib`-only), and add that build to every
PR. The `[workspace]` table keeps it independent.

Not CI-wired, same reason as `.github/scripts/measure-stderr-drift.sh`. **Six probes
need no sqlx and are worth promoting to `compile_fail` / `pass` fixtures when the
derive lands** — P4, P4b, P13, P18, P16, P17; all reduced to 1.85.0 with identical
error codes. Not before, or CI defends a shape **#34** may replace.
