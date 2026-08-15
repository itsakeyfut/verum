//! The user's crate. One real endpoint, plus every probe behind a feature.
//!
//! Being a separate crate from `fw` is load-bearing: probe A0 asks whether user
//! code can conjure a `Ctx`, and that question only exists across a crate
//! boundary.

// All three fire on the subject of the measurement.
//
// `manual_async_fn` suggests rewriting `-> impl Future<..> + Send` as `async fn`
// — which is precisely what cannot be done: `async fn` gives no place to write
// `+ Send`, and probe D1 exists because that bound changes the answer.
//
// `needless_lifetimes` would elide the `'req` these signatures are here to show,
// and `type_complexity` would hide the boxed-future form under an alias.
#![allow(
    clippy::manual_async_fn,
    clippy::needless_lifetimes,
    clippy::type_complexity
)]

use fw::{Condition, Ctx, Domain, Endpoint, Error, Handler, Job, Req, Result};

// ---------------------------------------------------------------------------
// The baseline — the design as `docs/specs/capability-system.md` describes it.
// ---------------------------------------------------------------------------

pub struct UpdateUser;

impl Endpoint for UpdateUser {
    // Stand-ins. The cons-list machinery lives in `crates/verum/src/typelevel.rs`
    // and is already verified (#7/#8/#9); reproducing it here would add noise to
    // every error message this spike is trying to read.
    type Reads = ();
    type Mutates = ();
}

/// A1 — `Handler` with RPITIT, `+ Send`, taking `Ctx<'req, Self>` by value.
impl Handler for UpdateUser {
    type Req = Req;
    type Res = String;

    fn decode(body: &[u8]) -> Result<Req> {
        let s = std::str::from_utf8(body).map_err(|e| Error(e.to_string()))?;
        let (id, email) = s
            .split_once(':')
            .ok_or_else(|| Error(format!("malformed body: {s}")))?;
        Ok(Req {
            id: id.parse().map_err(|_| Error(format!("bad id: {id}")))?,
            email: email.to_owned(),
        })
    }

    fn encode(res: String) -> Vec<u8> {
        res.into_bytes()
    }

    fn handle<'req>(
        &self,
        req: Req,
        ctx: Ctx<'req, Self>,
    ) -> impl Future<Output = Result<String>> + Send {
        async move {
            let mut user = ctx.users().find(req.id)?;
            ctx.users().set_email(&mut user, req.email.clone())?;
            Ok(format!("ok:{}", user.email()))
        }
    }
}

pub struct EmailChanged;

impl Condition for EmailChanged {
    fn holds(user: &Domain, req: &Req) -> bool {
        user.email() != req.email
    }
}

pub struct SendEmailJob;
impl Job for SendEmailJob {}

// ---------------------------------------------------------------------------
// A2 — the RPITIT future is `Send` for *every* `Handler`, not just the one above
// ---------------------------------------------------------------------------

/// Generic over `H`, so it holds for all handlers rather than for `UpdateUser`
/// specifically. Removing `+ Send` from `Handler::handle`'s return type makes
/// this stop compiling.
///
/// **Known limit** (`docs/rules/test.md` §9-13): "the return type of this method
/// is `Send`" cannot be written as a pure type-level assertion, because RPITIT
/// associated types are not nameable. Gutting the body would leave this green.
/// The `const` below pins the signature, which is as far as the type system
/// reaches here.
pub fn a2_future_is_send<H: Handler>(h: &H, req: H::Req, ctx: Ctx<'_, H>) {
    fn is_send<T: Send>(_: &T) {}
    is_send(&h.handle(req, ctx));
}

const _: for<'a> fn(&UpdateUser, Req, Ctx<'a, UpdateUser>) = a2_future_is_send::<UpdateUser>;

// ---------------------------------------------------------------------------
// A0 — can user code construct a `Ctx` at all?
//
// The control for the entire suite. If this compiles, every rejection measured
// below is walk-aroundable by building a fresh `Ctx` and the table means nothing.
// ---------------------------------------------------------------------------

#[cfg(feature = "a0-forge-ctx")]
pub fn a0_forge_ctx(rt: &fw::Runtime) -> Ctx<'_, UpdateUser> {
    Ctx::new(rt)
}

