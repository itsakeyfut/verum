# Verum — crate layout, module split, naming

> Structural rules that cut across the codebase. The **canon of the design** is
> [`../specs/`](../specs/README.md) and the **reasoning behind each decision** is
> [`../adr/`](../adr/README.md); this document says where things go.
> The boundary of the public API is [`api-surface.md`](./api-surface.md).

---

## 1. Crate layout

**Start with two crates, the minimum.** A proc-macro crate has to be separate —
a crate with `proc-macro = true` cannot export anything else.

| Crate | Holds | Depends on |
|---|---|---|
| `verum-macros` | Derive and attribute macros | `syn`, `quote`, `proc-macro2` only |
| `verum` | Type definitions, `Ctx`, `Repo`, `Handler`, the runtime | `verum-macros`, tokio, hyper, tower, tower-http, http, http-body, and (during the PoC) axum |

Users depend on `verum` alone; it re-exports the macros.

### Crates live under `crates/`

```text
verum/                    ← repository root (a virtual workspace)
├── Cargo.toml            ← [workspace] members, [workspace.dependencies]
└── crates/
    ├── verum/
    └── verum-macros/
```

**Any crate added later goes under `crates/` too, absent a specific reason.**
Crates sitting directly at the root end up mixed in with `docs/` and the CI
configuration, and the layout stops being readable.

Paths in `[workspace.dependencies]` are relative to the workspace root
(`path = "crates/verum-macros"`).

### Each crate carries a symlink to the licence

```text
LICENSE-MIT                        ← the real file (mode 100644)
LICENSE-APACHE                     ← the real file
crates/verum/LICENSE-MIT           → ../../LICENSE-MIT   (mode 120000)
crates/verum/LICENSE-APACHE        → ../../LICENSE-APACHE
crates/verum-macros/LICENSE-*      → likewise
```

**`cargo package` does not include files from outside the package directory.**
Keeping the licences only at the root means the published `.crate` contains not
one byte of licence text while declaring `license = "MIT OR Apache-2.0"`.
`include = ["../../LICENSE-MIT"]` **is silently ignored rather than rejected**
(measured), so the options are a symlink or a real copy.

**Bundling is not a legal obligation.** MIT's "shall be included in all copies"
and Apache §4(a) both bind the *redistributor*; the copyright holder's own
distribution does not violate them, and crates.io does not require it. The value
is downstream — a user running `cargo vendor` can satisfy the terms — and for
licence scanners, since `cargo-deny` and `cargo-about` read inside the `.crate`.

By convention this is clearly the default. Of nine major multi-crate workspaces
(serde, futures-rs, sqlx, clap, diesel, axum, tokio, tower, bevy), **all** place
the licence in each crate: six by symlink, three by real copy. The dependencies
Verum uses — syn, proc-macro2, unicode-ident — ship them in the published
artifact as well.

**The reason for a symlink over a real copy** is to keep one real file and avoid
drift. Changing the year or the copyright holder means editing one file at the
root.

> The cost: on Windows, a checkout with `core.symlinks=false` expands a symlink
> into a text file containing the path string. Running `cargo package` from there
> **ships that string as the licence.** This relies on development and CI being
> on Linux — the same assumption serde makes.

### Do not split out a backend crate yet

No `verum-axum` or `verum-hyper` crate for now. **An abstraction with a single
implementation gets in the way later.** Split it out when a second backend
actually exists ([`api-surface.md`](./api-surface.md)).

Code that touches Axum is collected in `crates/verum/src/runtime/`.

---

## 2. Module layout

