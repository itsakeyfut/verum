# Verum — logging

> Use **`tracing`**. Related: [rust.md](./rust.md), [perf.md](./perf.md) (lean
> runtime).

---

## Policy

- **Use the `tracing` crate only.** Not `log`.
- **A library does not initialise a subscriber.** Calling
  `tracing-subscriber`'s init is the application's job.
- Prefer **`tracing`'s structured fields** (`field = value`) over formatted
  messages.
- **Never use `println!` or `eprintln!`** — including leftover debug output in
  `verum-macros`. **This is mechanically enforced** (below).

---

## How it is enforced (wired in T-M0-12)

This section is what turned the policy from "written down" into "held to".

```toml
# Cargo.toml
[workspace.lints.clippy]
disallowed_macros = "forbid"

# clippy.toml — the list and the reasons live here
disallowed-macros = [
    { path = "std::println",  reason = "Verum is a library: use tracing with an explicit target (…)" },
    { path = "std::eprintln", reason = "…" },
    { path = "std::print",  ... }, { path = "std::eprint", ... },
    { path = "std::dbg",    reason = "leftover debug output; …" },
]
```

**Why a lint and not a grep.** Matching happens on the macro's *path*, so
`std::println!`, bringing it in with `use`, and reformatting all fail to evade
it. Every one of the three text-scanning guards added in #9 was walked around
through exactly these routes — a subdirectory, a substring that did not match,
a `use … as` alias — and this avoids repeating that. No new CI job is needed
either: the existing Clippy job already runs with `-D warnings`.

**`forbid`, not `deny`.** Measured: under `deny`, a single
`#[allow(clippy::disallowed_macros)]` **silences it completely**. Under `forbid`
the same line is a **hard error, E0453**. The justification attached to
`unsafe_code = "forbid"` applies unchanged — a stray `#[allow]` cannot quiet it,
so relaxing the policy always appears in a diff.

**A list of macros is not enough.** Found by trying to walk around it during
review: `write!(std::io::stdout(), "x")`, `writeln!(std::io::stderr(), "x")` and
`std::io::stdout().write_all(b"x")` all slip past `disallowed-macros`. The macro
that expands is `std::write`, and **`write!` cannot be banned** — it is
legitimate in every `Display` and `Debug` impl. What has to be caught is the
**function that hands out the stream**.

```toml
# clippy.toml
disallowed-methods = [
    { path = "std::io::stdout", reason = "…" },
    { path = "std::io::stderr", reason = "…" },
]
```

Measured: this catches all four routes, and `write!(f, ..)` inside a `Display`
impl is **not** a false positive.

**`reason` is not decoration.** Whoever trips the lint needs the policy, not the
lint's name — the same reasoning
[`../specs/diagnostics.md`](../specs/diagnostics.md) applies to Verum's other
errors.

**Test code is included.** This section grants no exception, and a `println!`
that reached a commit inside a test is precisely "leftover debug output".
Removing it temporarily while debugging means editing `Cargo.toml`, and **that
edit showing up in a diff** is the point of `forbid`.

> **`--all-targets` is required in CI.** Without it, Clippy only looks at the lib
> target, so **a `println!` in a test fails locally and passes in CI**. Added to
> the Clippy job as part of T-M0-12.

**This is not one of the defence layers.** It is a lint, not a macro check, an
equality bound or a trait bound (the three layers in
[`../specs/diagnostics.md`](../specs/diagnostics.md)). What it protects is not a
capability guarantee but **how a framework ought to behave**.

### What this does not protect

Banning `println!` **does not close the route where a domain value ends up in a
`tracing::info!` field.** That is ledger path 4 — data leaking through `Debug` or
`Serialize`
([`../specs/unverified-boundaries.md`](../specs/unverified-boundaries.md)) — and
it stays open until `#[domain]` generates a `Debug` that prints declared
fields only. "Do not log user data", below, is **a rule, not an enforcement**.

---

## Verum is a framework

Unlike an application, the first priority is **not getting in the way of the
user's logging design**.

| Principle | Meaning |
|---|---|
| Do not init a subscriber | The user controls it |
| State the target | `tracing::info!(target: "verum::runtime", ..)` lets the user filter |
| Do not over-emit | Framework logs must not bury the application's |
| Do not log user data | No request body, no domain field values (below) |

---

## Span design

Open a span per endpoint, shaped so a user can follow a request through it.

```rust,ignore   // fragment, not a complete item
let span = tracing::info_span!(
    "verum.endpoint",
    endpoint = E::NAME,
    method = %E::METHOD_STR,
    path = E::PATH,
);
```

- **Prefix span names with `verum.`** so they are distinguishable from the
  application's own.
- Include the request id as a field when there is one — assumed to be attached by
  the user's middleware.
- **Do not nest spans deeply.** One per endpoint, plus one at each escape hatch,
  is enough.

---

## Levels

### `tracing::error!`

A fatal failure that affects correctness. **A library normally returns
`Result::Err` instead** and leaves the decision to the caller.

The exception is a **caught panic**, which cannot be returned to the caller and
so is recorded.

```rust,ignore   // fragment, not a complete item
tracing::error!(target: "verum::runtime", endpoint = E::NAME, "handler panicked; returning 500");
```

### `tracing::warn!`

An unexpected state that processing continues through.

```rust,ignore   // fragment, not a complete item
tracing::warn!(target: "verum::runtime", endpoint = E::NAME, "escape hatch used without declaration");
```

### `tracing::info!`

Lifecycle events only. **Never per request.**

```rust,ignore   // fragment, not a complete item
tracing::info!(target: "verum::runtime", routes = n, "server started");
```

### `tracing::debug!` and `trace!`

Internal tracing, written on the assumption that a filter removes them in
release builds.

---

## Do not log user data

Verum handles domain values, but **they belong to the user.**

```rust,compile_fail
// ❌ a domain field value
tracing::debug!(email = %user.email(), "updated");
```

```rust,compile_fail
// ❌ the request body
tracing::debug!(body = ?req, "handling");
```

```rust,ignore   // needs a crate or a verum-private module this harness does not carry
// ✅ identifiers, counts and the endpoint name
tracing::debug!(endpoint = E::NAME, fields_mutated = 2, "contract satisfied");
```

`read-contract.md` settles that the derive emits a `Debug` printing declared
fields only, but **that is still not a reason to log a domain value.** A safe
`Debug` impl does not make it acceptable to emit user data without the user
asking for it.

---

## The hot path

Request handling is a hot path ([`perf.md`](./perf.md), lean runtime).

- **No `info!` per request.** A span plus `debug!` is the ceiling.
- Do not format strings for logging. Pass structured fields and let the
  subscriber decide.
- Pass static values (`E::NAME`, `E::PATH`) through as `&'static str`.

```rust,compile_fail
// ❌ formatting on every request
tracing::debug!("handling {} {}", method, path);
```

```rust,ignore   // needs a crate or a verum-private module this harness does not carry
// ✅ structured fields — close to zero cost when disabled
tracing::debug!(method = %method, path = E::PATH, "handling");
```

---

## Debug output from macros

While developing `verum-macros` it is normal to look at the generated code with
`eprintln!`. **It must not reach a commit.**

```bash
# use this to inspect generated code instead
cargo expand
```

---

## Never do this

- ❌ Initialise `tracing-subscriber` in a library.
- ❌ Use `println!` or `eprintln!`.
- ❌ Log a domain field value or a request body.
- ❌ Emit `info!` per request.
- ❌ Omit `target:` and make the output unfilterable.
- ❌ Swallow an error and log nothing.
