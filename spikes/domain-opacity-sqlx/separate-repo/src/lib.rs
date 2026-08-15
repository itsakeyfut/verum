//! The other reading of "the Repository implementation": its own crate.
//!
//! If the Repository lives here, `pub(crate)` in `app` excludes it, so the design
//! does not merely leak — it stops working. P5 is that probe.
//!
//! P10–P12 then measure the issue's third alternative (`Repr` public, fields
//! private), which is the only listed one that could serve a repository here at
//! all: P9 established that the trait-based conversion cannot exist while `Repr`
//! is `pub(crate)`, because binding a public trait's associated type to a
//! crate-private type is `error[E0446]`.

/// P5 — name `app::domain::UserRepr` from outside its crate.
#[cfg(feature = "p5-name-repr")]
pub async fn find(pool: &sqlx::SqlitePool, id: i64) -> Result<app::domain::User, sqlx::Error> {
    let repr = sqlx::query_as::<_, app::domain::UserRepr>(
        "SELECT id, name, email FROM users WHERE id = ?",
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    Ok(app::domain::User::from_repr(repr))
}

/// P10 — with `Repr` public and its fields private, **can a foreign crate load?**
///
/// `FromRow`'s derived impl constructs the struct inside `app`'s module, so no
/// field is ever named here. If this compiles, a repository in its own crate works.
#[cfg(feature = "p10-load")]
pub async fn find_via_pub_repr(
    pool: &sqlx::SqlitePool,
    id: i64,
) -> Result<app::domain::User, sqlx::Error> {
    let repr =
        sqlx::query_as::<_, app::domain::PubRepr>("SELECT id, name, email FROM users WHERE id = ?")
            .bind(id)
            .fetch_one(pool)
            .await?;
    Ok(app::domain::User::from_pub_repr(repr))
}

/// P11 — and can it **forge**? The value must come from a database row, not from a
/// struct literal. This is the half of the boundary that private fields do keep.
#[cfg(feature = "p11-forge-pub-repr")]
pub fn forge() -> app::domain::User {
    app::domain::User::from_pub_repr(app::domain::PubRepr {
        id: 1,
        name: "attacker".to_owned(),
        email: "attacker@example.com".to_owned(),
    })
}

/// P12 — the cost of that alternative. `query_as!` expands into a struct literal at
/// *this* call site, so the compile-time-checked macro form is unavailable to a
/// foreign repository even though the runtime-checked form works.
#[cfg(feature = "p12-macro-pub-repr")]
pub async fn find_via_macro(
    pool: &sqlx::SqlitePool,
    id: i64,
) -> Result<app::domain::PubRepr, sqlx::Error> {
    sqlx::query_as!(
        app::domain::PubRepr,
        "SELECT id, name, email FROM users WHERE id = ?",
        id
    )
    .fetch_one(pool)
    .await
}

/// P14 — the projection route, from a crate that cannot name the type.
///
/// `Repr` is `pub` inside a private module in `app`, so `E0446` never fires and the
/// conversion sits on `fw::DomainRepr`. This crate then denotes the type as
/// `<HiddenUser as DomainRepr>::Repr`, reads every field, and builds one with a
/// struct literal. If this compiles, `pub(crate)` was doing the work — not the
/// trait, and not module privacy.
#[cfg(feature = "p14-projection")]
pub fn projection_reads_and_forges() -> app::domain::HiddenUser {
    use fw::DomainRepr;
    type Repr = <app::domain::HiddenUser as DomainRepr>::Repr;

    let forged = app::domain::HiddenUser::from_repr(Repr {
        id: 1,
        name: "attacker".to_owned(),
        email: "attacker@example.com".to_owned(),
    });
    let r: &Repr = forged.as_repr();
    let _every_field = format!("{} {} {}", r.id, r.name, r.email);
    forged
}

/// Pins the projection and the conversion as types, so the probe cannot be gutted
/// into a no-op that still reports "as specified".
#[cfg(feature = "p14-projection")]
const _: fn(<app::domain::HiddenUser as fw::DomainRepr>::Repr) -> app::domain::HiddenUser =
    <app::domain::HiddenUser as fw::DomainRepr>::from_repr;
