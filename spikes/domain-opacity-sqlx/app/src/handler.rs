//! Stands in for endpoint / service code.
//!
//! **This is the module that matters.** It is ordinary application code, in the
//! same crate as the Domain, holding no capability and touching no repository.
//! `docs/specs/persistence.md` says of the `pub(crate)` Repr:
//!
//! > `Repr` を見られるのは Repository 実装だけであり、それ以外のコードから
//! > Domain 内部にアクセスする経路が存在しない
//!
//! Each probe below is one attempt to falsify that sentence.

#[cfg(any(
    feature = "p2-from-repr",
    feature = "p3-as-repr",
    feature = "p4-direct-field",
    feature = "p9-trait-from-repr",
    feature = "p18-newtype-named-field"
))]
use crate::domain::User;
#[cfg(any(feature = "p2-from-repr", feature = "p9-trait-from-repr"))]
use crate::domain::UserRepr;

/// P2 — construct a `User` with every field chosen freely.
///
/// No capability, no repository, no SQL, no `unsafe`. If this compiles, the
/// mutation contract is bypassable from any handler in the crate, and it is
/// strictly worse than ledger path 2 (`*user = other_user`): that one can only
/// install a value some `find` actually returned, this one invents it.
#[cfg(feature = "p2-from-repr")]
pub fn forge_a_user() -> User {
    User::from_repr(UserRepr {
        id: 1,
        name: "attacker".to_owned(),
        email: "attacker@example.com".to_owned(),
    })
}

/// P3 — read every field regardless of what the contract declared in `reads`.
///
/// Overlaps #15's question (whether capability-checked getters are enough) but is
/// distinct: this route does not go through a getter at all. Recorded, not pursued.
#[cfg(feature = "p3-as-repr")]
pub fn read_every_field(u: &User) -> String {
    let r = u.as_repr();
    format!("{} {} {}", r.id, r.name, r.email)
}

/// P4 — the control. Assignment straight through the newtype's inner field.
///
/// This one **must not compile**. Without it a run in which everything was
/// accepted would be indistinguishable from a run that proved something: opacity
/// itself has to be shown to still reject the thing it exists to reject
/// (ledger path 1, RK-008).
#[cfg(feature = "p4-direct-field")]
pub fn assign_directly(u: &mut User) {
    u.0.email = "attacker@example.com".to_owned();
}

/// P6 — the flat-fields Domain the specs appear to describe, with the `as_repr`
/// signature the specs actually write.
///
/// Must not compile: with flat fields there is no `UserRepr` stored anywhere, so
/// `-> &UserRepr` can only borrow a temporary. This is what forces `User` to be a
/// newtype over its `Repr`, and it is a constraint on the generated code that the
/// specs never state.
#[cfg(feature = "p6-flat-as-repr")]
pub mod flat {
    use crate::domain::UserRepr;

    pub struct FlatUser {
        id: i64,
        name: String,
        email: String,
    }

    impl FlatUser {
        pub(crate) fn as_repr(&self) -> &UserRepr {
            &UserRepr {
                id: self.id,
                name: self.name.clone(),
                email: self.email.clone(),
            }
        }
    }
}

/// P9 — the same forgery through the framework trait instead of the inherent
/// method. A trait method cannot be `pub(crate)`, so if the conversion ever moves
/// behind a `verum` trait it becomes reachable from *every* crate, not one.
#[cfg(feature = "p9-trait-from-repr")]
pub fn forge_via_trait() -> User {
    use fw::DomainRepr;
    User::from_repr(UserRepr {
        id: 9,
        name: "via-trait".to_owned(),
        email: "via-trait@example.com".to_owned(),
    })
}

/// P13 — the companion to P4. Same assignment, but against a newtype whose inner
/// field is `pub(crate)` rather than private.
///
/// This one **must compile**, and that is why the derive has to emit a private
/// field: `pub(crate)` on the inner field reopens ledger path 1 outright, without
/// even going through `from_repr`.
#[cfg(feature = "p13-pub-crate-inner")]
pub fn assign_through_pub_crate_inner(u: &mut crate::domain::LooseUser) {
    u.0.email = "attacker@example.com".to_owned();
}

/// P18 — the form the specs actually quote (`user.email = v`), which had **no
/// probe at all** while four documents attributed `E0616` to it.
///
/// Measured: on this newtype the error is **`E0615`** — "attempted to take value of
/// method `email`" — because `User` has a getter of that name, which the real design
/// always will. Without the getter it is `E0609` ("no field"). `E0616` ("field is
/// private"), the code four documents attributed to this line, occurs only for a
/// flat struct with a private *named* field, accessed from outside its module.
///
/// The distinction is not pedantry: `E0615` says nothing about contracts or
/// capabilities, and neither `E0615` nor `E0609` can be reworded through
/// `#[diagnostic::…]`, so the guidance `E0616` would have carried is unavailable.
#[cfg(feature = "p18-newtype-named-field")]
pub fn assign_named_field(u: &mut User) {
    u.email = "attacker@example.com".to_owned();
}

