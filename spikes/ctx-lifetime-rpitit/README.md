# T-M1-02 / #14 — `Ctx<'req, E>` × RPITIT / hyper / async closure

Measured on **rustc 1.85.0** (Verum's MSRV), tokio 1.53.1, hyper 1.11.0,
hyper-util 0.1.20, http-body-util 0.1.5. 30 packages resolved, 28 of them
dependencies, 21 not already in the root graph.

```bash
bash run.sh          # 21 rows, each with its expected outcome and error text
```

> ### The gate this spike set for itself is satisfied
>
> It read: *"#43 → #38 → then re-derive the verdict from this harness"*, because
> the first version transcribed the `when` signature from an unchecked spec code
> block and built a whole verdict on the mis-transcription. See "What the first
> version got wrong" below. **Both are merged, the harness was re-run before the
> specs were touched, and the verdict below is what it printed.**

---

## Verdict

| Criterion | Result |
|---|---|
| **(a)** `Handler` definable with RPITIT (`-> impl Future + Send`) | **holds** |
| **(b)** that future loads on a multi-thread hyper server | **holds** |
| **(c)** `tokio::spawn` rejects the `Ctx` | **holds** — `E0521` |
| **(d)** `ctx.when`'s `AsyncFnOnce` shape | **holds** — the spec's elided signature compiles and runs |

Nothing here forces a design change. What the probes add is four narrower
results, each tied to a row:

1. **Writing `when`'s elision out by hand breaks it** (D1b). The spec elides
   three *independent* higher-ranked lifetimes; binding them into one `for<'a>`
   — the obvious way to "make it explicit" — no longer compiles under `+ Send`.
2. **A `Ctx` cannot cross the erasure boundary, for two separate reasons**
   (B2a/B2b), only one of which survives a change to `Endpoint`'s bounds.
3. **The `'static` on the service future comes from this design, not from
   hyper** (B4a/B4b).
4. **`Ctx` is bound to the request; everything it hands out is not** (E1,
   asserted at runtime) — which is #39, and #40's working candidate reopens it
   from a second direction (F2/F3).

**Ledger path 8's recorded mechanism is wrong, and a named `'req` variant is
open** — reachable from an ordinary handler, measured in review. See below.

---

## Probe table

`fail` means "rejected, carrying the error text named here". Exact arguments are
in `run.sh`.

### (a) RPITIT `Handler`

| # | Probe | Expected | Where |
|---|---|---|---|
| A1 | `fn handle<'req>(&self, req, ctx: Ctx<'req, Self>) -> impl Future<..> + Send` | pass | baseline + `tests/live.rs` |
| A2 | that future is `Send` for **every** `H: Handler` | pass | baseline |
| **A0** | `app` constructs its own `Ctx` | fail `E0624` | control for the whole suite |
| **A3** | a handler body holding an `Rc` across an `await` | fail `future cannot be sent between threads` | control for A1/A2 |

### (b) erasure layer, router, live server

| # | Probe | Expected | Note |
|---|---|---|---|
| B1 | `Box<dyn ErasedHandler>` in a router, blanket `impl<H: Handler>` | pass | baseline + B5 |
| B3 | `hyper::service::Service` with `type Future = Pin<Box<dyn Future + Send>>` | pass | baseline |
| **B2a** | an erased trait taking `Ctx<'req, Self>` | fail `because it requires 'Self: Sized'` | reason 1 |
| **B2b** | the same with the `Sized` obligation removed | fail `references the 'Self' type in this parameter` | reason 2 |
| B2c | the same with a **concrete** type in the `Ctx` position | pass | control — a `Ctx` per se is fine in a trait object |
| **B4a** | `Service::call` returning a future that borrows `&self` | fail `lifetime may not live long enough` | the real constraint |
| B4b | a service **carrying its own lifetime**, `type Future` bounded by it | pass | control — hyper permits it |
| B5+ | multi-thread runtime, real socket, dispatch through `Box<dyn _>`, response body asserted | pass | 7 tests |

### (c) `tokio::spawn` — ledger paths 6 and 7

| # | Probe | Expected |
|---|---|---|
| **C1** | `tokio::spawn(async move { ctx.users()… })` inside `handle` | fail `E0521` |
| C2 | `tokio::spawn` of an owned value from the same position | pass (baseline) |
| **C3** | the `Ctx` handed to a `'static` channel sender | fail `E0521` |

### `when` / `AsyncFnOnce` — RK-005 and ledger path 8

