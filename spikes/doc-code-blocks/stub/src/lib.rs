//! The designed-but-unimplemented surface the docs' code blocks refer to.
//!
//! **Every item here cites the document line it is copied from.** That rule is
//! the whole point: #14 failed because a signature was re-derived from a reading
//! of the specs rather than transcribed from them, and the re-derivation was
//! subtly different. If a block fails to compile against this stub, the fix is
//! either the block or the spec — never a quiet adjustment here.
//!
//! Where a name is used by the docs but **never declared** in them, it is marked
//! `UNDECLARED` and that is itself a finding for #43.

#![allow(missing_docs, dead_code, unused_variables, clippy::all)]

use std::marker::PhantomData;

pub use verum::{Append, ConsList, Has, Here, Includes, Index, Lookup, There};

/// `docs/specs/rust-type-model.md:48`. The seal is dropped: `SealedEndpoint`
/// does not exist yet (it arrives with the derive in M2), and reproducing it
/// here would make every block in the docs unimplementable rather than checked.
pub trait Endpoint {
    type Method;
    type Domain;
    type Request;
    type Response;
    type Reads;
    type Mutates;
    type Creates;
    type Deletes;
    const PATH: &'static str;
}

/// `docs/rules/api-surface.md:519` — `pub struct Ctx<'req, E> { /* ... */ }`.
///
/// The field set is **not specified anywhere**; `api-surface.md:482` shows
/// `{ pub repos: RepoRegistry, ... }` in a ❌ example only. UNDECLARED.
pub struct Ctx<'req, E: ?Sized>(PhantomData<(&'req (), fn() -> E)>);

/// `docs/specs/rust-type-model.md:329`. As written the trait names
/// `Self::Request` / `Self::Response` without a supertrait that provides them —
/// it does not compile as printed. Given `Endpoint` here so the *blocks* can be
/// checked; the spec line is a #43 finding.
pub trait Handler: Endpoint {
    fn handle(
        &self,
        req: Self::Request,
        ctx: Ctx<'_, Self>,
    ) -> impl Future<Output = Result<Self::Response>> + Send;
}

/// `docs/specs/conditional-effects.md:258`, seal dropped as above.
pub trait Condition<Domain, Request> {
    const NAME: &'static str;
    fn holds(domain: &Domain, req: &Request) -> bool;
}

/// Used at `docs/specs/capability-system.md:187,195` as `Repo<D, R, M>` and
/// **never declared anywhere in `docs/`**. UNDECLARED — #43 finding.
pub struct Repo<D, R, M>(PhantomData<fn() -> (D, R, M)>);

/// `docs/specs/capability-system.md:70` takes `&'req Runtime<Sealed>`.
/// `Runtime` itself is never declared. UNDECLARED — #43 finding.
pub struct Runtime<S = ()>(PhantomData<fn() -> S>);
pub struct Sealed;

pub type Result<T, E = Error> = core::result::Result<T, E>;
#[derive(Debug)]
pub struct Error;

// --- Effect markers, from docs/specs/effect-system.md ------------------------
pub struct Read<D, F>(PhantomData<fn() -> (D, F)>);
pub struct Mutate<D, F>(PhantomData<fn() -> (D, F)>);
pub struct Create<D>(PhantomData<fn() -> D>);
pub struct Delete<D>(PhantomData<fn() -> D>);
pub struct Emit<E>(PhantomData<fn() -> E>);
pub struct Call<S>(PhantomData<fn() -> S>);
pub struct Spawn<J>(PhantomData<fn() -> J>);

// --- HTTP method markers, from docs/specs/rust-type-model.md:49 --------------
pub struct Get;
pub struct Head;
pub struct Post;
pub struct Put;
pub struct Patch;
pub struct Delete_;

// --- The sample domain the docs use throughout ------------------------------
// `User` appears in 29 blocks, `Email` in 13. They are the running example, not
// part of the framework.
pub struct User;
pub struct Order;
pub struct AuditLog;

pub mod user {
    pub struct Id;
    pub struct Name;
    pub struct Email;
    pub struct Status;
    pub struct PasswordHash;
}
pub use user::{Email, Name, Status};
pub struct UserId(pub u64);

pub struct UpdateUser;
pub struct GetUser;
pub struct DeleteUser;
pub struct UpdateUserRequest;
pub struct UserView;
pub struct EmailChanged;
pub struct IsPaid;
pub struct UserUpdated;
pub struct Context;
pub struct Req;
pub struct Res;

/// `docs/rules/type-level.md:331` uses `ReadOnly` as a **trait**
/// (`trait ReadOnly: Endpoint<Mutates = (), ...>`), not a marker struct. The
/// first version of this stub guessed struct and produced three `E0404`s that
/// looked like documentation defects. They were stub defects.
pub trait ReadOnly: Endpoint<Mutates = (), Creates = (), Deletes = ()> {}

/// `docs/specs/effect-system.md:125` and `docs/specs/rust-type-model.md:98`
/// both assert `GetUser: ReadOnly`. Making the sample endpoint satisfy it is
/// what lets those blocks be checked rather than assumed.
impl Endpoint for GetUser {
    type Method = Get;
    type Domain = (User, ());
    type Request = Req;
    type Response = Res;
    type Reads = (Read<User, Email>, ());
    type Mutates = ();
    type Creates = ();
    type Deletes = ();
    const PATH: &'static str = "/users/{id}";
}
impl ReadOnly for GetUser {}

/// `docs/specs/mutation-contract.md:170` names `Field` as a sealed trait.
/// Declared nowhere in `docs/` as a signature — UNDECLARED, #43 finding.
pub trait Field {
    const NAME: &'static str;
}

/// `docs/specs/conditional-effects.md:76` uses `When<C, ..>` to carry a
/// conditional effect set. Its parameter list is described in prose at
/// `:107-115` and never written as a declaration — UNDECLARED, #43 finding.
pub struct When<C, M = (), E = (), Ca = ()>(PhantomData<fn() -> (C, M, E, Ca)>);
