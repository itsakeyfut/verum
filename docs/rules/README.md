# Verum — implementation rules (`docs/rules`)

> This directory holds Verum's implementation rules: **what to follow while
> writing code**, decided ahead of time.
> The **canon of the design itself** is [`../specs/`](../specs/README.md); the
> **reasoning behind each decision** is [`../adr/`](../adr/README.md). These
> documents answer "how do we hold to it in code".

## Index

| File | Contents | Weight |
|---|---|---|
| [api-surface.md](./api-surface.md) | **The boundary of the public API** (Dependency Hiding Rule, sealed traits, prelude, MSRV) | **Highest** |
| [type-level.md](./type-level.md) | **Type-level programming** (cons lists, index parameters, forbidden operations) | **Highest** |
| [proc-macro.md](./proc-macro.md) | **Derive and attribute macros** (three defence layers, span preservation, rules for generated code) | **Highest** |
| [design.md](./design.md) | Crate layout, module split, dependency direction, naming | High |
| [test.md](./test.md) | Testing rules (**UI tests are the core**) | High |
| [rust.md](./rust.md) | Rust coding standards | Medium |
| [error-handling.md](./error-handling.md) | Runtime error handling (compile errors are in `../specs/diagnostics.md`) | Medium |
| [perf.md](./perf.md) | Performance (Axum-class throughput, compile time, lean runtime) | Medium |
| [logging.md](./logging.md) | Logging (`tracing`, and what a framework owes its users) | Medium |
| [unsafe.md](./unsafe.md) | `unsafe` rules (forbidden by default) | Low — no site is expected to need it |

---

## Assumptions (project-wide)

- **Rust / Cargo workspace**, edition **2024**, MSRV **1.85+**
  ([api-surface.md](./api-surface.md)).
- **Two crates**: `verum` (the framework) and `verum-macros` (the proc macros).
  No backend crate is split out ahead of a second backend
  ([design.md](./design.md)).
- **Router, extractor, handler and response are written here**, on top of Tokio,
  Hyper and Tower/tower-http. Axum is used only during the PoC
  ([`../specs/runtime-stack.md`](../specs/runtime-stack.md)).
- **No type from a replaceable dependency appears in the public API.** That is
  what buys the freedom to drop Axum later ([api-surface.md](./api-surface.md)).
- **The UI tests (`trybuild`) are the specification.** Error messages are a
  first-class deliverable ([test.md](./test.md),
  [`../specs/diagnostics.md`](../specs/diagnostics.md)).

---

## Where Verum's weight sits

Compared with an ordinary Rust project, the rules here lean somewhere different.

```text
Ordinary application work        Verum
─────────────────────────────────────────────────────
Business logic              →    Type-level programming
Runtime performance         →    Compile time *and* runtime performance
Error handling              →    Designing compile errors
Unit tests                  →    UI tests (`compile_fail`)
Making the API easy to use  →    Deciding what the API does not expose
```

**The value comes from wrong code failing to compile**, so the rules concentrate
there.

---

## Read these before implementing

```text
api-surface.md   — what must never reach the public surface (fixing it later is breaking)
type-level.md    — compiler constraints already verified (without it you will hit E0119)
proc-macro.md    — which layer rejects which error
```

And on the specs side:

```text
../adr/README.md                   — every design decision, and what is still open
../specs/unverified-boundaries.md  — the ledger of every path the type system does not reach
../specs/rust-type-model.md        — the canon of the type design
../specs/diagnostics.md            — the target shape of error messages
```

---

## How these rules are maintained

- **The rules are the default authority.** Implementations follow them.
- When an implementation and a rule collide, decide case by case which one
  gives:
  - **Rule wins** (usually): change the implementation to match.
  - **Implementation wins** (when the rule does not match reality, or gets in the
    way): proceed, and **update the rule document in the same change**. Rules and
    code do not drift apart.
- Rule changes may also be requested directly. Either way, the change lands in
  this directory.
- For collisions that are unclear or wide-reaching, ask rather than decide
  unilaterally.

### Rules specific to Verum

**Never leave documentation and implementation diverged.** Verum exists to reject
the state where a contract and its implementation disagree. **We do not get to do
that to our own documentation.**

- When a spec changes, update the affected parts of `specs/` and `rules/` in the
  same commit.
- Record constraints discovered while implementing, together with the evidence —
  the error code, the measured number.
- **Mark unverified claims as unverified.** That is
  [`../specs/unverified-boundaries.md`](../specs/unverified-boundaries.md)'s
  discipline applied to ourselves.
- **A design decision's rationale belongs in [`../adr/`](../adr/README.md) and
  nowhere else.** These documents state the outcome and link there. After
  correcting any documented claim, grep the whole repository for the **old**
  wording — a correction that reaches the canon and not the surrounding
  instructions is the failure mode this rule exists to stop.

**Write down the points where you hesitated while implementing.** The Q-C
experiment demonstrated that these work as an indicator of holes in the spec
— three were found that way
([`../specs/evaluation.md`](../specs/evaluation.md)). Your own hesitation while
implementing is the same indicator.

---

## Undecided areas

The following are not settled, so the rules covering them are provisional.
**They are decided while implementing**
([`../specs/research-questions.md`](../specs/research-questions.md)).

| Area | State |
|---|---|
| The handler's error type | The `E` in `Result<T, E>`. The Error Contract is undecided ([error-handling.md](./error-handling.md)) |
| Request extraction | How path parameters map to the body; the shape of `#[derive(Request)]` |
| Response / view generation | The shape of `#[derive(View)]` |
| Repository injection | How it reaches `Ctx` |
| The test API | The shape of `verum::test::run` ([test.md](./test.md)) |

When they are settled, the decision goes to [`../adr/`](../adr/README.md) and
this directory is updated to match.
