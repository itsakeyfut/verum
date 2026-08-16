//! `Ctx<'req, E>`, the capability handles it hands out, and the two scopes that
//! are supposed to be closed — `when` (ledger path 8) and `spawn` (path 6).

use std::collections::HashMap;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use crate::{Condition, Domain, Endpoint, Error, Job, Req, Result};

/// The shared store. `Arc` because `Repo` below has **no lifetime parameter**
/// and therefore cannot borrow anything — which is the whole of #39.
type Store = Arc<Mutex<HashMap<u64, Domain>>>;

/// What a request borrows. Owned by the server, outlives every request.
#[derive(Clone, Default)]
pub struct Runtime {
    store: Store,
}

impl Runtime {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn seed(&self, d: Domain) {
        self.store.lock().expect("store poisoned").insert(d.id(), d);
    }

    /// Reads straight out of the store, bypassing `Ctx`. Used by probes to
    /// observe what a leaked handle did *after* the request scope ended — the
    /// observation has to come from outside, or E1 would be asserting the
    /// leak's own account of itself.
    pub fn peek(&self, id: u64) -> Option<Domain> {
        self.store.lock().expect("store poisoned").get(&id).cloned()
    }
}

/// The request context.
///
/// `Send` but **not** `'static`: the `&'req Runtime` is what denies `'static`,
/// and `Runtime: Sync` is what keeps `Send`. `PhantomData<fn() -> E>` rather
/// than `PhantomData<E>` so the marker does not drag `E`'s auto traits into the
/// question — the subject here is the lifetime.
pub struct Ctx<'req, E: Endpoint> {
    rt: &'req Runtime,
    _e: PhantomData<fn() -> E>,
}

impl<'req, E: Endpoint> Ctx<'req, E> {
    /// `pub(crate)`, which is the whole of probe A0.
    ///
    /// `capability-system.md:66` requires a sealed `Runtime` token; here the
    /// visibility does the same job, because the question A0 asks is only
    /// "can `app` conjure a `Ctx`", not "is the token design sound".
    pub(crate) fn new(rt: &'req Runtime) -> Self {
        Self {
            rt,
            _e: PhantomData,
        }
    }

    /// The shape `docs/specs/capability-system.md:190` specifies:
    /// `fn users(&self) -> Repo<User, Self::R, Self::M>` — **no lifetime**.
    pub fn users(&self) -> Repo<Domain, E::Reads, E::Mutates> {
        Repo {
            store: Arc::clone(&self.rt.store),
            _p: PhantomData,
        }
    }