/// P20 — `Debug` / `Clone` on the `Repr` reopen ledger paths 4 and 3 from ordinary
/// handler code, through a type the specs call internal.
#[cfg(feature = "p20-repr-debug-clone")]
pub fn leak_and_own(u: &crate::domain::LeakyUser) -> (String, crate::domain::LeakyRepr) {
    let r = u.as_repr();
    (format!("{r:?}"), r.clone())
}

/// P16 — can a `derive` emit the newtype the specs describe? Must not compile.
#[cfg(feature = "p16-derive-newtype")]
#[derive(mac::DomainNewtype)]
pub struct DerivedUser {
    email: String,
}

/// P17 — control for P16. The same macro emits a `Repr` **and a newtype**, under a
/// name that does not collide with the input.
///
/// The assertion below has to live here, at the call site, not inside the macro's
/// output: an earlier version put it in the expansion, so emptying the macro deleted
/// the assertion along with the thing it was checking and the suite stayed at 21/0
/// — measured. Here, emptying the macro is `E0412`.
///
/// What P16 and P17 together establish is narrower than "a derive cannot emit a
/// newtype": a derive *can* (`ReprOnlyUserWrapper`), and cannot emit one named after
/// its input, because that name is already taken.
#[cfg(feature = "p17-derive-repr-only")]
#[derive(mac::DomainReprOnly)]
pub struct ReprOnlyUser {
    email: String,
}

#[cfg(feature = "p17-derive-repr-only")]
const _: fn(crate::handler::ReprOnlyUserRepr) -> crate::handler::ReprOnlyUserWrapper =
    crate::handler::ReprOnlyUserWrapper;

// ---------------------------------------------------------------------------
// #33 — the candidate gates, and the mechanism that survives them.
//
// There is deliberately no probe here for "a generated repository is present and
// the handler still forges". **P2 above already is that probe**: `app/src/repo.rs`
// exists and is the repository, and P2 forges anyway. Generating the repository
// changes who *should* call `from_repr`; it does not change who *can*.
// ---------------------------------------------------------------------------

/// P22 — the token gate, called without a token.
///
/// Must not compile. But note *how* it fails: `E0061`, "this function takes 2
/// arguments but 1 was supplied". That is an arity error. It carries no wording
/// Verum wrote and says nothing about contracts, which is the first half of why
/// #33's requirement 2 (`E0277`) is not reachable by this route.
#[cfg(feature = "p22-token-missing")]
pub fn forge_without_token() -> crate::domain::User {
    crate::domain::User::from_repr_tokened(crate::domain::UserRepr {
        id: 1,
        name: "attacker".to_owned(),
        email: "attacker@example.com".to_owned(),
    })
}

/// P23 — the second half: the token is **stealable**.
///
/// `fw::run_repository` mints a token and hands it to a `TokenRepository`. The
/// user writes their own impl, receives the token, and forges with it. This must
/// compile, and it is why no value-passing gate can work: the token has to reach
/// user code through a trait the user can implement, because that is the same
/// trait `#[derive(Domain)]` implements.
#[cfg(feature = "p23-token-stolen")]
pub struct ThiefRepo;

#[cfg(feature = "p23-token-stolen")]
impl fw::TokenRepository for ThiefRepo {
    type Domain = crate::domain::User;
    fn load(&self, t: fw::RepoToken) -> Self::Domain {
        crate::domain::User::from_repr_tokened(
            crate::domain::UserRepr {
                id: 1,
                name: "attacker".to_owned(),
                email: "attacker@example.com".to_owned(),
            },
            t,
        )
    }
}

#[cfg(feature = "p23-token-stolen")]
pub fn forge_with_stolen_token() -> crate::domain::User {
    fw::run_repository(&ThiefRepo)
}

/// P24 — the bound gate, forged from the application crate.
///
/// `from_repr_proved<P: fw::RepositoryProof>` looks like it closes the hole and
/// would reject with `E0277` carrying Verum's `on_unimplemented` wording. It does
/// not, because nothing stops the handler supplying its own proof: implementing a
/// **foreign trait for a local type** is what the orphan rules exist to permit.
/// This must compile.
#[cfg(feature = "p24-proof-forged")]
pub struct MyProof;

#[cfg(feature = "p24-proof-forged")]
impl fw::RepositoryProof for MyProof {}

