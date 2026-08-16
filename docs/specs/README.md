# Verum — specs

The technical specifications: **what the design is.**

The reasoning behind each decision lives in [`../adr/`](../adr/README.md); the
vision is in [`../concepts.md`](../concepts.md); the implementation rules are in
[`../rules/`](../rules/README.md).

**These documents state outcomes and link to the ADR.** They do not repeat the
rationale — that separation is what stops a claim being corrected in one place
and left standing in five others.

---

## Read before implementing

| File | Contents |
|---|---|
| [`unverified-boundaries.md`](./unverified-boundaries.md) | **The ledger of every path the type system does not reach.** The file that exists so nothing is left unrecorded |
| [`rust-type-model.md`](./rust-type-model.md) | Constraints confirmed by compiling: cons lists, index parameters, extension traits, MSRV |
| [`diagnostics.md`](./diagnostics.md) | Error messages are a specification. The three defence layers, and what cannot be reached |
| [`research-questions.md`](./research-questions.md) | What is still undecided, including the highest-priority items |

---

## Core model

| File | Contents |
|---|---|
| [`semantic-endpoint.md`](./semantic-endpoint.md) | The contract declaration syntax (attribute → type expansion) and what an endpoint is made of |
| [`handler-rules.md`](./handler-rules.md) | The four rules that keep implementations self-evident. A precondition for the capability design |
| [`effect-system.md`](./effect-system.md) | Effect classification, category splits, declaration granularity, the read-only guarantee on GET |
| [`mutation-contract.md`](./mutation-contract.md) | Field-level mutability. **Domain opacity** |
| [`read-contract.md`](./read-contract.md) | Enforcing `reads` through a projection type |
| [`conditional-effects.md`](./conditional-effects.md) | Issuing capabilities through a `when` scope, and the limits that cannot be removed |
| [`capability-system.md`](./capability-system.md) | The core mechanism: `Ctx<'req, E>`, sealed traits, and how this differs from authorisation |
| [`architecture-contract.md`](./architecture-contract.md) | Constraining the handler → service → repository path with types |

## Verification

| File | Contents |
|---|---|
| [`effect-inference.md`](./effect-inference.md) | The limits of an upper-bound check, and the "generate it from the implementation" alternative |
| [`diagnostics.md`](./diagnostics.md) | Error message design |
| [`rust-type-model.md`](./rust-type-model.md) | Which of Rust's type features are used |

## Runtime

| File | Contents |
|---|---|
| [`runtime-stack.md`](./runtime-stack.md) | Which layers are depended on and which are written here. The Dependency Hiding Rule. MSRV |
| [`middleware.md`](./middleware.md) | Typing middleware, and the boundary with tower / tower-http |
| [`persistence.md`](./persistence.md) | The repository trait's scope and its trust boundary. Interoperating with domain opacity |
| [`performance.md`](./performance.md) | Performance targets, and the policy of spending metadata at compile time |

## Output and evaluation

| File | Contents |
|---|---|
| [`ai-context.md`](./ai-context.md) | The semantic code graph. Stating enforcement levels and unverified boundaries |
| [`evaluation.md`](./evaluation.md) | The metrics for the AI coding benchmark |

## Open problems

| File | Contents |
|---|---|
| [`unverified-boundaries.md`](./unverified-boundaries.md) | The ledger of unchecked paths |
| [`research-questions.md`](./research-questions.md) | Unresolved design questions |
| [`../adr/README.md`](../adr/README.md) | Decisions still marked `proposed` — the ones the codebase already relies on |

---

## Reading order

Coming to this for the first time:

```text
../concepts.md            — what is being built
        ↓
semantic-endpoint.md      — how a contract is declared
        ↓
handler-rules.md          — how an implementation is written
        ↓
capability-system.md      — how types enforce it
        ↓
unverified-boundaries.md  — where they do not
        ↓
../adr/README.md          — why each choice was made, and what is still open
```

**Do not skip `unverified-boundaries.md`.** The more complete a contract looks,
the stronger the false assurance that it covers everything — so using Verum
without knowing what is *not* guaranteed is the largest risk it carries.
