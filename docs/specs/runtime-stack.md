# Runtime stack

Which layers are depended on and which are written here, plus the operating rule
that hides those dependencies.

> Consolidates the old §45 / §53 / §55 / §57 / §58. Decided 2026-08-13.

Related: [`middleware.md`](./middleware.md),
[`performance.md`](./performance.md).

---

## Prerequisites (language versions)

| Item | Requirement | Reason |
|---|---|---|
| **edition** | **2024** | The `when` scope requires async closures (`AsyncFnOnce`) |
| **MSRV** | **1.85+** | Async closures / `#[diagnostic::do_not_recommend]` |

Lending `&user` while passing it into the closure does not work in the
`FnOnce(..) -> Fut` form — the borrow cannot cross (confirmed by compiling).
Async closures are therefore a hard requirement. Detail in
[`rust-type-model.md`](./rust-type-model.md).

---

## The criterion

> **Do not build a concept whose specification is already settled. Put every
> resource into the unsolved design problems.**

---

## The decision

```text
Tokio                     — used (not rebuilt)
Hyper (+ hyper-util)      — used (not rebuilt)
Tower / tower-http        — used (not rebuilt)
─────────────────────────────────────────────
Router                    — written in Verum
Extractor                 — written in Verum
Handler / Endpoint        — written in Verum
Response                  — written in Verum
Middleware chain          — written in Verum
─────────────────────────────────────────────
Effect / Capability       — Verum proper
Mutation contract         — Verum proper
Semantic contract         — Verum proper
AI Context                — Verum proper
```

---

## Why these are not rebuilt

| Area | State | Owner |
|---|---|---|
| HTTP/1.1, HTTP/2 | Fixed by RFC 9112 / 9113 | Hyper |
| CORS | Fixed by WHATWG Fetch | tower-http |
| Content negotiation / compression | RFC 9110 | tower-http |
| Tracing span design | OpenTelemetry semantic conventions | tower-http |

These are **textbook, unchanging** concepts; rebuilding them differentiates
nothing.

Rebuilding the HTTP protocol layer in particular means taking on request
smuggling, HPACK bombs, and h2 rapid reset (CVE-2023-44487) as one's own security
risks. None of Verum's originality lives in the HTTP protocol layer.

And the performance target — Axum-class — comes almost automatically from using
Hyper. Rebuilt, reaching Axum-class becomes a project in itself, pushing the goal
further away.

---

## Why Axum is not used

Axum is still 0.x and has not reached a major version. It has a real history of
breaking changes.

```text
0.6 → 0.7
    axum::Server removed → axum::serve
    migration to hyper 1.0
    changes around FromRequest

0.7 → 0.8
    path syntax changed (/:id → /{id}) — every route definition breaks
    async_trait removed
    Option extractor handling changed
```

On top of that, Verum has **its own** router, extractor and handler anyway. It
would be replacing nearly all of Axum's value while taking on only its breaking
changes.

---

## Version-stability inventory

| Crate | Version | Judgement |
|---|---|---|
| tokio | 1.x | Stable. Compatibility guaranteed since 2020 |
| hyper | 1.x | Stable. Reached 1.0 in November 2023 |
| http / http-body | 1.x | Stable |
| tower | 0.5 | 0.x, but the `Service` trait has been effectively unchanged since 2019 |
| hyper-util | 0.1 | 0.x. A consequence of hyper 1.0 narrowing to low-level APIs. Hard to avoid |
| matchit | 0.8 | 0.x. Small enough to rebuild or vendor |

0.x dependencies are not excluded but **confined to a thin layer inside Verum**
(the Dependency Hiding Rule below).

---

## The Dependency Hiding Rule

The most important operating rule.

> **Not one type from a dependency that may be replaced appears in the `verum`
> crate's public API.**

Hiding is what buys the freedom to drop down later. Hidden from the start,
dropping Axum changes the public API not at all. Exposed, it can never be
dropped.

### What is hidden (replaced by a Verum type)

```text
axum::extract::State        ← the most important
axum::Router
axum::response::IntoResponse
axum::Json
axum::extract::Path / Query
axum::handler::Handler
tower::Service / Layer
hyper_util::*
matchit::*
```

#### Why `State` is the most important

Anything can be obtained from `State<AppState>`. Exposing it turns "make it
impossible to call in the first place" into a lie, and **breaks the capability
system at the root.**

