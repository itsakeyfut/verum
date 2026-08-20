//! T-M1-07 / #37 — the probes.
//!
//! `UpdateUser` below is `docs/specs/handler-rules.md` §The complete
//! implementation example, expression for expression. That example is
//! deliberately the hard case: a `when` block, an `after_commit` block, and a
//! free-function constructor.
//!
//! **Four lines differ.** Two are findings — `req.name.clone()` (`E0382`) and
//! `async |ctx|` (`E0282`), see README.md §Two by-products. Two are not:
//! `AuditLog::user_updated(&ctx, ..)` adds a `&ctx` so P4's free function can
//! reach an effect at all, and `Ok(UserView)` replaces `Ok(UserView::from(user))`
//! for no reason (both compile). rustfmt additionally reflows a couple of call
//! chains, which the scan cannot see: it reads the AST, and rustfmt preserves it.
//!
//! WHAT IS REAL HERE
//!   The tokens. Everything the macro reads is exactly what a user would write.
//!
//! WHAT IS NOT REAL HERE
//!   The types. `Ctx`, the repository accessors and `when`'s signature are
//!   stand-ins that exist only so the crate compiles — `when`'s real
//!   higher-ranked signature was measured in T-M1-02 (#14) and is not re-measured
//!   here. The macro runs before type checking and reads tokens only, so the
//!   stand-ins cannot flatter the result. What they *could* hide is a case where
//!   the real signature makes a construct unwritable; that limit is in README.md.

// The `-> impl Future<..> + Send` form is the spec's, and T-M1-02 (#14) measured
// that it is load-bearing: `Handler::handle` is deliberately not an `async fn`.
// Taking clippy's suggestion here would silently change the shape under test.
#![allow(clippy::manual_async_fn)]

use mac::observe;
use std::future::Future;

// ---------------------------------------------------------------------------
// Stand-ins. Nothing here is under test.
// ---------------------------------------------------------------------------

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct Error;

pub struct User {
    name: String,
    email: String,
}

impl User {
    pub fn from_repr(repr: UserRepr) -> Self {
        Self {
            name: repr.name,
            email: repr.email,
        }
    }
}

pub struct UserRepr {
    pub name: String,
    pub email: String,
}

pub struct UpdateUserRequest {
    pub id: u64,
    pub name: String,
    pub email: String,
}

pub struct UserView;

impl From<User> for UserView {
    fn from(_: User) -> Self {
        Self
    }
}

pub struct AuditLog;

impl AuditLog {
    /// P4 — a free associated function. `handler-rules.md` Rule 2's "grep `ctx.`
    /// enumerates every effect" **depends on these being pure**, and this one is
    /// not: it emits. If the scan could see through it, `HiddenEvent` would
    /// appear in `Sneaky`'s output.
    pub fn user_updated(ctx: &Ctx, _user: &User) -> Self {
        let _ = ctx.events().emit(HiddenEvent);
        Self
    }
}

pub struct HiddenEvent;
pub struct UserUpdated;
pub struct EmailVerificationRequested;
pub struct EmailChanged;

impl UserUpdated {
    pub fn from(_u: &User) -> Self {
        Self
    }
}
impl EmailVerificationRequested {
    pub fn for_user(_u: &User) -> Self {
        Self
    }
}

pub struct Repo;

impl Repo {
    pub async fn find(&self, _id: u64) -> Result<User> {
        Ok(User {
            name: String::new(),
            email: String::new(),
        })
    }
    pub fn set_name(&self, u: &mut User, v: String) -> Result<()> {
        u.name = v;
        Ok(())
    }
    pub fn set_email(&self, u: &mut User, v: String) -> Result<()> {
        u.email = v;
        Ok(())
    }
    pub fn create<T>(&self, _v: T) -> Result<()> {
        Ok(())
    }
    pub fn emit<T>(&self, _v: T) -> Result<()> {
        Ok(())
    }
    pub async fn send_verification(&self, _u: &User) -> Result<()> {
        Ok(())
    }
}