```text
crates/verum/src/
├── lib.rs           — re-exports only; no logic
├── prelude.rs       — what users bring into scope
├── sealed.rs        — the seal! macro and each seal (one per sealed trait, with matching type parameters)
├── endpoint.rs      — the Endpoint trait and method markers (Get, Head, Post, Put, Patch, Delete)
├── effect.rs        — Read / Mutate / Create / Delete / Emit / Call / When
├── typelevel.rs     — Has / Append / Lookup / Here / There
├── domain.rs        — the Field trait, Includes
├── capability.rs    — Ctx<'req, E>, Repo<'req, D, R, M>
├── handler.rs       — the Handler trait and the object-safe erasure layer
├── contract.rs      — ContractEntry, inventory collection, JSON output
├── error.rs         — VerumError
└── runtime/         — ★ the only place that touches Axum
    ├── mod.rs
    ├── server.rs
    └── axum_backend.rs
```

### Rules

- **No logic in `lib.rs`.** Re-exports and crate attributes such as
  `#![forbid(unsafe_code)]`, nothing else.
- **Do not import Axum outside `runtime/`.** CI checks this by grep.
- **The type-level parts (`typelevel.rs`) depend on nothing but `sealed`.** Pure
  type computation. `use crate::private` is the exception, required for the seal
  supertrait that [`api-surface.md`](./api-surface.md) §2 mandates — `sealed` is
  a leaf of the dependency graph.

---

## 3. Dependency direction

```text
typelevel  ←  effect  ←  endpoint  ←  capability  ←  handler  ←  runtime
              domain  ←──┘                    ↑
                                          contract
```

- **Dependencies point downward. No cycles.**
- `typelevel`, `effect` and `domain` know nothing about the runtime — they are
  pure type definitions.
- Only `runtime` knows the external HTTP stack.

---

## 4. Apply Verum's own thinking to Verum's code

Verum is a framework for building codebases that do not depend on comments. **Its
own code should be one.**

- Express intent through types and names; comments explain only *why*
  ([`rust.md`](./rust.md)).
- Claims made in `specs/` are verified by doc tests and UI tests. **Do not leave
  a claim that is written down and never checked.**
- Mark unverified claims as unverified — the discipline of
  [`../specs/unverified-boundaries.md`](../specs/unverified-boundaries.md),
  applied to ourselves.

---

## 5. Naming

### Endpoint types are `<Operation><Domain>`

```rust,ignore   // fragment, not a complete item
GetUser / UpdateUser / SuspendUser / DeleteUser
```

Since the `operation` field was removed, **the type name carries the business
operation** ([`../specs/semantic-endpoint.md`](../specs/semantic-endpoint.md)).
Recommended, not enforced.

### Field marker types

```text
module = the domain name in snake_case
type   = the field name in PascalCase

User::email  →  user::Email
```

### Extension traits

```text
For Ctx  : Ctx<plural domain>     e.g. CtxUsers, CtxAuditLogs
For Repo : <Domain>Repo           e.g. UserRepo, AuditLogRepo
```

### Internal identifiers

Internal identifiers in generated code carry a `__verum_` prefix.

```rust,ignore   // fragment, not a complete item
__verum_user_ext / __VerumUpdateUserMutates
```

### Domain values are newtypes

```rust,ignore   // needs a crate or a verum-private module this harness does not carry
pub struct UserId(uuid::Uuid);
```

So a bare number or string cannot be passed where another was meant
([`rust.md`](./rust.md)).

---

## 6. Feature flags

Do not add one until it is needed. When adding one:

- **Keep it additive** — enabling a feature never removes an existing type or
  function.
- Default to `default = []`, and make heavy dependencies opt-in.
- Verify feature combinations in CI
  (`cargo hack --feature-powerset --no-dev-deps check`).

Features currently anticipated:

| Feature | Contents | State |
|---|---|---|
| `macros` | Derive and attribute macros (on by default) | First PoC |
| `contract-json` | AI Context JSON output | First PoC |
| `ws` | WebSocket | Full PoC and later |
| `sse` | Server-Sent Events | Full PoC and later |

---

## Never do this

- ❌ Import Axum, tower or `hyper_util` outside `runtime/`.
- ❌ Put logic in `lib.rs`.
- ❌ Create a backend trait with a single implementation.
- ❌ Make `typelevel.rs` depend on a runtime-side type.
- ❌ Make a feature non-additive — enabling it removes something.