Likewise, Axum's `Handler` trait accepts any number of arbitrary extractors. That
is not an escape hatch but **an unregistered bypass.**

### What is not hidden (re-exported, in fact)

```text
http::StatusCode
http::HeaderMap
http::Method
http::Uri
```

These are not Axum-specific but the **stable http 1.x** foundation, and exactly
the same types will be used after the rewrite. Hiding them is waste.

### What may be exposed

Escape hatches expose the low-level layer deliberately. But:

> **Going through an escape hatch is recorded in the contract**

This is not an exception to hiding; it is a feature that only works *because* of
the hiding.

### No backend trait for now

Code that touches Axum is simply collected into one module (`src/runtime/` or
similar).

**Do not create a trait with one implementation.** Split it out when a second
backend (a Hyper backend) is needed. At that point two implementations exist, and
the abstraction is justified.

### Keep the Axum features used in the PoC minimal

The more that is used, the more it costs to remove later.

```text
used:
    Router::route
    path parameters
    reading the body
    returning a response
    axum::serve

not used:
    State              ← breaks the capability system; the most expensive to remove
    Json extractor     ← Verum's own extractor is used
    middleware         ← not needed in the PoC
    WebSocket / SSE    ← not part of what the PoC verifies
```

---

## Scope and estimate of what is written here (at the point Axum is dropped)

| Needed | Means | Estimate |
|---|---|---|
| Accept loop + graceful shutdown | `hyper_util::server::conn::auto` (HTTP/1+2 auto-detection) | ~100 lines |
| Path matching | `matchit`, or written here (static + `{param}` + `{*rest}`) | ~200 lines |
| Response conversion | An own trait + `http-body-util`'s `Full` / `BoxBody` | ~150 lines |
| Extractor | **Already planned to be written here** (capability-based) | no addition |
| Handler trait | RPITIT + `Send`, **plus an object-safe erasure layer** | ~100 lines |
| Middleware chain | An own trait (RPITIT) + an erasure layer | ~200 lines |

**The real additional cost is around 600–800 lines.** Adding WebSocket (hyper
upgrade + tokio-tungstenite) and SSE is another +300, and neither is needed for
the initial PoC.

> **Why an erasure layer is needed**: `async fn` in trait (AFIT) is dyn
> incompatible, so the router cannot hold a `Box<dyn Handler>` (E0038). And with
> AFIT the future is not `Send`, so it does not load onto hyper's multi-thread
> runtime.
>
> - `Send` is solved by RPITIT (`-> impl Future<Output = ..> + Send`)
> - dyn compatibility is solved by having the derive generate an erasure layer
>   returning `Pin<Box<dyn Future<Output = Response> + Send + '_>>`
>
> The middleware chain is under the same constraint. The original estimate
> (middleware chain ~100 lines) did not include the erasure layer's cost and has
> been corrected. Detail in [`rust-type-model.md`](./rust-type-model.md).

### Deferred

```text
WebSocket
SSE
multipart
TLS (delegated to a reverse proxy initially)
compile-time route optimisation
```

### Included from the start (not omitted, because of the trust boundary)

```text
body size limit
request timeout
path normalisation (`..` and encoded separators)
```

Hyper underwrites the HTTP protocol's safety, but these three are the
responsibility of what is written here.

---

## The design benefits of not using Axum

There are positive benefits, not just a cost saving.

### 1. Variadic handler magic can be dropped

Axum's `Handler` generates 16 tuple impls by macro so it can take any number of
arbitrary extractors. That is human ergonomics, and for Verum it is **harmful.**

The point is to constrain what a handler may receive using capabilities, and that
intake is far too open. A fixed signature constrains better.

```text
async fn handle(&self, req: Request, caps: &Caps<Self::Effects>) -> Response
```

### 2. A compile-time route table

Through a derive macro plus `inventory`, **every endpoint is known at compile
time** in Verum. So there is no need to build a radix trie at run time; it can
become a `match` expression or perfect hashing.

That optimisation is structurally impossible in Axum's design.

A straightforward matcher is enough at first. Recorded as room to exploit later.

### 3. The performance target is met straightforwardly

Hyper is called directly, so Axum's layer of overhead does not exist.
"Axum-class" becomes the floor.

---

## A custom runtime, eventually

Considered only if the need becomes clear.

```text
Semantic framework
        ↓
Custom optimised runtime
```

For now, use the mature Rust web ecosystem and concentrate on building the
semantic layer.
