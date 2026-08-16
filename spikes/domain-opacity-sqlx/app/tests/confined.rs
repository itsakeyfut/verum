//! P28 — #33's `pass` case: the confined constructor still works for the code it
//! is meant to work for.
//!
//! A probe set that only shows rejections proves nothing about the design being
//! usable — an implementation where *everything* fails to compile would score
//! perfectly. This is the counterweight: the repository the derive emits beside
//! the domain loads a real row through the module-private `from_repr`, and the
//! resulting `Account` reads back correctly.
//!
//! Same in-memory setup as `roundtrip.rs`, and for the same reason recorded
//! there: `max_connections(1)`, because a second connection to `:memory:` gets
//! its own empty database and the INSERT would vanish before the SELECT.

use sqlx::sqlite::SqlitePoolOptions;

async fn pool() -> sqlx::SqlitePool {
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
    sqlx::query("INSERT INTO users (id, name, email) VALUES (1, 'alice', 'alice@example.com')")
        .execute(&pool)
        .await
        .expect("insert");
    pool
}

#[tokio::test]
async fn generated_repository_should_load_through_the_confined_constructor() {
    let pool = pool().await;

    let account = app::confined::AccountRepository
        .find(&pool, 1)
        .await
        .expect("row 1 exists");
    assert_eq!(account.id(), 1);
    assert_eq!(account.email(), "alice@example.com");

    // The tightened shape — a module-private `Repr`, which is what P30 shows also
    // closes ledger paths 3 and 4 — loads identically. Constraint 2 costs nothing
    // at the call site.
    let secret = app::confined::SecretRepository
        .find(&pool, 1)
        .await
        .expect("row 1 exists");
    assert_eq!(secret.email(), account.email());
}

/// The nested mechanism's `pass` case, and the reason the needle above counts
/// **two**: review measured that emptying a single test's body still printed
/// `ok. 1 passed`, so the count guarded against deleting the test but not
/// against deleting its assertions. Two tests that each assert make the count
/// sensitive to either being hollowed out.
#[tokio::test]
async fn nested_repository_should_load_and_expose_only_declared_fields() {
    let pool = pool().await;
    let account = app::nested::AccountRepository
        .find(&pool, 1)
        .await
        .expect("row 1 exists");
    assert_eq!(account.id(), 1);
    assert_eq!(account.email(), "alice@example.com");
    assert_eq!(account.name(), "alice");

    // The read half works from inside the generated module and nowhere else
    // (P34 is the rejection). This is the call P35 proves a handler cannot make.
    let dumped = app::nested::AccountRepository.debug_repr(&account);
    assert!(dumped.contains("alice@example.com"));

    // run.sh needles on this line, not on the test count. Review measured that
    // `ok. N passed` counts *tests*, so emptying a body while keeping the fn left
    // the probe green. A value that only a real load can produce cannot be
    // emptied away without the needle going missing.
    println!("VERUM_P28_LOADED={}", account.email());
}