// ---------------------------------------------------------------------------
// A3 — a handler body holding a non-`Send` value across an await
//
// Without this, A1 and A2 could both be passing under a vacuous bound.
// ---------------------------------------------------------------------------

#[cfg(feature = "a3-non-send-body")]
pub struct NotSendEndpoint;

#[cfg(feature = "a3-non-send-body")]
impl Endpoint for NotSendEndpoint {
    type Reads = ();
    type Mutates = ();
}

#[cfg(feature = "a3-non-send-body")]
impl Handler for NotSendEndpoint {
    type Req = Req;
    type Res = String;

    fn decode(_: &[u8]) -> Result<Req> {
        Ok(Req {
            id: 1,
            email: String::new(),
        })
    }

    fn encode(res: String) -> Vec<u8> {
        res.into_bytes()
    }

    fn handle<'req>(
        &self,
        _req: Req,
        _ctx: Ctx<'req, Self>,
    ) -> impl Future<Output = Result<String>> + Send {
        async move {
            let not_send = std::rc::Rc::new(1u8);
            tokio::task::yield_now().await;
            Ok(format!("{}", not_send))
        }
    }
}

// ---------------------------------------------------------------------------
// B2 — can `Ctx<'req, E>` appear in an erased handler's signature?
//
// Two separate reasons, measured separately. The first version of this probe
// conflated them: it asserted only `E0038` and concluded "`E` cannot be named",
// while rustc was in fact objecting to `Endpoint: Sized` and would have said the
// same with the `Ctx` parameter removed.
// ---------------------------------------------------------------------------

/// B2a — reason 1: `Ctx<'_, E>` requires `E: Endpoint`, and `Endpoint: Sized`,
/// so any trait carrying one is dyn-incompatible on that ground alone.
#[cfg(feature = "b2a-erased-sized")]
pub trait ErasedTakingCtx: Endpoint {
    fn call<'req>(&'req self, ctx: Ctx<'req, Self>) -> Result<()>;
}

#[cfg(feature = "b2a-erased-sized")]
pub fn b2a_router_of_erased(h: Box<dyn ErasedTakingCtx>) -> Box<dyn ErasedTakingCtx> {
    h
}

/// B2b — reason 2, and the robust one: with the `Sized` obligation removed, the
/// `Self` in the parameter position still blocks vtable dispatch.
///
/// This is what keeps the conclusion true if `Endpoint`'s bounds ever change.
#[cfg(feature = "b2b-erased-self-param")]
pub trait ErasedSelfParam {
    fn call<'req>(&'req self, ctx: fw::CtxNoSized<'req, Self>) -> Result<()>;
}

#[cfg(feature = "b2b-erased-self-param")]
pub fn b2b_router_of_erased(h: Box<dyn ErasedSelfParam>) -> Box<dyn ErasedSelfParam> {
    h
}

/// B2c — the control: the same shape with a **concrete** type in the `Ctx`
/// position compiles. Without it, B2a/B2b could be read as "`Ctx` may never
/// appear in a trait object", which is false.
pub trait ErasedConcreteCtx {
    fn call<'req>(&'req self, ctx: fw::CtxNoSized<'req, UpdateUser>) -> Result<()>;
}

pub fn b2c_router_of_erased(h: Box<dyn ErasedConcreteCtx>) -> Box<dyn ErasedConcreteCtx> {
    h
}

// ---------------------------------------------------------------------------
// C2 — the control for C1: `tokio::spawn` of an owned value works here
// ---------------------------------------------------------------------------

pub struct SpawnControl;

impl Endpoint for SpawnControl {
    type Reads = ();
    type Mutates = ();
}

impl Handler for SpawnControl {
    type Req = Req;
    type Res = String;

    fn decode(_: &[u8]) -> Result<Req> {
        Ok(Req {
            id: 1,
            email: String::new(),
        })
    }

    fn encode(res: String) -> Vec<u8> {
        res.into_bytes()
    }

    fn handle<'req>(
        &self,
        req: Req,
        _ctx: Ctx<'req, Self>,
    ) -> impl Future<Output = Result<String>> + Send {
        async move {
            // Owned, so `'static` is satisfied and spawn is usable from this
            // position. C1 fails because of what it captures, not because of
            // where it is written.
            let owned = req.email.clone();
            let h = tokio::spawn(async move { owned.len() });
            Ok(format!("{}", h.await.map_err(|e| Error(e.to_string()))?))
        }
    }
}

