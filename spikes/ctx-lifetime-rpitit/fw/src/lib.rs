//! The `verum` role, cut down to what T-M1-02 / #14 has to measure.
//!
//! What is modelled, because the question depends on it:
//!
//! - `Ctx<'req, E>` — `Send`, not `'static`, constructor not reachable from `app`
//! - `Handler` — RPITIT, `-> impl Future + Send`
//! - `ErasedHandler` — the `Box<dyn _>` layer RK-012 says the router needs
//! - `Repo<D, R, M>` — the capability handle, in the shape
//!   `docs/specs/capability-system.md:190` specifies, plus the two candidates
//!   #39 weighs
//! - `Ctx::when` — the conditional scope, in three signatures, because ledger
//!   path 8 names one mechanism and desk analysis suggests a different one
//! - `Ctx::spawn` — the checked alternative `api-surface.md:525` promises
//!
//! What is NOT modelled, so the README does not have to claim it was:
//!
//! - **sealing.** `Endpoint` is implementable by `app` here. Seals are #6's
//!   subject and are already verified in `crates/verum/src/sealed.rs`; making
//!   `app` go through a derive would add a proc-macro crate to a spike whose
//!   question is about lifetimes.
//! - **field-granular capability checking.** `Repo::set_email` takes no `Has`
//!   bound. Whether such a bound is enforceable is #15; what matters here is
//!   the *lifetime* of the handle that carries it.
//! - **error contract, request extraction, view generation.** All three are on
//!   `docs/rules/README.md`'s undecided list. `Error` below is a placeholder and
//!   decides nothing.

// Both of these lints fire on the *subject of the measurement*, so silencing
// them is not a style concession.
//
// `type_complexity`: `Pin<Box<dyn Future<Output = ..> + Send + 'a>>` is the
// erasure layer and one of the two surviving `when` signatures. Hiding it behind
// a type alias would hide exactly what a reader needs to compare against the
// spec's version.
//
// `needless_lifetimes`: the explicit `'req` is what this spike is about. Eliding
// it would make the signatures stop saying what they are here to say.
#![allow(clippy::type_complexity, clippy::needless_lifetimes)]

mod ctx;
mod erase;
mod serve;

pub use ctx::{Ctx, CtxNoSized, JobCtx, Repo, RepoLt, RepoPhantom, Runtime};
pub use erase::{ErasedHandler, Handler, Router};
pub use serve::{Server, get};

use std::fmt;

/// A placeholder. The Error Contract is undecided
/// (`docs/rules/README.md` 現時点で未確定の領域); nothing here should be read as
/// a proposal for it.
#[derive(Debug)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Self(s)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// The endpoint's static description. Unsealed here — see the module docs.
pub trait Endpoint: Sized + Send + Sync + 'static {
    /// Stand-in for `E::Reads`. The cons-list machinery is `crates/verum`'s and
    /// is not what this spike measures.
    type Reads: Send + Sync + 'static;
    /// Stand-in for `E::Mutates`.
    type Mutates: Send + Sync + 'static;
}

/// A `ctx.when::<C, _>` condition. Only its existence matters here.
pub trait Condition {
    fn holds(user: &Domain, req: &Req) -> bool;
}

/// A named job for `ctx.spawn::<Job>`.
pub trait Job: Send + Sync + 'static {}

// ---------------------------------------------------------------------------
// A toy Domain and request, deliberately concrete.
//
// Generic Domain handling is `#[derive(Domain)]`'s job (#13, #34). Making these
// concrete keeps every probe below about the lifetime under test rather than
// about type plumbing.
// ---------------------------------------------------------------------------

/// Fields private, mutated only through a `Repo`. That much of the mutation
/// contract has to hold or `Repo`'s lifetime would not matter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Domain {
    id: u64,
    email: String,
}

impl Domain {
    /// `pub` only so tests can seed the store. In the real design this is what
    /// `#[derive(Domain)]` decides (#34), and #13 measured that the choice is
    /// not free — it is not a recommendation.
    pub fn new(id: u64, email: impl Into<String>) -> Self {
        Self {
            id,
            email: email.into(),
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn email(&self) -> &str {
        &self.email
    }

    pub(crate) fn set_email_raw(&mut self, v: String) {
        self.email = v;
    }
}

/// The request body, already decoded.
#[derive(Debug, Clone)]
pub struct Req {
    pub id: u64,
    pub email: String,
}