pub struct Ctx;

impl Ctx {
    pub fn users(&self) -> Repo {
        Repo
    }
    pub fn audit_logs(&self) -> Repo {
        Repo
    }
    pub fn events(&self) -> Repo {
        Repo
    }
    pub fn email(&self) -> Repo {
        Repo
    }

    /// The token shape matches the spec. The signature does not — T-M1-02 owns
    /// the real higher-ranked one, and re-deriving it here would risk repeating
    /// the elision mistake #14 recorded.
    pub async fn when<C, F>(&self, _u: &mut User, _r: &UpdateUserRequest, _f: F) -> Result<()>
    where
        F: AsyncFnOnce(&Ctx, &mut User, &UpdateUserRequest) -> Result<()>,
    {
        Ok(())
    }

    pub async fn after_commit<F>(&self, _f: F) -> Result<()>
    where
        F: AsyncFnOnce(&Ctx) -> Result<()>,
    {
        Ok(())
    }
}

pub trait Handler {
    fn handle(
        &self,
        req: UpdateUserRequest,
        ctx: Ctx,
    ) -> impl Future<Output = Result<UserView>> + Send;
}

// ---------------------------------------------------------------------------
// P1 / P2 / P3 / P4 — the canonical worked example, verbatim.
// ---------------------------------------------------------------------------

pub struct UpdateUser;

#[observe]
impl Handler for UpdateUser {
    fn handle(
        &self,
        req: UpdateUserRequest,
        ctx: Ctx,
    ) -> impl Future<Output = Result<UserView>> + Send {
        async move {
            let mut user = ctx.users().find(req.id).await?;

            // DEVIATION FROM THE SPEC, and it is a finding — see README.md.
            // `handler-rules.md` writes `req.name`, which partially moves `req`
            // two lines before `&req` is passed to `when`: `E0382`. The example
            // clones `req.email` inside the closure but not `req.name`.
            ctx.users().set_name(&mut user, req.name.clone())?;

            ctx.when::<EmailChanged, _>(&mut user, &req, async |ctx, user, req| {
                ctx.users().set_email(user, req.email.clone())?;
                ctx.events()
                    .emit(EmailVerificationRequested::for_user(user))?;
                Ok(())
            })
            .await?;

            ctx.audit_logs()
                .create(AuditLog::user_updated(&ctx, &user))?;
            ctx.events().emit(UserUpdated::from(&user))?;

            // DEVIATION FROM THE SPEC, and it is a finding — see README.md.
            // `handler-rules.md` Rule 4 writes `|ctx| async move { .. }`. Under
            // an `AsyncFnOnce` bound, that form is `E0282` and `async |ctx| { .. }`
            // compiles. This is RK-005, which the spec applied to `when` and not
            // to `after_commit`. `AsyncFnOnce` is not the *only* bound that lets
            // the future borrow `ctx` — two others accept the original form —
            // so the finding is scoped to this bound.
            ctx.after_commit(async |ctx| ctx.email().send_verification(&user).await)
                .await?;

            Ok(UserView)
        }
    }
}

// ---------------------------------------------------------------------------
// P4's control — the same emit, written inline instead of inside the constructor.
// ---------------------------------------------------------------------------

pub struct SneakyControl;

#[observe]
impl Handler for SneakyControl {
    fn handle(
        &self,
        _req: UpdateUserRequest,
        ctx: Ctx,
    ) -> impl Future<Output = Result<UserView>> + Send {
        async move {
            ctx.events().emit(HiddenEvent)?;
            Ok(UserView)
        }
    }
}

// ---------------------------------------------------------------------------
// P5 — the escape hatch. `from_repr` does not go through `ctx.`, so it can never
// be an effect; the question is whether the scan can see it sitting in `handle`.
// ---------------------------------------------------------------------------

pub struct EscapeHatch;

