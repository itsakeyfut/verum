//! The Repository implementation — what the specs call the trust boundary.
//!
//! In the same crate as the Domain, which is the single-crate-app reading of
//! `pub(crate)`. `separate-repo/` is the other reading.

use crate::domain::{User, UserRepr};

pub struct SqliteUserRepository {
    pub pool: sqlx::SqlitePool,
}

impl SqliteUserRepository {
    /// P1a — the **compile-time-checked** macro. This is the form the specs use
    /// and the one the issue asks about first. It needs `DATABASE_URL` at compile
    /// time, which is why `run.sh` creates a SQLite file before building.
    ///
    /// `query_as!` does not use `FromRow` at all: it expands into
    /// `UserRepr { id: …, name: …, email: … }` right here, so the fields must be
    /// visible at this call site.
    pub async fn find_macro(&self, id: i64) -> Result<User, sqlx::Error> {
        let repr = sqlx::query_as!(
            UserRepr,
            "SELECT id, name, email FROM users WHERE id = ?",
            id
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(User::from_repr(repr))
    }

    /// P1b — the **runtime-checked** function form. Needs no `DATABASE_URL`; it
    /// goes through `FromRow`, whose derived impl constructs the struct inside
    /// `domain.rs`, so field visibility here is irrelevant.
    pub async fn find_fn(&self, id: i64) -> Result<User, sqlx::Error> {
        let repr = sqlx::query_as::<_, UserRepr>("SELECT id, name, email FROM users WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        Ok(User::from_repr(repr))
    }

    /// P1c — the write direction, through `as_repr`.
    pub async fn save(&self, u: &User) -> Result<(), sqlx::Error> {
        let r = u.as_repr();
        sqlx::query!(
            "UPDATE users SET name = ?, email = ? WHERE id = ?",
            r.name,
            r.email,
            r.id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// P1d — the same `Repr` against a different driver. Type-checked, never
    /// executed, and that is the point: it shows the mechanism under test does not
    /// depend on SQLite, so the verdict is not an artefact of the driver chosen
    /// because it needs no server.
    #[allow(dead_code)]
    fn postgres_path_type_checks() {
        let _ = sqlx::query_as::<sqlx::Postgres, UserRepr>("SELECT id, name, email FROM users");
    }
}

/// P7 — `query_as!` where the Repr's fields are private and the call site is in a
/// different module. Isolates *field* visibility from *struct* visibility.
#[cfg(feature = "p7-private-repr-fields")]
impl SqliteUserRepository {
    pub async fn find_priv(&self, id: i64) -> Result<crate::domain::PrivFieldRepr, sqlx::Error> {
        sqlx::query_as!(
            crate::domain::PrivFieldRepr,
            "SELECT id, name, email FROM users WHERE id = ?",
            id
        )
        .fetch_one(&self.pool)
        .await
    }
}
