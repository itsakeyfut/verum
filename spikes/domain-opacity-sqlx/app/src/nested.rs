//! #33 (review round 2) — the mechanism that **dominates** `confined.rs`.
//!
//! `confined.rs` puts the constructor and the repository in *the domain's own
//! module*. Review found two holes in that, both compile-verified:
//!
//!   * a helper the user writes beside their own `struct User` forges (P29), and
//!   * if the domain is declared at the **crate root**, "no modifier" *is*
//!     `pub(crate)` and the mechanism buys nothing at all (P33).
//!
//! Both disappear if the derive emits its items into a **nested private module**
//! of its own rather than into whatever module the user put the domain in:
//!
//! ```text
//! mod __verum_account {          // generated, private, derive-owned
//!     pub struct Account(AccountRepr);
//!     struct AccountRepr { .. }  // module-private: paths 3/4 shut with it
//!     impl Account { fn from_repr(..) }   // no modifier
//!     pub struct AccountRepository;       // the only legitimate caller, inside
//! }
//! pub use __verum_account::{Account, AccountRepository};
//! ```
//!
//! The confinement radius stops being "wherever the user happened to put the
//! domain" and becomes a scope the derive controls. That is the same property
//! RK-016 asks of a guard — **it must not depend on placement** — applied to a
//! type-level mechanism instead of a scanning guard.
//!
//! | Caller | Probe | `confined.rs` (option D) | here |
//! |---|---|---|---|
//! | the generated repository | P28 / P37 | works | works |
//! | a handler in another module | P26 / P31 | `E0624` | `E0624` |
//! | a helper **beside the user's struct** | P29 / P31 | **compiles** | **`E0624`** |
//! | the domain declared at the **crate root** | P33 / P32 | **compiles** | **`E0624`** |
//! | `as_repr`, the read half | not probed / P34 | — | **`E0624`** |
//!
//! **What it costs, and it is not nothing.** Everything that touches the `Repr`
//! must be generated. A user-written `impl UserRepository for PgUserRepository`
//! — which `docs/specs/persistence.md` still shows — cannot reach `from_repr`
//! from outside. Whether that trade is acceptable is a persistence-API decision
//! (#39 / #40), not this spike's; what this spike settles is that the trade
//! exists and that option D is dominated on containment.

/// The derive-owned module. Nothing here is written by the user.
mod __verum_account {
    /// The opaque Domain. Re-exported below, so users still write `Account`.
    pub struct Account(AccountRepr);

    /// Module-private, so ledger paths 3 and 4 cannot reach it either — P35
    /// measures that with an actual `Debug` call rather than by naming it.
    #[derive(Debug, Clone, sqlx::FromRow)]
    struct AccountRepr {
        pub id: i64,
        pub name: String,
        pub email: String,
    }

    impl Account {
        /// No visibility modifier. Confined to `__verum_account`, which the user
        /// cannot add code to — unlike the domain's own module.
        fn from_repr(r: AccountRepr) -> Self {
            Self(r)
        }

        /// The read half gets the same treatment. Ledger path 21 names
        /// `as_repr()` alongside `from_repr`, and the first round measured only
        /// the constructor — the claim covered both.
        fn as_repr(&self) -> &AccountRepr {
            &self.0
        }

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

    /// Generated into the same private module — the only legitimate caller.
    pub struct AccountRepository;

    impl AccountRepository {
        pub async fn find(
            &self,
            pool: &sqlx::SqlitePool,
            id: i64,
        ) -> Result<Account, sqlx::Error> {
            let r = sqlx::query_as::<_, AccountRepr>(
                "SELECT id, name, email FROM users WHERE id = ?",
            )
            .bind(id)
            .fetch_one(pool)
            .await?;
            Ok(Account::from_repr(r))
        }

        /// Exercises the read half from inside, so P34's rejection is measured
        /// against a call that genuinely works rather than one that never could.
        pub fn debug_repr(&self, a: &Account) -> String {
            format!("{:?}", a.as_repr().clone())
        }
    }
}

pub use __verum_account::{Account, AccountRepository};

/// P31 — **the row that decides between the two mechanisms.**
///
/// This is P29's shape: a helper the user writes beside their own domain
/// declaration. Under `confined.rs` it compiles and is the residue that kept
/// ledger path 21 open. Here it must be `E0624`.
#[cfg(feature = "p31-nested-user-helper")]
pub fn user_helper_beside_the_domain() -> Account {
    Account::from_repr(__verum_account::AccountRepr {
        id: 1,
        name: "attacker".to_owned(),
        email: "attacker@example.com".to_owned(),
    })
}

/// P34 — the read half, from outside the generated module.
#[cfg(feature = "p34-nested-as-repr")]
pub fn read_every_field(a: &Account) -> String {
    format!("{:?}", a.as_repr())
}

/// P35 — constraint 2, measured properly this time.
///
/// The first round's P30 only *named* the `Repr` and was found (in review) to be
/// completely insensitive to the `Debug` / `Clone` derives it was cited as
/// neutralising: removing them left it green. `Debug` needs a **value**, not a
/// name. So this probe obtains a value the only way a handler could — through the
/// public API — and tries to format it.
#[cfg(feature = "p35-nested-repr-debug")]
pub fn leak_through_repr(a: &Account) -> String {
    format!("{:?}", a.as_repr().clone())
}
