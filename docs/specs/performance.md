# Performance

Runtime performance targets, and the policy that keeps semantic metadata out of
the runtime.

Related: [`runtime-stack.md`](./runtime-stack.md),
[`evaluation.md`](./evaluation.md). The implementation rules are in
[`../rules/perf.md`](../rules/perf.md).

---

## Principle

Being AI-first is not a reason to give up runtime performance.

---

## Goal

- **Axum-class performance is the target.**
- Investigate Actix Web-class where possible.
- But beating Actix Web is not a hard requirement from the outset.

Since Hyper is called directly, Axum-class is expected to be the effective floor.

---

## Spending it at compile time

Semantic metadata is consumed at compile time as far as possible.

```text
Semantic contract
        ↓
Compile time
        ↓
Validation
        ↓
Optimisation
        ↓
Lean runtime
```

Ideally the runtime carries no significant overhead from metadata that exists for
an AI's benefit.

---

## Room for optimisation

### A compile-time route table

With a derive macro plus `inventory`, every endpoint is known at compile time.
That removes the runtime construction of a radix trie and allows a `match`
expression or perfect hashing instead.

This optimisation is structurally impossible in Axum's design. A straightforward
matcher is enough to begin with.

### Capability tokens cost nothing

Capabilities are ZSTs (zero-sized types) wherever possible, with no runtime
representation. They exist only for type checking.

---

## Costs to watch

| Item | Risk |
|---|---|
| Type-level set operations | Trait resolution explodes → compile time degrades |
| Effect tuples growing | Same |
| Volume of derive output | Compile time |
| Metadata retained at runtime | Memory, execution speed |

Compile time feeds directly into developer experience, so it is measured as a
performance metric.

---

## Open questions

- How much impact on performance and compile time is acceptable?
- Reconciling developer experience with AI experience.

See [`research-questions.md`](./research-questions.md).
