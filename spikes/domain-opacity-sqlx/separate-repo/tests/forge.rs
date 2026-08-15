//! P15 — the probe that replaces a wrong conclusion.
//!
//! `README.md` originally reported that "`Repr` public, fields private" **cannot be
//! forged**, on the strength of P11's `E0451`. P11 only shows that a *struct
//! literal* is rejected. `FromRow`'s derived impl builds the struct inside `app`'s
//! own module, and the row it builds from is supplied by the caller — so a foreign
//! crate forges by handing over a row it wrote itself.
//!
//! No table, no access to the application's database, no `unsafe`.

#[cfg(feature = "p15-forge-via-select")]
#[tokio::test]
async fn a_foreign_crate_should_forge_a_domain_from_literal_select_columns() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite");

    let repr = sqlx::query_as::<_, app::domain::PubRepr>(
        "SELECT 1 AS id, 'attacker' AS name, 'attacker@example.com' AS email",
    )
    .fetch_one(&pool)
    .await
    .expect("literal row");

    let u = app::domain::User::from_pub_repr(repr);
    assert_eq!(u.id(), 1);
    assert_eq!(u.name(), "attacker");
    assert_eq!(u.email(), "attacker@example.com");
}
