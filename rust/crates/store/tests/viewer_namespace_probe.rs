//! Does a VIEWER session still respect the WORKSPACE WALL?
//!
//! `viewer_session_probe` proved a root-level VIEWER cannot write. That is only half the job.
//! lb's workspace wall is a per-query `USE NS <ws> DB main;` prepended to the caller's SQL. The old
//! parse gate allowlisted a SINGLE `SELECT`/`INFO`/`SHOW`, so a caller could not smuggle a second
//! statement — a `USE NS other-ws;` — into the batch.
//!
//! Remove that gate and the question becomes urgent: can a caller reach another workspace by
//! writing its own `USE`? A ROOT-level viewer can read every namespace, so the answer would be yes,
//! and the wall would be gone. The fix is to sign the reader in at DATABASE level for the one
//! workspace, so the engine itself refuses the other.
//!
//! Real embedded store, no mocks.

use surrealdb::engine::local::{Db, Mem};
use surrealdb::opt::auth::{Database, Root};
use surrealdb::Surreal;

/// Two workspaces, one row each, seeded through root.
async fn root() -> Surreal<Db> {
    let db = Surreal::new::<Mem>(()).await.expect("open mem engine");
    for (ns, val) in [("ws-a", "secret-a"), ("ws-b", "secret-b")] {
        db.query(format!(
            "USE NS `{ns}` DB main;\nCREATE thing:x SET v = '{val}';"
        ))
        .await
        .expect("seed runs")
        .check()
        .expect("seed succeeds");
    }
    db
}

async fn root_viewer(db: &Surreal<Db>) -> Surreal<Db> {
    db.query("DEFINE USER IF NOT EXISTS r_reader ON ROOT PASSWORD 'pw' ROLES VIEWER;")
        .await
        .expect("define")
        .check()
        .expect("ok");
    let c = db.clone();
    c.signin(Root {
        username: "r_reader".to_string(),
        password: "pw".to_string(),
    })
    .await
    .expect("signin");
    c
}

/// A viewer scoped to ONE database (`ns`/main) rather than to root.
async fn db_viewer(db: &Surreal<Db>, ns: &str) -> Surreal<Db> {
    db.query(format!(
        "USE NS `{ns}` DB main;\nDEFINE USER IF NOT EXISTS d_reader ON DATABASE PASSWORD 'pw' ROLES VIEWER;"
    ))
    .await
    .expect("define")
    .check()
    .expect("ok");
    let c = db.clone();
    c.signin(Database {
        namespace: ns.to_string(),
        database: "main".to_string(),
        username: "d_reader".to_string(),
        password: "pw".to_string(),
    })
    .await
    .expect("signin");
    c
}

async fn read_v(h: &Surreal<Db>, ns: &str) -> Result<Vec<String>, surrealdb::Error> {
    let mut r = h
        .query(format!(
            "USE NS `{ns}` DB main;\nSELECT VALUE v FROM thing;"
        ))
        .await?;
    r.take(1)
}

#[tokio::test]
async fn a_root_viewer_can_read_across_workspaces_so_it_is_not_enough() {
    let db = root().await;
    let v = root_viewer(&db).await;
    let a = read_v(&v, "ws-a").await;
    let b = read_v(&v, "ws-b").await;
    println!("ROOT-VIEWER ws-a: {a:?}");
    println!("ROOT-VIEWER ws-b: {b:?}");
    // Recorded, not required: this documents WHY a root-level viewer cannot be the design.
}

#[tokio::test]
async fn a_database_viewer_reads_its_own_workspace() {
    let db = root().await;
    let v = db_viewer(&db, "ws-a").await;
    let got = read_v(&v, "ws-a").await.expect("own ws reads");
    assert_eq!(got, vec!["secret-a".to_string()]);
}

#[tokio::test]
async fn a_database_viewer_cannot_reach_another_workspace() {
    let db = root().await;
    let v = db_viewer(&db, "ws-a").await;
    let got = read_v(&v, "ws-b").await;
    println!("DB-VIEWER reaching ws-b: {got:?}");
    let leaked = got.unwrap_or_default();
    assert!(
        !leaked.iter().any(|s| s == "secret-b"),
        "a ws-a reader must NOT see ws-b rows, got {leaked:?}"
    );
}

#[tokio::test]
async fn a_database_viewer_still_cannot_write_its_own_workspace() {
    let db = root().await;
    let v = db_viewer(&db, "ws-a").await;
    let _ = v
        .query("USE NS `ws-a` DB main;\nCREATE thing:y SET v = 'nope';")
        .await;
    let got = read_v(&db, "ws-a").await.expect("reread");
    assert_eq!(got, vec!["secret-a".to_string()], "the write must not land");
}
