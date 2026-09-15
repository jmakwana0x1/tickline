//! Phase 0 smoke test for the database harness.
//!
//! `#[sqlx::test]` creates a fresh database per test and drops it afterwards. This test exists
//! to prove that machinery works before any ledger logic depends on it. Run it with
//! `just test-db` (needs `just up`); it is behind the `db-tests` feature so a developer without
//! Docker still gets a green `just test-rust`.
#![cfg(feature = "db-tests")]

use sqlx::{PgPool, Row};

#[sqlx::test]
async fn sqlx_test_creates_and_drops_a_database(pool: PgPool) -> sqlx::Result<()> {
    let name: String = sqlx::query("SELECT current_database()")
        .fetch_one(&pool)
        .await?
        .get(0);
    assert!(
        !name.is_empty(),
        "sqlx::test should hand us a live, per-test database"
    );

    // Migrations ran: the ledger's append-only guard is present from the first schema.
    let applied: i64 = sqlx::query("SELECT count(*) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await?
        .get(0);
    assert!(applied >= 1, "expected at least one applied migration");

    Ok(())
}
