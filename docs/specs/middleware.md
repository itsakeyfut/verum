# Middleware

Laying a typed path through middleware and the low-level APIs, including where
the boundary with tower / tower-http sits.

Related: [`runtime-stack.md`](./runtime-stack.md),
[`capability-system.md`](./capability-system.md).

---

## Approach

Being able to use middleware and the lower layers freely matters. What is to be
avoided is the middleware that does everything:

```rust,ignore   // fragment, not a complete item
async fn middleware(req, next) {
    // Authentication
    // Database
    // Cache
    // External API
    // Logging
    // Response manipulation
    // etc.
}
```

Components with a clear meaning are provided instead.

```text
AuthenticationMiddleware
LoggingMiddleware
RateLimitMiddleware
TracingMiddleware
CacheMiddleware
```

For each one, the following are expressed in types or metadata:

- Allowed effects
- Forbidden effects
- Capabilities
- Inputs
- Outputs

---

## Assessing `tower::Service`

```rust
trait Service<Request> {
    type Response;
    type Error;
    type Future: Future<Output = Result<Self::Response, Self::Error>>;
    fn poll_ready(&mut self, cx: &mut Context) -> Poll<Result<(), Self::Error>>;
    fn call(&mut self, req: Request) -> Self::Future;
}
```

This is **a design from 2019, before `async fn` in traits existed**, and it fits
Verum badly in three ways.

1. **Only three type parameters: request, response, error.**
   - There is structurally nowhere to carry an effect or a capability.
   - "Express a middleware's allowed and forbidden effects in types" cannot be
     written with tower's types.

2. **`&mut self` plus `poll_ready`.**
   - Backpressure is conceptually elegant, but 99% of HTTP middleware just
     returns `Poll::Ready(Ok(()))`.
   - `&mut self` means concurrent invocation requires `Clone`.
   - Today it can be written as
     `fn call(&self, req) -> impl Future<Output = Response> + Send` — **RPITIT**.
   - **Not AFIT (`async fn` in trait).** The chain cannot hold a
     `Box<dyn Middleware>` (`E0038`), and the future is not `Send`, so it does
     not load onto hyper's multi-thread runtime (both measured;
     [`rust-type-model.md`](./rust-type-model.md)). dyn compatibility is bought
     back with an **erasure layer** — that is the cost included in the 200 lines
     below.

3. **`type Error`.**
   - In an HTTP server, an error is also a response.
   - Axum pins it to `Error = Infallible`, so the parameter is effectively dead.

**Conclusion:** the `Service` trait is not an abstraction Verum wants to adopt.
It is the adapter needed to use tower-http.

---

## The boundary

tower is treated as a boundary, not a dependency.

```text
hyper connection
      ↓
┌─ the tower Service world (outermost infrastructure) ──┐
│  tower-http: CORS / Compression / Trace / Timeout     │
└───────────────────────────────────────────────────────┘
      ↓  ← the only boundary; one adapter
┌─ Verum's world ───────────────────────────────────────┐
│  Router                                                │
│      ↓                                                 │
│  Semantic middleware chain (RPITIT, &self, effects)    │
│      ↓                                                 │
│  Endpoint<Effects, Capabilities>                        │
└────────────────────────────────────────────────────────┘
```

This gives:

- tower-http's proven logic, used as it is.
- Verum's middleware API written in current Rust idiom — **RPITIT with `Send`**,
  `&self`, effect type parameters.
- Breaking changes from tower 0.5 to 0.6 **confined to one adapter.**
- Users and AI never see a tower type.

### There is no need to reimplement tower

All Verum needs is composing a middleware chain. tower's `discover`, `balance`,
`retry`, `buffer` and `load` are not needed at all.

A hand-written middleware trait plus chain composition is roughly **200 lines**.
Not a reimplementation of tower — **only the parts needed, in a current shape.**

> **The original estimate was 100 lines.**
> [`runtime-stack.md`](./runtime-stack.md) revised it to 200 once the erasure
> layer's cost was included, and this file had not received that correction.
> **The estimate table in [`runtime-stack.md`](./runtime-stack.md) is the
> canon**; this document cites it.

---

## Where middleware divides

The middleware list splits exactly along this boundary.

| Middleware | Owner | Reason |
|---|---|---|
| CORS / Compression / Tracing / Logging | tower-http | The behaviour is fixed. These are effects permitted even on a GET |
| **Authentication** | **Verum** | It issues capabilities. This is the part that belongs in types |
| **Rate limiting** | **Verum** | It carries CacheRead / CacheWrite effects. Treated as a capability rather than as `tower::limit` |
| **Cache** | **Verum** | CacheRead and CacheWrite are involved |

Authentication middleware issuing capabilities that then flow to the endpoint is
the core of the capability system, and tower's types cannot express it.

The dividing line is **"does it carry meaning, or is it infrastructure?"**