| # | Probe | Expected |
|---|---|---|
| D1 | the spec's signature, **elided as written**, inside a `+ Send` future | pass — baseline + runtime |
| **D1b** | the same elision **written out as one `for<'a>`** | fail `not general enough` |
| D1d | `FnOnce(..) -> Pin<Box<dyn Future + Send + 'a>>` | pass — baseline + runtime |
| **D1e** | `FnOnce(..) -> Fut`, unboxed | fail `lifetime may not live long enough` |
| **D2** | the same value lent **and** captured | fail `E0499` |
| **D3** | return type `Result<()>`, closure returns `Ok(ctx)` | fail `E0308` |
| **D4** | the same with the return type **free** | fail `lifetime may not live long enough` |
| **D5a** | `'req` **named** rather than higher-ranked, **no leak attempted** | fail `not general enough` |
| **D5b** | the same, now leaking the `Ctx` to an outer `Option` | fail `not general enough` |
| D5c | the same leaking body with `+ Send` dropped and nothing else | **pass** — the construction is real |
| **D5d** | D5c **awaited** from a `+ Send` position | fail `not general enough` |

### #39 / #40 — measured, not decided

| # | Probe | Expected |
|---|---|---|
| E1 | `Repo<D, R, M>` (specified shape, no lifetime) spawned out of the request | **pass = the defect**, asserted at runtime |
| **E2** | candidate 1 `RepoLt<'req, ..>` under the same attack | fail `E0521` |
| E3 | candidate 1 still serves an ordinary handler | pass — baseline + runtime |
| **E4a** | candidate 2 `RepoPhantom<'req, ..>` under the same attack | fail `E0521` |
| E4b | candidate 2 still serves an ordinary handler | pass (baseline) |
| **F1** | `ctx.spawn::<Job>` with the child borrowing the parent | fail `E0521` |
| F2 | candidate: an **owned** `JobCtx<J>` | pass — baseline + runtime |
| F3 | that `JobCtx` re-spawned from inside the job | **pass = the cost** (baseline) |

---

## What the results mean

### (d) holds, and the elision is load-bearing

The spec writes `AsyncFnOnce(Ctx<'_, ..>, &mut Domain, &Req) -> Result<()>`.
That desugars to **three independent** higher-ranked lifetimes and compiles
inside a `+ Send` handler future — D1, which also runs (`tests/live.rs`).

Binding them together does not:

```
error: implementation of `AsyncFnOnce` is not general enough
```

Measured against the realistic pattern (`user` read after the scope, inside
`+ Send`):

| Form | Result |
|---|---|
| `AsyncFnOnce(Ctx<'_,E>, &mut D, &R)` — **the spec, elided** | compiles |
| `for<'a,'b,'c> AsyncFnOnce(Ctx<'a,E>, &'b mut D, &'c R)` | compiles |
| `for<'a,'b> AsyncFnOnce(Ctx<'a,E>, &'b mut D, &'b R)` | rejected |
| `for<'a> AsyncFnOnce(Ctx<'a,E>, &'a mut D, &'a R)` | rejected — **D1b** |

The last row is the footgun and is the only new thing here: an implementer
making the elision explicit will reach for it.

RK-005's other two claims are confirmed as recorded: the unboxed
`FnOnce(..) -> Fut` form does not compile (D1e), and lending a value **while
capturing the same value** is a borrow error (D2, `E0499`).

### Ledger path 8 — the recorded remedy is not the mechanism

The remedy three documents record is "fix the closure's return type to
`Result<()>`". What is measured:

| Probe | Return type | `Ctx` lifetime | `+ Send` | Result |
|---|---|---|---|---|
| D3 | `Result<()>` | higher-ranked | yes | rejected `E0308` |
| D4 | **free** | higher-ranked | yes | rejected `lifetime may not live long enough` |
| D5a | `Result<()>` | **named** — *no leak attempted* | yes | rejected `not general enough` |
| D5b | `Result<()>` | **named** — leaking | yes | rejected `not general enough` |
| **D5c** | `Result<()>` | **named** — leaking | **no** | **compiles** |
| **D5d** | D5c called from a handler | | yes | rejected `not general enough` |

For the **specified** signature the remedy is redundant but harmless: D3 and D4
show the return type and the higher-ranked `Ctx` each reject `Ok(ctx)` on their
own. For a **named** `'req` — which is what an implementer reaches for when
"making the lifetime explicit" — the return type does nothing at all. D5c is that
form, and it type-checks.

**What closes the specified signature is the higher-ranked `Ctx`.** D4 isolates
it: strip the return-type constraint and the form is still rejected. The remedy
three documents (four, counting the ledger row itself) recorded — "fix the return
type to `Result<()>`" — is real but redundant.

**A named `'req` is closed by nothing.** D5c type-checks, and it is **reachable
from an ordinary handler**.