#[observe]
impl Handler for EscapeHatch {
    fn handle(
        &self,
        req: UpdateUserRequest,
        _ctx: Ctx,
    ) -> impl Future<Output = Result<UserView>> + Send {
        async move {
            let user = User::from_repr(UserRepr {
                name: req.name,
                email: req.email,
            });
            Ok(UserView::from(user))
        }
    }
}

// ---------------------------------------------------------------------------
// P6 — aliasing. Ordinary Rust; the effect still happens.
// ---------------------------------------------------------------------------

pub struct Aliased;

#[observe]
impl Handler for Aliased {
    fn handle(
        &self,
        req: UpdateUserRequest,
        ctx: Ctx,
    ) -> impl Future<Output = Result<UserView>> + Send {
        async move {
            let mut user = ctx.users().find(req.id).await?;
            let repo = ctx.users();
            repo.set_name(&mut user, req.name)?;
            Ok(UserView::from(user))
        }
    }
}

// ---------------------------------------------------------------------------
// P7 — a sibling method on the same impl block. Unlike a free function, this IS
// inside the tokens the attribute was given.
// ---------------------------------------------------------------------------

pub struct ViaHelper;

#[observe]
impl Handler for ViaHelper {
    fn handle(
        &self,
        req: UpdateUserRequest,
        ctx: Ctx,
    ) -> impl Future<Output = Result<UserView>> + Send {
        async move {
            let mut user = ctx.users().find(req.id).await?;
            Self::apply(&ctx, &mut user, req.name)?;
            Ok(UserView::from(user))
        }
    }
}

impl ViaHelper {
    fn apply(ctx: &Ctx, user: &mut User, name: String) -> Result<()> {
        ctx.users().set_name(user, name)
    }
}

// ---------------------------------------------------------------------------
// Routes found in Tier-2 review. Each is ordinary Rust and none was in the
// issue's list of five.
// ---------------------------------------------------------------------------

pub struct AlsoVerified;

/// **V1 — the parameter is spelled `cx`.** An impl need not reuse the trait's
/// parameter names and nothing warns. The scan matches the receiver by spelling
/// (`is_ctx`), so this is expected to come back **byte-identical to `Noop`** for
/// a handler that mutates and emits. Closable at layer 1 by checking the name.
pub struct RenamedCtx;

#[observe]
impl Handler for RenamedCtx {
    fn handle(
        &self,
        req: UpdateUserRequest,
        cx: Ctx,
    ) -> impl Future<Output = Result<UserView>> + Send {
        async move {
            let mut user = cx.users().find(req.id).await?;
            cx.users().set_name(&mut user, req.name)?;
            cx.events().emit(UserUpdated::from(&user))?;
            Ok(UserView::from(user))
        }
    }
}

/// **V2 — a statement that is never compiled.** The feature is declared and
/// never enabled, and the type named does not exist. An attribute macro runs
/// **before** cfg-stripping, so this is expected to appear in the output: the
/// scan is not a subset of what the program does.
pub struct CfgGated;

#[observe]
impl Handler for CfgGated {
    fn handle(
        &self,
        _req: UpdateUserRequest,
        ctx: Ctx,
    ) -> impl Future<Output = Result<UserView>> + Send {
        async move {
            #[cfg(feature = "never-enabled")]
            ctx.events().emit(ThisTypeDoesNotExist)?;
            let _ = &ctx;
            Ok(UserView)
        }
    }
}

/// **W1 — a `when` inside a `when`.** Reality is `EmailChanged ∧ AlsoVerified`.
/// Reporting only the innermost condition would be an over-claim, so the scope
/// tag carries the whole stack.
pub struct NestedWhen;

