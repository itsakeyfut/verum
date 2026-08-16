# Verum — performance rules

> The canon for the design is
> [`../specs/performance.md`](../specs/performance.md).
> Verum is a **framework**. "It is AI-first, so it may be slow" does not follow.
> **Semantic metadata is spent at compile time and left out of the runtime.**

## References

- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Criterion](https://bheisler.github.io/criterion.rs/book/)

---

## Targets

| Metric | Target | How it is measured |
|---|---|---|
| Throughput | **Axum-class** as the floor; investigate Actix-class where possible | Criterion, load testing |
| Extra allocations per request | **No more than Axum** | `dhat`, heaptrack |
| Runtime cost of the contract | **Zero** — static strings only | Read the generated code, plus a benchmark |
| Compile time | **Within 2× of Axum** | `cargo build --timings` |
| Scaling with endpoint count | No worse than linear at 1 / 10 / 50 | Same |

Since Verum calls Hyper directly, **Axum-class is effectively the floor**. Coming
in below it means something is wrong with the design.

---

## A lean runtime — two principles

### 1. Capabilities are ZSTs

```rust
pub struct Mutate<D, F>(PhantomData<(D, F)>);
```

- They are never materialised as values.
- They are expressed only through `Ctx<'req, E>`'s type parameters.
- **No capability may have a runtime representation**
  ([`type-level.md`](./type-level.md)).

### 2. The contract becomes a static string at expansion time

```rust
// ✅ the macro assembles the JSON while expanding and embeds it as a &'static str
const CONTRACT_JSON: &str = r#"{"endpoint":"UpdateUser",...}"#;
```

```rust,ignore   // an alternative described in prose, not a compilable item
// ❌ calling serde_json::to_string at runtime
```

The AI Context output is fixed at compile time. **Do not assemble it at runtime**
([`proc-macro.md`](./proc-macro.md)).

---

## Compile time — Verum's largest single risk

Type-level computation feeds straight into compile time. **Make measuring it a
habit every time an endpoint is added.**

```bash
cargo build --timings
```

| What to watch | Sign of trouble |
|---|---|
| Number of `Has` resolutions | Grows worse than linearly in endpoints × effects |
| Depth of trait resolution | `There<There<There<...>>>` — the depth is the element's position |
| Amount of derive output | Generated lines per endpoint |
| Type-alias expansion | Also affects error-message length |

### Rules that keep it from degrading

- **Do not write `Subset` or `Filter`** — combinatorial explosion.
- **Generate `Conditional` split by category**, which removes the need for
  `Filter` at all.
- **Keep effect categories separate.** A short cons list is walked; a unified one
  walks every effect.
- Shorten long types with type aliases.

### Kill criteria

If compile time exceeds 2× Axum, **narrow the scope of the type-level
computation** ([`../specs/evaluation.md`](../specs/evaluation.md)) — either
restrict what `Has` is applied to, per category, or reduce how much the types
enforce.

---

## The request hot path

### Do not add allocations

```rust
// ✅ a static value stays a &'static str
const PATH: &'static str = "/users/{id}";
```

```rust,ignore   // fragment, not a complete item
// ✅ preallocate when the size is known
let mut params = Vec::with_capacity(param_count);
```

```rust,compile_fail
// ❌ format! on the hot path
let key = format!("{}:{}", method, path);
```

### Be aware of the erasure layer's cost

Making RPITIT dyn-compatible introduces an erasure layer that returns
`Pin<Box<dyn Future + Send>>` ([`rust.md`](./rust.md)).

- **Box once, at endpoint dispatch.** Do not box again inside a handler.
- The middleware chain is under the same constraint, so design it so the number
  of boxes does not grow with the number of chain stages.

### A compile-time route table (later)

With a derive plus `inventory`, every endpoint is known at compile time. That
removes the runtime construction of a radix trie and allows a `match` expression
or perfect hashing instead.

**This optimisation is structurally impossible in Axum**, but **a
straightforward matcher is enough for the First PoC**. It is recorded here as
headroom to use later.

---

## Benchmarks (Criterion)

They live in `crates/verum/benches/`.

| Subject | Contents |
|---|---|
| Route matching | Path resolution, varying the endpoint count |
| Dispatch | Overhead from request to handler call |
| Comparison with Axum | Throughput on the same GET/PUT endpoint |

```bash
cargo bench -- --save-baseline before
cargo bench -- --load-baseline before --save-baseline after
```

- **Do not run them in CI.** Run them by hand, before and after a change that
  touches performance.
- **Always include the comparison with Axum.** The relative number is the
  indicator, not the absolute one.

---

## Profiling

- Use **`tracing` spans** to measure each stage — route match, extract, handle,
  response.
- Allocations: `dhat` or `heaptrack`.
- Compile time: `cargo build --timings`, plus `cargo llvm-lines` for the amount
  of generated code.

---

## Never do this

- ❌ Hold a capability as a runtime value.
- ❌ Assemble the contract JSON at runtime.
- ❌ Call `format!` or allocate a `String` needlessly on the hot path.
- ❌ Box a future anywhere except endpoint dispatch.
- ❌ Write `Subset` or `Filter` at the type level — compile time explodes.
- ❌ Add type-level computation without measuring compile time.
- ❌ Run the benchmarks in CI — unstable and slow.
