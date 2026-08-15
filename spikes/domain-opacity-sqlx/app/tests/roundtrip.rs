//! P8 — does the round trip actually work, or does it only type-check?
//!
//! RK-014's lesson generalised: a compile that succeeds is not evidence the thing
//! runs. `query_as!` verifying SQL at compile time would still leave a `from_repr`
//! that produced a `User` nobody could read back.

use sqlx::sqlite::SqlitePoolOptions;

async fn pool() -> sqlx::SqlitePool {
    // max_connections(1) because each connection to `:memory:` would otherwise get
    // its own empty database and the INSERT would vanish before the SELECT.
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite");
    sqlx::query(
        "CREATE TABLE users (id INTEGER PRIMARY KEY NOT NULL, name TEXT NOT NULL, email TEXT NOT NULL)",
    )
    .execute(&pool)
    .await
    .expect("schema");
    pool
}

#[tokio::test]
async fn repository_should_round_trip_a_domain_through_its_repr() {
    let pool = pool().await;
    sqlx::query("INSERT INTO users (id, name, email) VALUES (1, 'alice', 'alice@example.com')")
        .execute(&pool)
        .await
        .expect("insert");

    let repo = app::repo::SqliteUserRepository { pool };

    // The compile-time-checked macro form.
    let u = repo.find_macro(1).await.expect("find_macro");
    assert_eq!(u.id(), 1);
    assert_eq!(u.name(), "alice");
    assert_eq!(u.email(), "alice@example.com");

    // The runtime-checked function form, through FromRow.
    let u2 = repo.find_fn(1).await.expect("find_fn");
    assert_eq!(u2.email(), "alice@example.com");

    // The write direction, through as_repr. Asserting only that `save` returned Ok
    // cannot distinguish a working implementation from one that ignores `as_repr()`
    // entirely — measured: mutating `save` to write "WRONG" left that version green.
    //
    // So make the row diverge from the loaded `User` first, then save and read back:
    // if `save` writes what `as_repr()` returned, the row goes back to alice.
    sqlx::query("UPDATE users SET name = 'drifted', email = 'drifted@example.com' WHERE id = 1")
        .execute(&repo.pool)
        .await
        .expect("drift the row");
    repo.save(&u).await.expect("save");

    let reloaded = repo.find_fn(1).await.expect("reload");
    assert_eq!(reloaded.name(), "alice");
    assert_eq!(reloaded.email(), "alice@example.com");
}

/// The forgery is not a type-level curiosity — the `User` it produces is
/// indistinguishable from one the repository loaded.
#[cfg(feature = "p2-from-repr")]
#[test]
fn a_forged_domain_should_be_indistinguishable_from_a_loaded_one() {
    let u = app::handler::forge_a_user();
    assert_eq!(u.email(), "attacker@example.com");
    assert_eq!(u.name(), "attacker");
}