#[observe]
impl Handler for NestedWhen {
    fn handle(
        &self,
        req: UpdateUserRequest,
        ctx: Ctx,
    ) -> impl Future<Output = Result<UserView>> + Send {
        async move {
            let mut user = ctx.users().find(req.id).await?;
            ctx.when::<EmailChanged, _>(&mut user, &req, async |ctx, user, req| {
                ctx.when::<AlsoVerified, _>(user, req, async |ctx, user, req| {
                    ctx.users().set_email(user, req.email.clone())?;
                    Ok(())
                })
                .await
            })
            .await?;
            Ok(UserView::from(user))
        }
    }
}

/// **X1 — the helper as a nested `fn` inside `handle`.** The same factoring as
/// `ViaHelper`, relocated. Expected **visible**: the visibility boundary is the
/// item the attribute is attached to, and a nested `fn` is inside it.
pub struct NestedFnHelper;

#[observe]
impl Handler for NestedFnHelper {
    fn handle(
        &self,
        req: UpdateUserRequest,
        ctx: Ctx,
    ) -> impl Future<Output = Result<UserView>> + Send {
        async move {
            fn apply(ctx: &Ctx, user: &mut User, name: String) -> Result<()> {
                ctx.users().set_name(user, name)
            }
            let mut user = ctx.users().find(req.id).await?;
            apply(&ctx, &mut user, req.name)?;
            Ok(UserView::from(user))
        }
    }
}

/// **U1 — UFCS.** `Repo::set_name(&ctx.users(), ..)` is written directly in
/// `handle` with no indirection at all, and is an `ExprCall` rather than an
/// `ExprMethodCall`. Expected **invisible**.
pub struct Ufcs;

#[observe]
impl Handler for Ufcs {
    fn handle(
        &self,
        req: UpdateUserRequest,
        ctx: Ctx,
    ) -> impl Future<Output = Result<UserView>> + Send {
        async move {
            let mut user = ctx.users().find(req.id).await?;
            Repo::set_name(&ctx.users(), &mut user, req.name)?;
            Ok(UserView::from(user))
        }
    }
}

macro_rules! rename {
    ($ctx:expr, $u:expr, $n:expr) => {
        $ctx.users().set_name($u, $n)
    };
}

/// **M1 — the effect is produced by a `macro_rules!` expansion.** A proc macro
/// receives **unexpanded** tokens, so this is invisible by construction — and
/// unlike a helper it cannot be reached by any analysis inside this crate,
/// because the macro may come from another one. A second member of P7's class.
pub struct MacroExpanded;

#[observe]
impl Handler for MacroExpanded {
    fn handle(
        &self,
        req: UpdateUserRequest,
        ctx: Ctx,
    ) -> impl Future<Output = Result<UserView>> + Send {
        async move {
            let mut user = ctx.users().find(req.id).await?;
            rename!(ctx, &mut user, req.name)?;
            Ok(UserView::from(user))
        }
    }
}

// ---------------------------------------------------------------------------
// NOOP — the global control. A macro that emits a constant passes every
// positive probe; this is what makes that visible.
// ---------------------------------------------------------------------------

pub struct Noop;

#[observe]
impl Handler for Noop {
    fn handle(
        &self,
        _req: UpdateUserRequest,
        _ctx: Ctx,
    ) -> impl Future<Output = Result<UserView>> + Send {
        async move { Ok(UserView) }
    }
}

// ---------------------------------------------------------------------------
// R2 — could the helper simply live in the block the attribute sees?
//
// Not in the *trait impl*: a trait impl may contain only the trait's members, so
// `apply` beside `handle` is `E0407`.
//
// But that does NOT make the blindness structural, and an earlier version of this
// file said it did. Probe X1 is the refutation: the same helper written as a
// nested `fn` inside `handle`'s body IS seen, and so is a trait default method.
// The wall is the **item**, not `handle` — syn's visitor descends into nested
// items and closures. P7 is blind because the helper sits in a *sibling* item,
// which is a placement, not a law.
// ---------------------------------------------------------------------------

#[cfg(feature = "r2-helper-in-the-observed-block")]
pub struct HelperInBlock;

