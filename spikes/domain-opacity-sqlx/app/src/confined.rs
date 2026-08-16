//! #33 — the mechanism that closes ledger path 21, written out as the derive
//! would emit it.
//!
//! **The difference from `domain.rs` is two characters and one item.** The
//! constructor loses its `pub(crate)`, and the repository is emitted *here*
//! instead of in `repo.rs`. That is the whole mechanism; no token, no trait, no
//! seal.
//!
//! `docs/specs/unverified-boundaries.md` ruled this out on a premise that does
//! not hold — it says the derive "cannot emit `pub(in ...)` because it does not
//! know which module the repository is written in". True, and beside the point:
//! the derive does not need `pub(in ...)`. It needs **no modifier at all**, which
//! confines the constructor to the module the domain is declared in, and it needs
//! to put the repository in that module so something can still call it.
//!
//! What this buys and what it does not:
//!
//! | Caller | Probe | Outcome |
//! |---|---|---|
//! | The generated repository, beside the domain | P28 | loads — the design works |
//! | A handler elsewhere in the same crate | P26 | rejected |
//! | A foreign crate | P27 | rejected |
//! | A helper written **next to `struct Account`** | P29 | **compiles — the residue** |
//!
//! P29 is why this lands in the ledger as *narrowed*, not *closed*. The forgery
//! surface shrinks from the whole crate to one module, and the user's own
//! `struct Account` shares that module with the expansion.

/// The opaque Domain. Same shape as `domain::User`.
pub struct Account(AccountRepr);

/// `pub(crate)`, exactly as `domain.rs` has it.
///
/// Deliberate: it isolates what P26 measures. With the `Repr` module-private too
/// the handler would fail on the *type name* before it ever reached the
/// constructor, and the probe would be measuring the wrong thing. `Secret` below
/// is the variant that tightens this.
// `name` is never read by a probe, but the column is in the SELECT and the Repr
// must match it — so the field is live for sqlx and dead for rustc.
#[allow(dead_code)]
#[derive(sqlx::FromRow)]
pub(crate) struct AccountRepr {
    pub id: i64,
    pub name: String,
    pub email: String,
}

impl fw::Domain for Account {}

impl Account {
    /// **No visibility modifier.** This is the entire change.
    ///
    /// `pub(crate)` meant "every handler in the application"; bare means "this
    /// module and its children". The repository below is in this module, so it
    /// still compiles; `handler.rs` is not, so P26 does not.
    fn from_repr(r: AccountRepr) -> Self {
        Self(r)
    }

    pub fn id(&self) -> i64 {
        self.0.id
    }
    pub fn email(&self) -> &str {
        &self.0.email
    }
}

/// Emitted **into the domain's module**, which is the half that makes privacy
/// usable rather than merely restrictive.
///
/// ARK-002: blocking a path without a checked alternative pushes people onto
/// unchecked routes. The alternative is this — it is generated, so the user never
/// has to reach for `from_repr` themselves.
pub struct AccountRepository;

impl AccountRepository {
    pub async fn find(&self, pool: &sqlx::SqlitePool, id: i64) -> Result<Account, sqlx::Error> {
        let r = sqlx::query_as::<_, AccountRepr>("SELECT id, name, email FROM users WHERE id = ?")
            .bind(id)
            .fetch_one(pool)
            .await?;
        Ok(Account::from_repr(r))
    }
}

/// P29 — **the residue, measured rather than conceded in prose.**
///
/// A helper the user writes beside their own `struct Account` is inside the
/// confinement, so it forges exactly as `handler.rs` used to. This must compile.
/// Without it the probe table would show two rejections and read as "closed",
/// which is the shape that got #15's verdict refuted three separate ways.
///
/// It is also the shortest way around a P26 rejection — "move that code into the
/// domain file" — so it is ARK-002's pattern showing up inside the fix for
/// ARK-002's pattern.
#[cfg(feature = "p29-same-module-forge")]
pub fn forge_from_the_domains_own_module() -> Account {
    Account::from_repr(AccountRepr {
        id: 1,
        name: "attacker".to_owned(),
        email: "attacker@example.com".to_owned(),
    })
}

/// P30 — constraint 2: the `Repr` must not become a second forgery surface.
///
/// `AccountRepr` above is `pub(crate)`, so deriving `Debug` / `Clone` on it would
/// reopen ledger paths 4 and 3 from any handler, exactly as P20 measures for
/// `domain::LeakyRepr`. Making the `Repr` **module-private** as well takes both
/// with it: the name is unreachable outside this module, so there is nothing to
/// call `Debug` on.
///
/// This is the shape the derive should actually emit. `AccountRepr` stays
/// `pub(crate)` only so P26 can isolate the constructor.
#[allow(dead_code)]
#[derive(Debug, Clone, sqlx::FromRow)]
struct SecretRepr {
    pub id: i64,
    pub name: String,
    pub email: String,
}

pub struct Secret(SecretRepr);

impl Secret {
    fn from_repr(r: SecretRepr) -> Self {
        Self(r)
    }
    pub fn email(&self) -> &str {
        &self.0.email
    }
}

/// The same generated repository, for the tightened shape.
pub struct SecretRepository;

impl SecretRepository {
    pub async fn find(&self, pool: &sqlx::SqlitePool, id: i64) -> Result<Secret, sqlx::Error> {
        let r = sqlx::query_as::<_, SecretRepr>("SELECT id, name, email FROM users WHERE id = ?")
            .bind(id)
            .fetch_one(pool)
            .await?;
        Ok(Secret::from_repr(r))
    }
}

/// P36 — **the constraint ADR-0010 must state, or the decision is void.**
///
/// Everything above relies on `from_repr` being an *inherent* method, whose
/// visibility is its impl block's module. Move the conversion onto a public
/// framework trait — which `fw::DomainRepr` is, and which P9 and P14 in this same
/// spike already use — and the method's visibility becomes **the trait's**. Every
/// wall the mechanism builds evaporates, from this crate and from foreign ones.
///
/// The `Repr` has to be `pub` in a private child module for the trait form to
/// exist at all: binding a public trait's associated type to a `pub(crate)` type
/// is `E0446` (P9 measured that). This is the shape that survives `E0446` and is
/// therefore the one anyone reaching for a generic runtime would write.
#[cfg(feature = "p36-trait-defeats")]
pub mod trait_repr {
    pub struct TraitAccountRepr {
        pub email: String,
    }
}

#[cfg(feature = "p36-trait-defeats")]
pub struct TraitAccount(trait_repr::TraitAccountRepr);

#[cfg(feature = "p36-trait-defeats")]
impl fw::DomainRepr for TraitAccount {
    type Repr = trait_repr::TraitAccountRepr;
    fn from_repr(r: Self::Repr) -> Self {
        Self(r)
    }
    fn as_repr(&self) -> &Self::Repr {
        &self.0
    }
}

/// Pins P29's forgery as a *type-level* fact. Review measured that replacing the
/// body with `unimplemented!()` left the probe green — and P29 is the sole
/// evidence for "narrowed, not closed", so a vacuous green there is the most
/// expensive one in the suite. Same technique as `handler.rs`'s P17 pin.
#[cfg(feature = "p29-same-module-forge")]
const _: fn(AccountRepr) -> Account = Account::from_repr;