> ### ⚠️ `+ Send` does not close this path — withdrawn after Tier-2 review
>
> An earlier version of this section concluded that `+ Send` was the closer and
> that D5c had no caller, reasoning that `Handler::handle` and
> `ErasedHandler::call` both return `Send` futures. **That inference is wrong.**
>
> `Handler::handle` is `fn .. -> impl Future<..> + Send`, **not `async fn`**
> (`fw/src/erase.rs:25-29`). The bound constrains only what the *returned future*
> holds across awaits. The body is an ordinary synchronous body that already holds
> `Ctx<'req, Self>` with `'req` **named** — D5c's precondition — and it can drive
> the leaking future to completion before it ever builds the future it returns:
>
> ```text
> fn handle<'req>(&self, req, ctx: Ctx<'req, Self>) -> impl Future<..> + Send {
>     let out = block_on(leak_body(ctx, req));   // no await — no Send obligation
>     async move { out }
> }
> ```
>
> Two review agents built this independently and ran it against the real
> multi-thread hyper server; the store was mutated through a `Ctx` that outlived
> its `when` scope. Safe Rust, stdlib only (`Waker::noop`, stable in 1.85), no
> god-mode constructor, no relaxed `Send`.
>
> D5d fails only because it **awaits** — `.await` is the only thing that
> propagates the obligation. **The probe measured one call shape and the prose
> generalised it to "no caller"** — the same failure this README documents about
> its own first version, one level up. Recorded as RK-017.
>
> **The sync-body probe is not in this suite.** It should be, before anyone
> relies on path 8 again.

`+ Send` *is* the discriminator for the awaited form, measured both directions
(D5a fails with it, passes without; D5c passes without it, fails with). That is a
fact about HRTB inference, not containment.

**The remedy is a constraint on the signature, not a bound**: `when` must be
generated with the elided form and never with a named `'req`. `when` is
macro-generated, so that is enforceable at defence layer 1.

#### #44 was right

#44 recorded this path as leaking, "compiled and run". An earlier version of this
README reconciled that away as *"both are correct — #44 measured the
construction, this spike measured reachability"*. **There was nothing to
reconcile.** The leak is reachable; #44's measurement stands unqualified and this
spike's contradiction of it was the error.

### (b) holds; the two supporting facts are not what the first version said

**A `Ctx` cannot cross the erasure boundary** — but for two reasons, and only
B2b's survives a change to `Endpoint`. B2a fails because `Ctx<'_, E>` requires
`E: Endpoint` and `Endpoint: Sized`, which makes any such trait
dyn-incompatible on its own; B2b removes that obligation and the `Self` in the
parameter position still blocks vtable dispatch. B2c is the control: a `Ctx`
with a concrete type parameter sits in a trait object fine.

So the erased layer takes the `Runtime` and builds the `Ctx` on the far side of
the boxing (`fw/src/erase.rs`) — a signature no document describes.

**hyper does not force the service future to be `'static`.** B4b compiles: a
service carrying its own lifetime, with `type Future` bounded by it, is
accepted. What hyper *does* forbid is returning a future that borrows `&self`
(B4a) — `Service::call(&self, ..)` offers nowhere to name that borrow. The
`'static` in this design comes from `serve.rs`'s per-connection `tokio::spawn`,
which is a choice, not a constraint. Note this is measured for **http1 only**;
hyper's http2 builder additionally requires `S::Future: Send + 'static`.

### #39 — confirmed by execution

`Repo<D, R, M>` as specified has no lifetime parameter, so it cannot borrow and
must own its access, so it is `'static`. E1 spawns a task holding one, returns
the response, and the store still reads `before@example.com`; 150 ms later it
reads `escaped@example.com`. Both candidates block the attack (E2/E4a) and both
still serve an ordinary handler (E3/E4b).

### #40 — the promised alternative, and what the working one costs

F1: the child borrowing the parent cannot be implemented — `E0521`, the same
error as the `tokio::spawn` it replaces. F2: an **owned** `JobCtx<J>` compiles
and runs. F3: that `JobCtx` is `'static`, so the child can spawn it onward
without bound.

Note the coupling the design review found: `JobCtx` holds the store directly, so
adopting a lifetime-bound `Repo` for #39 does not help if #40 adopts F2.
**#39 and #40 cannot be decided independently.**

---

## What the first version of this README got wrong

Recorded because the failure is more useful than the result.