const _: fn() -> &'static str = || "C2 compiled";

// ---------------------------------------------------------------------------
// B4 — where does the `'static` on the service future actually come from?
//
// The first version of this probe wrote an undeclared `'a` in `type Future` and
// read the resulting `E0261` as "hyper leaves nowhere to put a lifetime". That
// was a typo-equivalent: the impl fails before it is ever matched against
// `Service`. B4b below shows hyper accepts a borrowing service outright.
// ---------------------------------------------------------------------------

/// B4a — `type Future` has no lifetime (so it is `'static`), and `call` tries to
/// borrow from `&self`.
///
/// Expected to **fail**: `Service::call(&self, ..)` offers no place to name the
/// borrow's lifetime, so a future that borrows `self` cannot be returned. This
/// is the real structural constraint.
#[cfg(feature = "b4a-borrow-from-self")]
pub struct SelfBorrowingSvc {
    rt: fw::Runtime,
}

#[cfg(feature = "b4a-borrow-from-self")]
impl hyper::service::Service<hyper::Request<hyper::body::Incoming>> for SelfBorrowingSvc {
    type Response = hyper::Response<http_body_util::Full<bytes::Bytes>>;
    type Error = Error;
    type Future = std::pin::Pin<Box<dyn Future<Output = Result<Self::Response>> + Send>>;

    fn call(&self, _req: hyper::Request<hyper::body::Incoming>) -> Self::Future {
        let rt = &self.rt;
        Box::pin(async move {
            let _ = rt.peek(1);
            Ok(hyper::Response::new(http_body_util::Full::new(
                bytes::Bytes::new(),
            )))
        })
    }
}

/// B4b — the **control**, and the correction: a service that carries its own
/// lifetime parameter, with `type Future` bounded by it, **compiles**.
///
/// hyper does not require the service future to be `'static`. What does is
/// `serve.rs`'s per-connection `tokio::spawn` — a choice of this design, not a
/// property of hyper. Recorded because the first version of the README asserted
/// the opposite.
pub struct BorrowingSvc<'a> {
    rt: &'a fw::Runtime,
}

impl<'a> hyper::service::Service<hyper::Request<hyper::body::Incoming>> for BorrowingSvc<'a> {
    type Response = hyper::Response<http_body_util::Full<bytes::Bytes>>;
    type Error = Error;
    type Future = std::pin::Pin<Box<dyn Future<Output = Result<Self::Response>> + Send + 'a>>;

    fn call(&self, _req: hyper::Request<hyper::body::Incoming>) -> Self::Future {
        let rt = self.rt;
        Box::pin(async move {
            let _ = rt.peek(1);
            Ok(hyper::Response::new(http_body_util::Full::new(
                bytes::Bytes::new(),
            )))
        })
    }
}

// ---------------------------------------------------------------------------
// C1 — `tokio::spawn` carrying the `Ctx` out of the request
//
// Ledger path 6. The recorded error text seeds the M3 UI test.
// ---------------------------------------------------------------------------

#[cfg(feature = "c1-spawn-ctx")]
pub struct SpawnCtx;

#[cfg(feature = "c1-spawn-ctx")]
impl Endpoint for SpawnCtx {
    type Reads = ();
    type Mutates = ();
}

#[cfg(feature = "c1-spawn-ctx")]
impl Handler for SpawnCtx {
    type Req = Req;
    type Res = String;

    fn decode(_: &[u8]) -> Result<Req> {
        Ok(Req {
            id: 1,
            email: String::new(),
        })
    }

    fn encode(res: String) -> Vec<u8> {
        res.into_bytes()
    }

    fn handle<'req>(
        &self,
        req: Req,
        ctx: Ctx<'req, Self>,
    ) -> impl Future<Output = Result<String>> + Send {
        async move {
            tokio::spawn(async move {
                let mut user = ctx.users().find(req.id)?;
                ctx.users().set_email(&mut user, req.email.clone())
            });
            Ok(String::from("spawned"))
        }
    }
}

// ---------------------------------------------------------------------------
// C3 — ledger path 7: hand the `Ctx` to a long-lived channel
// ---------------------------------------------------------------------------