    /// #39 candidate 1: the handle borrows the request.
    pub fn users_lt(&self) -> RepoLt<'req, Domain, E::Reads, E::Mutates> {
        RepoLt {
            rt: self.rt,
            _p: PhantomData,
        }
    }

    /// #39 candidate 2: the handle still owns its access, but carries `'req` as
    /// a marker. Measured separately from candidate 1 because it does not make
    /// the handle depend on the `Runtime` borrow, which may matter for storing
    /// one in a struct later.
    pub fn users_phantom(&self) -> RepoPhantom<'req, Domain, E::Reads, E::Mutates> {
        RepoPhantom {
            store: Arc::clone(&self.rt.store),
            _p: PhantomData,
        }
    }

    /// The conditional scope, written **exactly as the spec elides it**.
    ///
    /// The elision matters and is the subject of probe D1: this desugars to
    /// three *independent* higher-ranked lifetimes. Writing it out by hand as
    /// `for<'a> AsyncFnOnce(Ctx<'a, E>, &'a mut Domain, &'a Req)` — binding all
    /// three together, which is the natural way to "make the elision explicit"
    /// — does **not** compile under the `+ Send` that `Handler` requires. See
    /// `when_bound` below.
    ///
    /// `user` and `req` are lent as closure parameters, not captured (RK-005),
    /// and the return type is fixed to `Result<()>` (ledger path 8, whose
    /// mechanism D3/D4/D5 correct).
    pub async fn when<C, F>(&self, user: &mut Domain, req: &Req, f: F) -> Result<()>
    where
        C: Condition,
        F: AsyncFnOnce(Ctx<'_, E>, &mut Domain, &Req) -> Result<()>,
    {
        if !C::holds(user, req) {
            return Ok(());
        }
        let child = Ctx::new(self.rt);
        f(child, user, req).await
    }

    /// D1 — the same scope with the three elided lifetimes **bound together**.
    ///
    /// This is what an implementer writes when making `when`'s elision explicit,
    /// and it is the only form in the family that fails. Behind no feature
    /// because the *method* compiles; it is the call site that cannot satisfy
    /// the bound once the surrounding future is `+ Send`.
    pub async fn when_bound<C, F>(&self, user: &mut Domain, req: &Req, f: F) -> Result<()>
    where
        C: Condition,
        F: for<'a> AsyncFnOnce(Ctx<'a, E>, &'a mut Domain, &'a Req) -> Result<()>,
    {
        if !C::holds(user, req) {
            return Ok(());
        }
        f(Ctx::new(self.rt), user, req).await
    }

    /// D1d — candidate: `FnOnce` returning a **boxed** `Send` future, with
    /// `user` and `req` still lent as parameters.
    ///
    /// This is the shape RK-005 records as a dead end ("`FnOnce(..) -> Fut`
    /// 方式に逃げても…借用を跨げない"). Measured: that holds for the *unboxed*
    /// generic-`Fut` form (`when_unboxed_fut` below) and not for this one. The
    /// cost is one allocation per `when`.
    pub async fn when_boxed<C, F>(&self, user: &mut Domain, req: &Req, f: F) -> Result<()>
    where
        C: Condition,
        F: for<'a> FnOnce(
            Ctx<'a, E>,
            &'a mut Domain,
            &'a Req,
        ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>,
    {
        if !C::holds(user, req) {
            return Ok(());
        }
        f(Ctx::new(self.rt), user, req).await
    }

    /// D1e — RK-005's recorded dead end, kept so the half of that entry which
    /// *does* hold is measured rather than inherited.
    pub async fn when_unboxed_fut<C, F, Fut>(
        &self,
        user: &mut Domain,
        req: &Req,
        f: F,
    ) -> Result<()>
    where
        C: Condition,
        F: for<'a> FnOnce(Ctx<'a, E>, &'a mut Domain, &'a Req) -> Fut,
        Fut: Future<Output = Result<()>> + Send,
    {
        if !C::holds(user, req) {
            return Ok(());
        }
        f(Ctx::new(self.rt), user, req).await
    }

    /// D4 — the same scope with the closure's return type left free.
    ///
    /// If path 8 is closed by "the return type is fixed to `Result<()>`", this
    /// one must let `Ok(ctx)` through. If it does not, the recorded remedy names
    /// the wrong mechanism.
    pub async fn when_unconstrained<C, F, R>(&self, req: &Req, f: F) -> Result<R>
    where
        C: Condition,
        F: for<'a> AsyncFnOnce(Ctx<'a, E>, &'a Req) -> Result<R>,
    {
        let child = Ctx::new(self.rt);
        f(child, req).await
    }

    /// D5 — return type fixed to `Result<()>` exactly as specified, but `'req`
    /// is **named** rather than higher-ranked.
    ///
    /// This is the signature an implementer reaches for when fighting
    /// "lifetime may not live long enough" from the higher-ranked form. It
    /// satisfies every word of the written rule.
    pub async fn when_named<C, F>(&self, req: &Req, f: F) -> Result<()>
    where
        C: Condition,
        F: AsyncFnOnce(Ctx<'req, E>, &Req) -> Result<()>,
    {
        let child = Ctx::new(self.rt);
        f(child, req).await
    }

    /// F2 — #40's candidate: hand the child task an **owned** context.
    ///
    /// This compiles where `spec_spawn` (below, behind `f1-spec-spawn`) does
    /// not. The cost is measured by probe F3: `JobCtx` is `'static`, so a
    /// `'static` capability-carrying type now exists, which is the thing `'req`
    /// was introduced to rule out.
    pub fn spawn_owned<J, F, Fut>(&self, f: F) -> tokio::task::JoinHandle<Result<()>>
    where
        J: Job,
        F: FnOnce(JobCtx<J>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        let jctx = JobCtx {
            store: Arc::clone(&self.rt.store),
            _j: PhantomData,
        };
        tokio::spawn(async move { f(jctx).await })
    }

    /// F1 — the shape `capability-system.md:55` and `api-surface.md:525`
    /// promise: `ctx.spawn::<Job>(|jctx| async move { .. })` where the child
    /// receives a `Ctx` borrowing the parent.
    ///
    /// Behind a feature because it is **expected not to compile**. The body is
    /// the honest one — constructing the child `Ctx` from `self.rt` is the only
    /// thing it could do, and gutting it would make the probe vacuous.
    #[cfg(feature = "f1-spec-spawn")]
    pub fn spec_spawn<J, F>(&self, f: F) -> tokio::task::JoinHandle<Result<()>>
    where
        J: Job,
        F: for<'a> AsyncFnOnce(Ctx<'a, E>) -> Result<()> + Send + 'static,
    {
        let rt = self.rt;
        tokio::spawn(async move {
            let child = Ctx::new(rt);
            f(child).await
        })
    }
}

