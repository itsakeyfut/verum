# Verum — error handling

> Related: [rust.md](./rust.md) (the summary) and
> [`../specs/diagnostics.md`](../specs/diagnostics.md) (the design of **compile**
> errors).
> This document is about **runtime** errors. Compile-error design is a separate
> subject; do not conflate the two.

---

## Policy

- Error types are built with **`thiserror`**. Verum is a library, so **`anyhow`
  is not used**.
- **Recoverable failures return `Result`; broken invariants panic.** With the
  qualification below: nothing on the request path panics.
- **When a framework panics, the user's server goes down.**

---

## `Result` or panic

| Class | Examples | Handling |
|---|---|---|
| **`Result`** | Body size exceeded, timeout, path parse failure, a handler's business error | Return a typed error |
| **Panic at startup (allowed)** | Duplicate route, unregistered repository, malformed bind address | **Failing at startup is the safer outcome.** Detect before serving |
| **Panic at runtime (forbidden)** | Anywhere during request handling | Catch it, return 500, record with `tracing::error!` |

### The catch layer for runtime panics lives in the runtime

```text
// runtime/ wraps the handler call in catch_unwind.
// One request's panic must not take the process down.
```

- On catching, record with `tracing::error!` and return 500.
- **Never put the panic's content in the response body** — that leaks internals.
- The same applies when a user's handler panics.

---

## `unwrap()` and `expect()`

**Forbidden** in production code as a rule; allowed only inside `#[cfg(test)]`.

The one exception is a **genuine invariant**, and it carries a `// INVARIANT:`
comment giving the grounds.

```rust,ignore   // fragment, not a complete item
// ✅ INVARIANT: the route table is validated at startup — duplicates and
// malformed paths already panicked there.
let route = self.table.get(idx).expect("index from validated route table");
```

```rust,compile_fail
// ❌ unwrapping a value that came from the request
let id = path.parse::<Uuid>().unwrap();
```

---

## Designing the error type

### Give it variants that mean something

```rust,ignore   // needs a crate or a verum-private module this harness does not carry
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum VerumError {
    #[error("request body exceeded the configured limit")]
    BodyTooLarge,

    #[error("request timed out")]
    Timeout,

    #[error("invalid path segment")]
    InvalidPath,

    #[error("no route matched")]
    NotFound,

    #[error(transparent)]
    Handler(Box<dyn std::error::Error + Send + Sync>),
}
```

- **`#[non_exhaustive]`** keeps adding a variant non-breaking
  ([`api-surface.md`](./api-surface.md)).
- Never return an error that is only a string (`"something went wrong"`).

### Wrap lower-level errors with `#[from]`

```rust,ignore   // fragment, not a complete item
#[error(transparent)]
Http(#[from] http::Error),
```

---

## The handler's error type (undecided)

What the `E` in `Handler::handle`'s `Result<T, E>` should be is **not decided**.

- Should the contract declare `fails = [NotFound, Conflict]`?
- Should errors be treated as a kind of effect?
- Where does the mapping to HTTP status live?

This is an open item in
[`../specs/research-questions.md`](../specs/research-questions.md). **The First
PoC proceeds with the simple form (`Result<T, VerumError>`) and changes it when
the Error Contract is designed.**

The deferral is deliberate, in full knowledge that the change will be breaking.

---

## Converting to a response

The layer that turns an error into an HTTP response lives in `runtime/`.

- **Leak nothing internal.** No panic message, no SQL, no stack trace in the
  body.
- Record the detail through `tracing`; return the minimum in the response.
- Keep the status-code mapping in one place.

```rust,ignore   // fragment, not a complete item
// ✅ detail to the log, minimum to the response
tracing::error!(error = ?e, endpoint = %E::NAME, "handler failed");
StatusCode::INTERNAL_SERVER_ERROR
```

---

## What is exported

- `verum` re-exports from the crate root: `pub use error::VerumError;`
- A user's handler must be able to return its own error type. Whether that is
  through `Box<dyn Error>` or an associated type is part of the undecided item
  above.

---

## Never do this

- ❌ Panic on the request path (startup is fine).
- ❌ Use `unwrap()` / `expect()` in production code — except for a genuine
  invariant carrying `// INVARIANT:`.
- ❌ Swallow an error silently. If it is ignored deliberately, emit
  `tracing::warn!`.
- ❌ Return an error that is only a string.
- ❌ Put a panic message or any internal detail in the response body.
- ❌ Use `anyhow` in a library.