#[cfg(feature = "c3-static-channel")]
pub static LEAK: std::sync::OnceLock<std::sync::mpsc::Sender<Ctx<'static, UpdateUser>>> =
    std::sync::OnceLock::new();

#[cfg(feature = "c3-static-channel")]
pub fn c3_send_ctx_to_static_channel(ctx: Ctx<'_, UpdateUser>) -> Result<()> {
    LEAK.get()
        .ok_or_else(|| Error("no sender".into()))?
        .send(ctx)
        .map_err(|e| Error(e.to_string()))
}

// ---------------------------------------------------------------------------
// D / E / F — `when`, capability-handle lifetimes, and `ctx.spawn`
//
// These take a `Ctx` as a parameter rather than each defining a whole `Handler`.
// `app` cannot *construct* one (probe A0), but receiving one is exactly the
// position a handler body is in, and the boilerplate would otherwise bury what
// is under test.
// ---------------------------------------------------------------------------

/// D1 — the conditional scope **exactly as the spec elides it**, inside a
/// future that is `+ Send` as `Handler` requires.
///
/// Compiles, and runs (`tests/live.rs`). The elision desugars to three
/// *independent* higher-ranked lifetimes, and that independence is what makes
/// it work — see `d1_bound_lifetimes` below.
pub fn d1_when_lends<E: Endpoint>(
    ctx: Ctx<'_, E>,
    req: Req,
) -> impl Future<Output = Result<String>> + Send {
    async move {
        let mut user = ctx.users().find(req.id)?;
        ctx.when::<EmailChanged, _>(&mut user, &req, async |c, u, r| {
            c.users().set_email(u, r.email.clone())?;
            Ok(())
        })
        .await?;
        Ok(user.email().to_owned())
    }
}

/// D1-bound — the same call against `when_bound`, whose signature binds the
/// three elided lifetimes into one `for<'a>`.
///
/// Expected to **fail**. This is the footgun: writing the elision out by hand
/// in the obvious way changes what the bound demands, and the closure can no
/// longer satisfy it once the surrounding future is `+ Send`.
#[cfg(feature = "d1-bound-lifetimes")]
pub fn d1_bound_lifetimes<E: Endpoint>(
    ctx: Ctx<'_, E>,
    req: Req,
) -> impl Future<Output = Result<String>> + Send {
    async move {
        let mut user = ctx.users().find(req.id)?;
        ctx.when_bound::<EmailChanged, _>(&mut user, &req, async |c, u, r| {
            c.users().set_email(u, r.email.clone())?;
            Ok(())
        })
        .await?;
        Ok(user.email().to_owned())
    }
}

/// D1d — an alternative that also compiles: `FnOnce` returning a boxed `Send`
/// future. Kept because it is what RK-005 recorded as a dead end, and the
/// record is only half right (D1e below is the half that holds).
///
/// Not a *necessary* alternative — `when` itself works. The cost is one
/// allocation per `when`.
pub fn d1d_when_boxed<E: Endpoint>(
    ctx: Ctx<'_, E>,
    req: Req,
) -> impl Future<Output = Result<String>> + Send {
    async move {
        let mut user = ctx.users().find(req.id)?;
        ctx.when_boxed::<EmailChanged, _>(&mut user, &req, |c, u, r| {
            Box::pin(async move {
                c.users().set_email(u, r.email.clone())?;
                Ok(())
            })
        })
        .await?;
        Ok(user.email().to_owned())
    }
}

/// D1e — RK-005's recorded dead end: `FnOnce(..) -> Fut` with a generic,
/// unboxed future. Expected to **fail** with `lifetime may not live long
/// enough`, which is the half of RK-005 that holds.
#[cfg(feature = "d1e-when-unboxed-fut")]
pub fn d1e_when_unboxed_fut<E: Endpoint>(
    ctx: Ctx<'_, E>,
    req: Req,
) -> impl Future<Output = Result<String>> + Send {
    async move {
        let mut user = ctx.users().find(req.id)?;
        ctx.when_unboxed_fut::<EmailChanged, _, _>(&mut user, &req, |c, u, r| async move {
            c.users().set_email(u, r.email.clone())?;
            Ok(())
        })
        .await?;
        Ok(user.email().to_owned())
    }
}

