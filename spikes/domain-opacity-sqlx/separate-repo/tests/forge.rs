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

/// P41 — the same route, second instance, and the cheaper one.
///
/// `#35` asks for the forgery-route principle to be stated with **both** measured
/// instances as evidence. `serde::Deserialize` was named alongside `FromRow` in
/// #13's report and in four documents' enumerations, but it had never been
/// compiled — so "both measured" was not yet true. This makes it true.
///
/// It is worth having as its own row rather than assuming it follows from P15:
/// `FromRow` at least needs a live connection to hand the row over, so P15's
/// forgery carries the cost of standing up sqlite. `Deserialize` needs a string
/// literal. Same mechanism, strictly lower bar.
///
/// Note what is NOT named here: not one field of `PubRepr`. They are private
/// (`app/src/domain.rs`), and `serde`'s generated impl reads them from inside
/// `app`'s own module. Field privacy is not the lever — the type's name is.
#[cfg(feature = "p41-forge-via-json")]
#[test]
fn a_foreign_crate_should_forge_a_domain_from_a_json_string() {
    let repr: app::domain::PubRepr =
        serde_json::from_str(r#"{"id":1,"name":"attacker","email":"attacker@example.com"}"#)
            .expect("attacker-authored json");

    let u = app::domain::User::from_pub_repr(repr);
    assert_eq!(u.id(), 1);
    assert_eq!(u.name(), "attacker");
    assert_eq!(u.email(), "attacker@example.com");
}