/// A `Ctx`-shaped type that does **not** require `E: Endpoint`.
///
/// Exists only for probe B2b. `Ctx` requires `E: Endpoint`, and `Endpoint:
/// Sized`, so any trait with `Ctx<'_, Self>` in a method is dyn-incompatible
/// for that reason alone — which would hide the second, more robust reason.
/// B2b measures what happens once the `Sized` obligation is out of the way.
// The field is never read on purpose: probe B2b only needs the *shape* of the
// type in a trait method's parameter position, and giving it a body would mean
// giving `?Sized` types an accessor they cannot have.
#[allow(dead_code)]
pub struct CtxNoSized<'req, E: ?Sized>(&'req Runtime, PhantomData<fn() -> E>);

/// The capability handle, in the specified shape: **no lifetime parameter**.
///
/// Which forces it to own its access rather than borrow it, which is exactly
/// why it is `'static` and escapes. `R` and `M` stand in for `E::Reads` /
/// `E::Mutates`; no `Has` bound is enforced here (that is #15).
pub struct Repo<D, R, M> {
    store: Store,
    _p: PhantomData<fn() -> (D, R, M)>,
}

impl<R, M> Repo<Domain, R, M> {
    pub fn find(&self, id: u64) -> Result<Domain> {
        self.store
            .lock()
            .expect("store poisoned")
            .get(&id)
            .cloned()
            .ok_or_else(|| Error(format!("no such user: {id}")))
    }

    pub fn set_email(&self, user: &mut Domain, v: String) -> Result<()> {
        user.set_email_raw(v.clone());
        self.store
            .lock()
            .expect("store poisoned")
            .entry(user.id())
            .and_modify(|d| d.set_email_raw(v));
        Ok(())
    }
}

/// #39 candidate 1 — the handle borrows the request.
pub struct RepoLt<'req, D, R, M> {
    rt: &'req Runtime,
    _p: PhantomData<fn() -> (D, R, M)>,
}

impl<R, M> RepoLt<'_, Domain, R, M> {
    pub fn find(&self, id: u64) -> Result<Domain> {
        self.rt
            .peek(id)
            .ok_or_else(|| Error(format!("no such user: {id}")))
    }

    pub fn set_email(&self, user: &mut Domain, v: String) -> Result<()> {
        user.set_email_raw(v.clone());
        self.rt
            .store
            .lock()
            .expect("store poisoned")
            .entry(user.id())
            .and_modify(|d| d.set_email_raw(v));
        Ok(())
    }
}

/// #39 candidate 2 — owns its access, carries `'req` as a marker only.
pub struct RepoPhantom<'req, D, R, M> {
    store: Store,
    _p: PhantomData<(fn() -> (D, R, M), &'req ())>,
}

impl<R, M> RepoPhantom<'_, Domain, R, M> {
    pub fn find(&self, id: u64) -> Result<Domain> {
        self.store
            .lock()
            .expect("store poisoned")
            .get(&id)
            .cloned()
            .ok_or_else(|| Error(format!("no such user: {id}")))
    }

    pub fn set_email(&self, user: &mut Domain, v: String) -> Result<()> {
        user.set_email_raw(v.clone());
        self.store
            .lock()
            .expect("store poisoned")
            .entry(user.id())
            .and_modify(|d| d.set_email_raw(v));
        Ok(())
    }
}

/// F2's owned child context. `'static` by construction — that is the point, and
/// the cost.
pub struct JobCtx<J: Job> {
    store: Store,
    _j: PhantomData<fn() -> J>,
}

impl<J: Job> JobCtx<J> {
    pub fn set_email(&self, id: u64, v: String) -> Result<()> {
        self.store
            .lock()
            .expect("store poisoned")
            .entry(id)
            .and_modify(|d| d.set_email_raw(v));
        Ok(())
    }
}