| Claim | Reality |
|---|---|
| "(d) does not hold — the specified signature does not compile" | The signature measured was not the one specified: three elided lifetimes were bound into one. The spec's form compiles and runs |
| "`Ctx` cannot appear in an erased signature (B2, `E0038`)" | `E0038` was firing on `Endpoint: Sized`; removing the `Ctx` parameter gave the same error. The probe measured nothing about `Ctx` |
| "hyper forces the service future to be `'static` (B4, `E0261`)" | `E0261` was an undeclared lifetime — a typo-equivalent. A borrowing service compiles |
| "path 8 is closed by higher-rankedness, not the return type" | The named-lifetime variant is not callable from a handler at all, so the comparison never held |
| "two surviving `when` candidates" | One of the two never evaluated its condition — its signature had no `user`/`req` to pass to `C::holds` |
| "27 packages" | 30 resolved, 28 dependencies, 21 not already in the root graph |

Every one of these was in prose arguing for a conclusion, and every probe table
row was individually green. **A probe that fails with the expected error code
can still be failing for a different reason** — `docs/rules/test.md` §9-1 asks
for the code and B2a/B4a both supplied it.

---

## Not measured

- **Sealing.** `Endpoint` is implementable by `app` here (#6 covers seals).
- **Field-granular capability checking** — `Repo::set_email` carries no `Has`
  bound; that is #15.
- **`Runtime::new` / `seed` / `peek` and `Domain::new` are `pub` in `fw`**, so
  `app` can read and write the store with no `Ctx` at all. A fidelity gap: do
  not read "the Capability system held" out of this spike.
- **`ErasedHandler::call` is a public trait method**, so `app` can `Box::leak` a
  `Runtime` and drive a handler with `'req = 'static`. A0 only shows `app`
  cannot *construct* a `Ctx`.
- **Elevated `when` scopes** (`WhenScope`, `Append`) — see path 8 above.
- **Ledger paths 9, 10, 11.**
- **Brand lifetimes (GhostCell)** — `capability-system.md` rejects them on
  diagnostics grounds; both cheaper candidates work, so the question did not
  arise. If #39 rejects both, revisit rather than inherit.
- **tower / middleware composition** — blocked on `middleware.md`.
- **http2** — B4b's result is http1-only.

---

## C1 — the text M3's UI test is seeded from

rustc 1.85.0, `cargo +1.85.0 check -p app --features c1-spawn-ctx`:

```
error[E0521]: borrowed data escapes outside of method
   --> app/src/lib.rs:348:13
    |
342 |       fn handle<'req>(
    |                 ---- lifetime `'req` defined here
...
345 |           ctx: Ctx<'req, Self>,
    |           --- `ctx` is a reference that is only valid in the method body
...
348 | /             tokio::spawn(async move {
349 | |                 let mut user = ctx.users().find(req.id)?;
350 | |                 ctx.users().set_email(&mut user, req.email.clone())
351 | |             });
    | |              ^
    | |              |
    | |______________`ctx` escapes the method body here
    |                argument requires that `'req` must outlive `'static`
```

`E0521` is a **rustc** diagnostic, not a trait-bound one, so
`#[diagnostic::on_unimplemented]` cannot reword it. Line numbers and the crate
name (`fw`) will both differ in `crates/verum`; the *shape* is what transfers.

---

## The harness

`run.sh` implements `docs/rules/test.md` §9. Proven by planting mutations, not
by reading:

| Mutation | Caught by |
|---|---|
| drop `+ Send` from `Handler::handle`'s return type | baseline `FATAL` |
| make `Ctx::new` public | A0 `fail → pass` |
| swap A3's `Rc` for an `Arc` | A3 `fail → pass` |
| gut `d1_when_lends`'s body | B5+ `pass → fail` |
| gut `e1_handle_escapes`'s body | B5+ `pass → fail` |
| delete one runtime test | B5+ count assertion (`7 passed`) |
| **delete one `probe` line from `run.sh`** | `FATAL: 20 rows ran, expected 21` |
| a feature-name typo in `#[cfg(..)]` | `unexpected_cfgs = "deny"`, all three compilation units |

The last two closed holes the first version had. Every endpoint in
`tests/live.rs` **delegates to the function in `app`'s lib** rather than
reimplementing it — without that, eleven pass-probe bodies could be emptied at
once and the suite still reported green (measured).

### Remaining limits

- **A2, C2, E4b and F3 have no runtime coverage** and are pinned only by a
  `const _: fn(..) = ..;` on their signature, or not at all. Gutting their
  bodies is not detected. `impl Future` return types cannot be named, so the
  type-level form is unavailable (§9-13's recorded limit).
- **`B5+` asserts a count, and a count is not an identity** (§9-2). Replacing
  one runtime test with another would pass.
- **The mutation script is not in the repository** — the table above was
  produced by hand and cannot be re-run from a checkout.
- The verdict is version-dependent; `run.sh` prints what it measured and asserts
  the toolchain is 1.85.0.