// D1 and D1d return `impl Future`, whose type cannot be named, so
// `const _: fn(..) = ..;` cannot pin them (`docs/rules/test.md` §9-13's recorded
// limit). Both are called from `tests/live.rs` and asserted on the store, which
// is strictly stronger — it also fails if the body is gutted.

/// D2 — RK-005's naive shape: the same values captured by the closure instead
/// of lent to it.
#[cfg(feature = "d2-when-capture")]
pub async fn d2_when_captures(ctx: Ctx<'_, UpdateUser>, req: Req) -> Result<String> {
    let mut user = ctx.users().find(req.id)?;
    ctx.when::<EmailChanged, _>(&mut user, &req, async |c, _u, _r| {
        c.users().set_email(&mut user, req.email.clone())?;
        Ok(())
    })
    .await?;
    Ok(user.email().to_owned())
}

/// D3 — ledger path 8 as specified: the closure's return type is fixed to
/// `Result<()>`, so `Ok(ctx)` should not type-check.
#[cfg(feature = "d3-when-leak-fixed-return")]
pub async fn d3_when_leak_fixed_return(ctx: Ctx<'_, UpdateUser>, req: Req) -> Result<()> {
    let mut user = ctx.users().find(req.id)?;
    ctx.when::<EmailChanged, _>(&mut user, &req, async |c, _u, _r| Ok(c))
        .await?;
    Ok(())
}

/// D4 — the same scope with the return type left free.
///
/// If "the return type is fixed to `Result<()>`" is what closes path 8, this
/// must let the `Ctx` out.
#[cfg(feature = "d4-when-leak-unconstrained")]
pub async fn d4_when_leak_unconstrained(ctx: Ctx<'_, UpdateUser>, req: Req) -> Result<()> {
    let escaped = ctx
        .when_unconstrained::<EmailChanged, _, _>(&req, async |c, _r| Ok(c))
        .await?;
    let _ = escaped.users().find(req.id)?;
    Ok(())
}

/// D5a — `when_named` (return type `Result<()>` as specified, but `'req`
/// **named** rather than higher-ranked) called from inside a `+ Send` handler
/// future, with **no leak attempted**.
///
/// Isolates the variable. If even the plain call fails, then D5b's rejection
/// says nothing about leaking.
#[cfg(feature = "d5a-when-named-call")]
pub fn d5a_when_named_call<E: Endpoint>(
    ctx: Ctx<'_, E>,
    req: Req,
) -> impl Future<Output = Result<String>> + Send {
    async move {
        ctx.when_named::<EmailChanged, _>(&req, async |c, _r| {
            let _ = c.users().find(req.id);
            Ok(())
        })
        .await?;
        Ok(String::new())
    }
}

/// D5b — the same, now assigning the scope's `Ctx` to an outer `Option` and
/// using it after the scope returned.
#[cfg(feature = "d5b-when-named-leak")]
pub fn d5b_when_named_leak<'req, E: Endpoint>(
    ctx: Ctx<'req, E>,
    req: Req,
) -> impl Future<Output = Result<String>> + Send + 'req {
    async move {
        let mut escaped: Option<Ctx<'req, E>> = None;
        ctx.when_named::<EmailChanged, _>(&req, async |c, _r| {
            escaped = Some(c);
            Ok(())
        })
        .await?;
        let leaked = escaped.ok_or_else(|| Error("condition did not hold".into()))?;
        let user = leaked.users().find(req.id)?;
        Ok(user.email().to_owned())
    }
}

/// E1 — the capability handle in the shape `capability-system.md:190`
/// specifies. No lifetime parameter, so it owns its access and is `'static`.
/// Expected to **compile and run**: the `Ctx` is correctly denied `'static`
/// while everything it hands out is not.
pub fn e1_handle_escapes<E: Endpoint>(
    ctx: Ctx<'_, E>,
    mut user: Domain,
    to: String,
) -> tokio::task::JoinHandle<()> {
    let repo = ctx.users();
    tokio::spawn(async move {
        // The sleep is what lets `tests/live.rs` observe the ordering: the
        // response is sent while this task is still parked, so a mutation seen
        // afterwards cannot be attributed to the request.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let _ = repo.set_email(&mut user, to);
    })
}

const _: for<'a> fn(Ctx<'a, UpdateUser>, Domain, String) -> tokio::task::JoinHandle<()> =
    e1_handle_escapes::<UpdateUser>;

