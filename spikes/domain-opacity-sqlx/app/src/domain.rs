//! What `#[derive(Domain)]` would generate, written out by hand.
//!
//! Shape taken from `docs/specs/persistence.md` §Interoperating with domain opacity and
//! `docs/specs/mutation-contract.md` §Decision: a domain is exposed as an opaque type.

/// The opaque Domain.
///
/// **A newtype over its `Repr`, not a struct with flat fields.** The specs write
/// `as_repr(&self) -> &UserRepr`, and there is nothing to borrow unless `User`
/// actually contains a `UserRepr` — P6 is the probe that pins this down. So the
/// spec's own signature already forces this shape; it just never said so.
///
/// The inner field is fully private, not `pub(crate)`. `as_repr` below reaches it
/// because an inherent impl in the same module can, and making it `pub(crate)`
/// would add a *second*, more direct bypass (`user.0.email = v`) on top of the
/// one this spike is measuring.
pub struct User(UserRepr);

/// The Repr. `pub(crate)` per the specs, with `pub` fields.
///
/// Both halves are load-bearing and for different reasons — P1 and P7 separate
/// them. `query_as!` expands into a **struct literal at the call site**, so what
/// it needs is the *fields* visible there; the struct's own visibility only
/// governs whether the name can be written.
// No `Clone`, no `Debug`: P20 measures what deriving them costs — ledger
// path 3 (`into_owned`) and path 4 (`Debug` leak) reopen through the `Repr`,
// which nothing in the specs constrains.
#[derive(sqlx::FromRow)]
pub(crate) struct UserRepr {
    pub id: i64,
    pub name: String,
    pub email: String,
}

impl fw::Domain for User {}

impl User {
    pub(crate) fn from_repr(r: UserRepr) -> Self {
        Self(r)
    }

    pub(crate) fn as_repr(&self) -> &UserRepr {
        &self.0
    }

    // In the real design these carry `where R: Has<Read<User, F>, I>`; #15 asks
    // whether that alone is enough to enforce `reads`. Plain here — this spike is
    // about the Repr boundary, not about read enforcement.
    pub fn id(&self) -> i64 {
        self.0.id
    }
    pub fn name(&self) -> &str {
        &self.0.name
    }
    pub fn email(&self) -> &str {
        &self.0.email
    }
}

/// P9 — the conversion as a framework trait rather than an inherent method.
///
/// Note what the impl itself already shows: a public trait's associated type
/// bound to a `pub(crate)` type. Whether rustc accepts that at all is part of
/// what P9 measures.
#[cfg(feature = "p9-trait-from-repr")]
impl fw::DomainRepr for User {
    type Repr = UserRepr;
    fn from_repr(r: UserRepr) -> Self {
        Self(r)
    }
    fn as_repr(&self) -> &UserRepr {
        &self.0
    }
}

/// P7 — a Repr whose fields are private, used by `query_as!` from another module.
#[cfg(feature = "p7-private-repr-fields")]
#[derive(sqlx::FromRow)]
pub(crate) struct PrivFieldRepr {
    id: i64,
    name: String,
    email: String,
}

/// P13 — the same newtype with a `pub(crate)` inner field instead of a private one.
///
/// Here so the recommendation "the derive must emit a **private** inner field" is
/// measured rather than asserted: if `pub(crate)` were used, `u.0.email = v` would
/// compile from anywhere in the crate, which is ledger path 1 reopened directly
/// rather than through `from_repr`.
#[cfg(feature = "p13-pub-crate-inner")]
pub struct LooseUser(pub(crate) UserRepr);

/// P10–P12 — the issue's third alternative: **`Repr` public, fields private.**
///
/// Reached for after P9 showed the trait-based conversion cannot exist at all
/// while `Repr` is `pub(crate)` (E0446). This shape is the one that has a chance
/// of serving a repository in its own crate, so what it does and does not protect
/// is worth measuring precisely rather than assuming.
///
/// `#[derive(FromRow)]` constructs the struct inside *this* module, so a foreign
/// crate can obtain a value without ever seeing a field.
///
/// P41 adds `Deserialize` under its own feature. It is the **second** instance of
/// the same route (`api-surface.md` §8) and it had never been compiled — #13
/// asserted it in prose. It is also the cheaper of the two: `FromRow` needs a
/// database connection to hand the row over, `Deserialize` needs a string.
#[cfg(feature = "p10-pub-repr")]
#[derive(sqlx::FromRow)]
#[cfg_attr(feature = "p41-repr-deserialize", derive(serde::Deserialize))]
pub struct PubRepr {
    id: i64,
    name: String,
    email: String,
}

#[cfg(feature = "p10-pub-repr")]
impl User {
    /// Public, because a repository in another crate has to be able to call it.
    pub fn from_pub_repr(r: PubRepr) -> Self {
        Self(UserRepr {
            id: r.id,
            name: r.name,
            email: r.email,
        })
    }
}