#[cfg(feature = "r2-helper-in-the-observed-block")]
#[observe]
impl Handler for HelperInBlock {
    fn handle(
        &self,
        req: UpdateUserRequest,
        ctx: Ctx,
    ) -> impl Future<Output = Result<UserView>> + Send {
        async move {
            let mut user = ctx.users().find(req.id).await?;
            Self::apply(&ctx, &mut user, req.name)?;
            Ok(UserView::from(user))
        }
    }

    fn apply(ctx: &Ctx, user: &mut User, name: String) -> Result<()> {
        ctx.users().set_name(user, name)
    }
}

// ---------------------------------------------------------------------------
// R1 — `#[observe]` on something that is not an impl block.
// ---------------------------------------------------------------------------

#[cfg(feature = "r1-observe-on-a-struct")]
#[observe]
pub struct NotAnImpl;

// §9-13 — and the anchor is `src/bin/observed.rs`, not a `const _` block.
//
// The first version of this file carried `const _: () = { let _ = __VERUM_OBSERVED_*; };`
// for every probe. **Measured: deleting one left the suite green** — the bin
// already names every const, so it is the bin that makes a removed probe a
// compile error. Deleting a whole probe endpoint is `FATAL: the baseline does
// not compile`, verified by planting it.
//
// The `const _` block was therefore a check that could not fail, which this
// project's own rule says is not a check. It is gone rather than kept for
// decoration.

// ---------------------------------------------------------------------------
// D1 / D2 — #42's defect 1: dead code is counted by BOTH bounds.
//
// This is the probe #42's requirement 4 asks for, and it is deliberately two
// rows, because the claim has two halves and only one of them can be measured
// with this crate's `Ctx`.
//
// Note what it is NOT. V2 above measures a `#[cfg]`-gated statement — code that
// is **never compiled**, which is the *unsoundness*. Dead code is compiled,
// type-checked, and never **run**. The two are different mechanisms and only one
// of them is also required to be declared.
// ---------------------------------------------------------------------------

/// **D1 — the lower-bound half.** An effect written inside `if false { .. }`
/// appears in the emitted JSON exactly as an unconditional one does. The scan
/// reads tokens; `if false` is a token.
pub struct DeadCode;

#[observe]
impl Handler for DeadCode {
    fn handle(
        &self,
        _req: UpdateUserRequest,
        ctx: Ctx,
    ) -> impl Future<Output = Result<UserView>> + Send {
        async move {
            // Never executed. Still scanned, still required to be declared (D2).
            if false {
                ctx.events().emit(UserUpdated)?;
            }
            Ok(UserView)
        }
    }
}

/// **D2 — the upper-bound half**, which this crate's `Ctx` cannot show: it carries
/// no effect-set parameter, so there is no `Has` bound to fail. Modelled with the
/// minimal shape instead, gated so the row can assert the rejection.
///
/// The point: `if false` does **not** relieve the declaration obligation. So a
/// declared-but-dead effect satisfies the upper bound *and* appears in the lower
/// one — and `declared \ observed ≠ ∅` sees nothing to report. The CI gate cannot
/// distinguish "declared and dead" from "declared and live".
#[cfg(feature = "d2-dead-code-still-declared")]
pub mod dead_code_upper_bound {
    use core::marker::PhantomData;

    pub struct Here;
    pub struct MutateEmail;
    pub struct MutateName;

    pub trait Has<E, I> {}
    impl<H, T> Has<H, Here> for (H, T) {}

    pub struct Repo<M>(PhantomData<fn() -> M>);
    impl<M> Repo<M> {
        pub fn set_email<I>(&self)
        where
            M: Has<MutateEmail, I>,
        {
        }
    }

    /// Declares `MutateName` only.
    type Declared = (MutateName, ());

    pub fn handler(repo: &Repo<Declared>) {
        if false {
            // `E0277`: unreachable code still has to satisfy the bound.
            repo.set_email();
        }
    }
}
