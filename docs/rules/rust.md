# Verum — Rust coding standards

> Applies to `verum` and `verum-macros`. The canon of the design is
> [`../specs/`](../specs/README.md); the reasoning behind each decision is
> [`../adr/`](../adr/README.md).
> The public API is [`api-surface.md`](./api-surface.md), the type level is
> [`type-level.md`](./type-level.md), macros are
> [`proc-macro.md`](./proc-macro.md).

## References

- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [tokio](https://docs.rs/tokio) / [hyper](https://docs.rs/hyper) / [tower](https://docs.rs/tower)

---

## Language versions

| Item | Value |
|---|---|
| edition | **2024** |
| MSRV | **1.85+** |

Required for `#[diagnostic::do_not_recommend]`, which stabilised in 1.85, and for
edition 2024. Async closures (`AsyncFnOnce`), used by the `when` scope, landed in
the same release. CI verifies the MSRV build.

---

## Error handling

> The detail is in [error-handling.md](./error-handling.md); this is the summary.

Verum is a **library crate**. It returns typed errors and does not use `anyhow`.

```rust,ignore   // needs a crate or a verum-private module this harness does not carry
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum VerumError {
    #[error("request body exceeded the configured limit")]
    BodyTooLarge,
    #[error("request timed out")]
    Timeout,
}
```

### `unwrap()` and `expect()`

**Forbidden** in production code as a rule; allowed only inside `#[cfg(test)]`.

The exception is a **genuine invariant** that holds by construction. `expect` is
permitted there, with a `// INVARIANT:` comment giving the grounds.

```rust,ignore   // fragment, not a complete item
// ✅ genuine invariants only, and the comment is required
// INVARIANT: the route table the derive generates is validated at startup
let route = table.get(idx).expect("route index from validated table");
```

### Panics

**When a framework panics, the user's server goes down.**

- Do not panic while running a handler, handling a request, or routing.
- Put a layer in the runtime that catches panics, returns 500, and records with
  `tracing::error!`.
- One request's failure must not take the process down.

The exception is **misconfiguration at startup** — a duplicate route, an
unregistered repository. Panicking there is fine; failing before serving is
safer.

---

## Concurrency and the runtime

### tokio is assumed

Verum runs on tokio. It does not aim to be a runtime-agnostic library
([`../specs/runtime-stack.md`](../specs/runtime-stack.md)).

### Preserve the `Send` bound

To load onto hyper's multi-thread runtime, **the handler's future must be
`Send`.**

```rust
// ✅ RPITIT, with Send stated
pub trait Handler: Endpoint {
    fn handle(&self, req: Self::Request, ctx: Ctx<'_, Self>)
        -> impl Future<Output = Result<Self::Response>> + Send;
}

// ❌ leaving it as AFIT gives a future that is not Send
//    error: future cannot be sent between threads safely
```

`Ctx<'req, E>` keeps `Send` while **not** being `'static`
([`api-surface.md`](./api-surface.md)).

### dyn compatibility is bought back with an erasure layer

RPITIT makes the trait dyn-incompatible (`E0038`), so the router cannot hold a
`Box<dyn Handler>`. The derive generates an object-safe erasure layer.

```rust,ignore   // fragment, not a complete item
fn call(&self, req: Request<Body>) -> Pin<Box<dyn Future<Output = Response> + Send + '_>>;
```

### Do not bring blocking work into async

CPU-bound work goes to `tokio::task::spawn_blocking`. **Verum itself has
essentially none** — contract checking is over by the time the program runs.

---

## Type design

### A named struct, not a tuple

```rust,compile_fail
// ❌ opaque
fn parts(&self) -> (Method, &str) { }
```

```rust
// ✅
pub struct RouteKey { pub method: Method, pub path: &'static str }
```

**The exception**: type-level cons lists use tuples
([`type-level.md`](./type-level.md)). Those are types, not values, so this rule
does not reach them.

### Domain values are newtypes

```rust
pub struct EndpointId(&'static str);
pub struct RouteIndex(usize);
```

### Public enums and option structs are `#[non_exhaustive]`

So adding a variant or a field stays non-breaking
([`api-surface.md`](./api-surface.md)).

### Capabilities are ZSTs

```rust
pub struct Mutate<D, F>(PhantomData<(D, F)>);
```

No runtime representation, no way to construct one as a value, and no `Copy`
([`type-level.md`](./type-level.md)).

### Use a builder for complex construction

Three or more optional items means a builder. Required items go in `new()`.

---

## Code quality

### Iterators over manual loops

```rust,ignore   // fragment, not a complete item
let paths: Vec<_> = contracts.iter().map(|c| c.path).collect();
```

### Annotate a non-obvious clone

```rust,ignore   // fragment, not a complete item
// clone required: the router demands 'static
let handler = handler.clone();
```

### Write no `unsafe`

Verum sits on safe abstractions — tokio, hyper, tower. The workspace sets
`unsafe_code = "forbid"` ([unsafe.md](./unsafe.md)).

### Leave no dead code

Delete unused `use` statements, functions and variables. If
`#[allow(dead_code)]` is genuinely needed, comment why.

**The exception**: a type-level part that is "not used yet but structurally
required" stays only once a UI test exercises it.

### Lint configuration

Lints live in the root `Cargo.toml`, and **each crate opts in with
`[lints] workspace = true`** — the declaration alone does nothing.

```toml
[workspace.lints.rust]
unsafe_code = "forbid"             # forbid, not deny (see unsafe.md)
unsafe_op_in_unsafe_fn = "warn"
missing_docs = "warn"              # docs are mandatory on the public API
unreachable_pub = "warn"           # catches unintended `pub`

[workspace.lints.clippy]
all = { level = "warn", priority = -1 }   # -1 lets individual lints override the group
undocumented_unsafe_blocks = "warn"
```

That is the **whole** lint table. The grounds for `unsafe_code`,
`unsafe_op_in_unsafe_fn` and `undocumented_unsafe_blocks` are in
[unsafe.md](./unsafe.md).

`missing_docs` makes documentation mandatory on the public API. **Verum's public
API is what an AI reads**, so documentation is a first-class deliverable
([design.md](./design.md)).

---

## Comments

Verum is a framework for building codebases that do not depend on comments. **Its
own code should be one.**

- Comments explain **why**. Do not write the obvious *what*.
- **Non-obvious type-level intent** — why an index parameter is needed, and
  similar — may use a multi-line block comment.
- When a constraint was confirmed by compiling, name the error code that proves
  it.

```rust,ignore   // verum-internal: legal only inside the crate that owns the trait or type
// ✅ why, with the evidence
// coherence does not consult where clauses here, so without the index this is E0119
impl<H, X, T: ConsList, I: Index> Has<H, There<I>> for (X, T) where T: Has<H, I> {}
```

```rust,compile_fail
// ❌ the obvious what
// look up a User
fn find(&self, id: UserId) -> Result<User> { }
```

### Doc comments

The public API carries documentation (`missing_docs = "warn"`). But:

- **Do not restate the spec.** Link to `specs/` instead.
- **Only make claims a doc test can check.** If it is not verified, do not write
  it.

```text
/// Declares the endpoint's semantic contract.
///
/// Fields not listed in `mutates` cannot be written — the call does not compile.
/// See [`docs/specs/mutation-contract.md`] for the full model.
```
