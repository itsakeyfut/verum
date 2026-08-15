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
