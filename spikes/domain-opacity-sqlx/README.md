# T-M1-01 / #13 — Domain opacity × sqlx

```
rustc 1.97.1 · sqlx 0.9.0 · SQLite · 21 probes · 148 packages (4 members + 144 deps)
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
§判定. **No alternative measured here improves on the status quo**, and the closing
mechanism is #18's decision, not this spike's.

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

`bash run.sh` → `result: 21 as specified, 0 unexpected`. Reproduced cold and warm, on
1.97.1 and on nightly 1.99.

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
  incompatible" is **not** what was measured. What was measured is P16. #18 decides.
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
error codes. Not before, or CI defends a shape #18 may replace.