#[cfg(feature = "p24-proof-forged")]
pub fn forge_with_own_proof() -> crate::domain::User {
    crate::domain::User::from_repr_proved(
        crate::domain::UserRepr {
            id: 1,
            name: "attacker".to_owned(),
            email: "attacker@example.com".to_owned(),
        },
        MyProof,
    )
}

/// P26 — **the mechanism.** The same forgery against a constructor that carries
/// no visibility modifier, from a module that is not the domain's own.
///
/// Must not compile. `AccountRepr` is `pub(crate)` precisely so that this probe
/// fails on the *constructor* and not on the type name — otherwise it would be
/// measuring `E0603` and proving nothing about `from_repr`.
#[cfg(feature = "p26-confined-handler")]
pub fn forge_confined() -> crate::confined::Account {
    crate::confined::Account::from_repr(crate::confined::AccountRepr {
        id: 1,
        name: "attacker".to_owned(),
        email: "attacker@example.com".to_owned(),
    })
}

/// P30 — constraint 2. With the `Repr` module-private, the handler cannot even
/// name it, so `Debug` / `Clone` on it cannot leak (contrast P20).
#[cfg(feature = "p30-secret-repr-hidden")]
pub fn leak_secret_repr() -> Option<crate::confined::SecretRepr> {
    // Naming the type is already enough to fail. Kept to exactly that: an earlier
    // version also called a `From` that does not exist, which added an `E0308` and
    // would have let the probe pass on an error unrelated to visibility.
    None
}

// P31 / P34 / P35 live in `nested.rs` itself, not here. The point of P31 is a
// helper written *beside the user's own domain declaration* — under the nested
// mechanism that is `nested.rs`, outside the generated module. Probing from
// `handler.rs` would fail on the private module name (`E0603`) and measure
// nothing about the constructor.

/// P33 — the crate-root hole in the FLAT mechanism. Must compile; that is the point.
#[cfg(feature = "p33-root-flat")]
pub fn forge_root_flat() -> crate::RootUser {
    crate::RootUser::from_repr(crate::RootUserRepr {
        email: "attacker@example.com".to_owned(),
    })
}

/// P32 — the same layout under the nested mechanism. Must be rejected.
#[cfg(feature = "p32-root-nested")]
pub fn forge_root_nested() -> crate::RootNested {
    crate::RootNested::from_repr(crate::__verum_rootuser::RootNestedRepr {
        email: "attacker@example.com".to_owned(),
    })
}

/// P36 — the framework-trait form defeats the confinement entirely.
///
/// Trait-method visibility is the **trait's**, not the impl module's. So the
/// moment the conversion moves onto `fw::DomainRepr` — which this spike's own P9
/// and P14 already do, and which a generic runtime would need — every wall above
/// evaporates. This must compile, and it is why ADR-0010 has to state that the
/// conversion may not sit on a public trait.
#[cfg(feature = "p36-trait-defeats")]
pub fn forge_via_public_trait() -> crate::confined::TraitAccount {
    use fw::DomainRepr;
    crate::confined::TraitAccount::from_repr(crate::confined::trait_repr::TraitAccountRepr {
        email: "attacker@example.com".to_owned(),
    })
}

/// P37 — the bound gate DOES render Verum-authored wording.
///
/// Round 1 asserted `E0277` with `on_unimplemented` was "unreachable". Review
/// refuted that by compiling it. It is reachable and it renders; what it cannot
/// do is *close* the hole (P24 / P25 supply the two-line bypass). Measuring the
/// wording keeps the ADR's Option C honest — it is rejected for being
/// unenforceable, not for being unavailable.
#[cfg(feature = "p37-proof-wording")]
pub fn forge_without_proof() -> crate::domain::User {
    struct NoProof;
    crate::domain::User::from_repr_proved(
        crate::domain::UserRepr {
            id: 1,
            name: "attacker".to_owned(),
            email: "attacker@example.com".to_owned(),
        },
        NoProof,
    )
}

// --- vacuity pins (rule 13). Review gutted each of these bodies to
// `unimplemented!()` and the probes stayed green. The artifact each probe claims
// to establish is a *signature*, so pin the signature at the call site.
#[cfg(feature = "p2-from-repr")]
const _: fn(UserRepr) -> User = User::from_repr;
#[cfg(feature = "p23-token-stolen")]
const _: fn(&ThiefRepo, fw::RepoToken) -> crate::domain::User =
    <ThiefRepo as fw::TokenRepository>::load;
#[cfg(feature = "p24-proof-forged")]
const _: fn(crate::domain::UserRepr, MyProof) -> crate::domain::User =
    crate::domain::User::from_repr_proved;