/// E2 — #39 candidate 1 under the same attack.
#[cfg(feature = "e2-repo-lifetime-attack")]
pub fn e2_lifetime_handle_escapes(
    ctx: Ctx<'_, UpdateUser>,
    mut user: Domain,
    to: String,
) -> tokio::task::JoinHandle<()> {
    let repo = ctx.users_lt();
    tokio::spawn(async move {
        let _ = repo.set_email(&mut user, to);
    })
}

/// E3 — and candidate 1 still serves an ordinary handler.
///
/// Without this, E2's rejection could just mean the candidate is unusable.
pub fn e3_lifetime_handle_ordinary_use<E: Endpoint>(ctx: Ctx<'_, E>, req: &Req) -> Result<String> {
    let repo = ctx.users_lt();
    let mut user = repo.find(req.id)?;
    repo.set_email(&mut user, req.email.clone())?;
    Ok(user.email().to_owned())
}

const _: for<'a, 'b> fn(Ctx<'a, UpdateUser>, &'b Req) -> Result<String> =
    e3_lifetime_handle_ordinary_use::<UpdateUser>;

/// E4a — #39 candidate 2 under the same attack.
#[cfg(feature = "e4-repo-phantom-attack")]
pub fn e4_phantom_handle_escapes(
    ctx: Ctx<'_, UpdateUser>,
    mut user: Domain,
    to: String,
) -> tokio::task::JoinHandle<()> {
    let repo = ctx.users_phantom();
    tokio::spawn(async move {
        let _ = repo.set_email(&mut user, to);
    })
}

/// E4b — and candidate 2 still serves an ordinary handler.
pub fn e4_phantom_handle_ordinary_use<E: Endpoint>(ctx: Ctx<'_, E>, req: &Req) -> Result<String> {
    let repo = ctx.users_phantom();
    let mut user = repo.find(req.id)?;
    repo.set_email(&mut user, req.email.clone())?;
    Ok(user.email().to_owned())
}

const _: for<'a, 'b> fn(Ctx<'a, UpdateUser>, &'b Req) -> Result<String> =
    e4_phantom_handle_ordinary_use::<UpdateUser>;

/// F1 — the `ctx.spawn::<Job>` shape `capability-system.md:55` and
/// `api-surface.md:525` promise. The failure is inside `fw` (see
/// `fw::Ctx::spec_spawn`); this call site is what makes the feature reachable.
#[cfg(feature = "f1-spec-spawn")]
pub fn f1_spec_spawn(ctx: Ctx<'_, UpdateUser>) -> tokio::task::JoinHandle<Result<()>> {
    ctx.spec_spawn::<SendEmailJob, _>(async |c| {
        let _ = c.users().find(1)?;
        Ok(())
    })
}

/// F2 — #40's candidate: the child receives an **owned**, `'static` context.
pub fn f2_owned_jobctx<E: Endpoint>(
    ctx: Ctx<'_, E>,
    id: u64,
) -> tokio::task::JoinHandle<Result<()>> {
    ctx.spawn_owned::<SendEmailJob, _, _>(move |jctx| async move {
        jctx.set_email(id, "job@example.com".to_owned())
    })
}

const _: for<'a> fn(Ctx<'a, UpdateUser>, u64) -> tokio::task::JoinHandle<Result<()>> =
    f2_owned_jobctx::<UpdateUser>;

/// F3 — the cost of F2. `JobCtx` is `'static`, so the child can spawn it onward
/// without bound. Expected to **compile**: this is what `'req` was introduced to
/// prevent, reintroduced by the alternative that replaces it.
pub fn f3_respawn_jobctx<E: Endpoint>(
    ctx: Ctx<'_, E>,
    id: u64,
) -> tokio::task::JoinHandle<Result<()>> {
    ctx.spawn_owned::<SendEmailJob, _, _>(move |jctx| async move {
        tokio::spawn(async move {
            let _ = jctx.set_email(id, "grandchild@example.com".to_owned());
        });
        Ok(())
    })
}

const _: for<'a> fn(Ctx<'a, UpdateUser>, u64) -> tokio::task::JoinHandle<Result<()>> =
    f3_respawn_jobctx::<UpdateUser>;