/// P14 — the alternative `README.md` had dismissed **without probing**: `Repr` is
/// `pub` but lives in a private module, so its name is unreachable from outside.
///
/// The point of measuring it: with the `Repr` type `pub`, `E0446` does not fire, so
/// the conversion *can* sit on a framework trait — and then a foreign crate can
/// denote the type by projection (`<HiddenUser as DomainRepr>::Repr`) without ever
/// naming it. `fw/src/lib.rs` predicted exactly this; the first version of this
/// spike's README overrode that prediction with the `E0446` observation.
#[cfg(feature = "p14-hidden-module-repr")]
mod hidden {
    pub struct HiddenRepr {
        pub id: i64,
        pub name: String,
        pub email: String,
    }
}

#[cfg(feature = "p14-hidden-module-repr")]
pub struct HiddenUser(hidden::HiddenRepr);

#[cfg(feature = "p14-hidden-module-repr")]
impl fw::DomainRepr for HiddenUser {
    type Repr = hidden::HiddenRepr;
    fn from_repr(r: Self::Repr) -> Self {
        Self(r)
    }
    fn as_repr(&self) -> &Self::Repr {
        &self.0
    }
}

/// P19 — a **flat** Domain that still satisfies `as_repr(&self) -> &UserRepr`, by
/// owning a cached `Repr` rather than being a newtype over one.
///
/// Here because the claim "the spec's own signature already forced the newtype"
/// was an over-statement: what the signature forces is *owning something
/// borrowable*, and a newtype is one way to do that, not the only one. No
/// `unsafe`, no interior mutability. The newtype is still the better design — this
/// duplicates state and can desynchronise — but that is a design judgement, not a
/// thing rustc rejects, and the specs stated it as the latter.
#[cfg(feature = "p19-flat-cached-repr")]
pub struct CachedUser {
    id: i64,
    cached: UserRepr,
}

#[cfg(feature = "p19-flat-cached-repr")]
impl CachedUser {
    pub(crate) fn as_repr(&self) -> &UserRepr {
        &self.cached
    }
    pub fn id(&self) -> i64 {
        self.id
    }
}

/// The claim *is* the signature, so pin the signature rather than a body.
#[cfg(feature = "p19-flat-cached-repr")]
const _: for<'a> fn(&'a CachedUser) -> &'a UserRepr = CachedUser::as_repr;

/// P20 — `Debug` / `Clone` on the `Repr`, which is what the default shape of this
/// spike originally derived.
///
/// Ledger path 4's remedy is "a derive-generated `Debug` that prints declared
/// fields only", and path 3's is "do not provide `into_owned`". Both are recorded
/// against the *Domain*; nothing constrains the `Repr`, so deriving them there
/// reopens both paths through a type the specs describe as internal.
#[cfg(feature = "p20-repr-debug-clone")]
#[derive(Debug, Clone)]
pub(crate) struct LeakyRepr {
    pub id: i64,
    pub name: String,
    pub email: String,
}

#[cfg(feature = "p20-repr-debug-clone")]
pub struct LeakyUser(LeakyRepr);

#[cfg(feature = "p20-repr-debug-clone")]
impl LeakyUser {
    pub(crate) fn as_repr(&self) -> &LeakyRepr {
        &self.0
    }
}

/// Pins the claim as a *type-level* fact, so gutting a function body cannot make
/// this probe pass quietly. Measured: with the assertion only in a body, replacing
/// that body with `unimplemented!()` left the suite at 21/0.
#[cfg(feature = "p20-repr-debug-clone")]
const _: () = {
    const fn requires_debug_and_clone<T: core::fmt::Debug + Clone>() {}
    requires_debug_and_clone::<LeakyRepr>()
};

/// P4b — the companion P4 was missing. The same assignment P4 rejects, written
/// **inside the module that defines `User`**.
///
/// This must compile, and that is the point: field privacy is a *module* boundary,
/// not a type boundary. `#[derive(Domain)]` expands in the user's own module, so
/// anything the user writes beside their `struct User` — helper `impl`s, a
/// constructor, a mapper — sits on the permissive side of the line P4 measures.
/// It is also the shortest way around an `E0616`: move the code into this file.
#[cfg(feature = "p4b-same-module-assign")]
pub fn assign_from_defining_module(u: &mut User) {
    u.0.email = "attacker@example.com".to_owned();
}

/// P4c — and from a *child* module, which is the same side of the boundary.
#[cfg(feature = "p4b-same-module-assign")]
pub mod child {
    pub fn assign_from_child_module(u: &mut super::User) {
        u.0.email = "attacker@example.com".to_owned();
    }
}

/// P22 / P23 — the constructor gated by a **token value**.
///
/// `pub` rather than `pub(crate)` on purpose: the premise of a token gate is that
/// the token, not the visibility, is what protects the constructor. If it still
/// needed `pub(crate)` to be safe, the token would be doing nothing.
#[cfg(any(feature = "p22-token-missing", feature = "p23-token-stolen"))]
impl User {
    pub fn from_repr_tokened(r: UserRepr, _t: fw::RepoToken) -> Self {
        Self(r)
    }
}

/// P24 / P25 — the constructor gated by a **trait bound**, the only shape whose
/// rejection would be `E0277` and could therefore carry Verum's own wording.
#[cfg(any(feature = "p24-proof-forged", feature = "p37-proof-wording"))]
impl User {
    pub fn from_repr_proved<P: fw::RepositoryProof>(r: UserRepr, _p: P) -> Self {
        Self(r)
    }
}
