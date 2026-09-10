//! Does a VIEWER session on an EMBEDDED store actually stop writes?
//!
//! The `store.query` read-only design rests on this. SurrealDB 3 removed the public `read_only()`
//! reporting the old text-parsing gate used, so the replacement is to let the ENGINE refuse writes:
//! run the caller's SQL in a session authenticated as `Role::Viewer`.
//!
//! `ctx::check_perms` (surrealdb-core 3.2.4, `src/ctx/context.rs:612`) enforces table PERMISSIONS
//! only when `opt.perms` is set AND the auth is not anonymous-with-auth-disabled. An embedded store
//! has authentication disabled by default, so only a real run settles what actually happens.
//!
//! FIRST RESULT: a viewer's `CREATE` returns `Ok([])` — an empty array, NOT an error. So the
//! question that matters is not "did it error" but **"did the data change"**. These tests assert the
//! effect on the data, which is the only thing that makes this safe to build on.
//!
//! Real embedded store, no mocks.

use surrealdb::engine::local::{Db, Mem};
use surrealdb::opt::auth::Root;
use surrealdb::Surreal;

async fn root() -> Surreal<Db> {
    let db = Surreal::new::<Mem>(()).await.expect("open mem engine");
    db.query("USE NS probe DB main;\nCREATE person:a SET name = 'a';")
        .await
        .expect("seed runs")
        .check()
        .expect("seed succeeds");
    db
}

/// Define a VIEWER and sign a CLONE of the handle in as that user.
async fn reader(db: &Surreal<Db>) -> Surreal<Db> {
    db.query("DEFINE USER IF NOT EXISTS probe_reader ON ROOT PASSWORD 'probe-pw' ROLES VIEWER;")
        .await
        .expect("define viewer")
        .check()
        .expect("define succeeds");
    let clone = db.clone();
    clone
        .signin(Root {
            username: "probe_reader".to_string(),
            password: "probe-pw".to_string(),
        })
        .await
        .expect("signin as viewer");
    clone
}

/// Every `name` currently in `person`, read through the ROOT handle (the ground truth).
async fn names(db: &Surreal<Db>) -> Vec<String> {
    let mut r = db
        .query("USE NS probe DB main;\nSELECT VALUE name FROM person ORDER BY name;")
        .await
        .expect("read runs");
    r.take(1).expect("names")
}

#[tokio::test]
async fn a_viewer_session_still_reads() {
    let db = root().await;
    let r = reader(&db).await;
    let mut resp = r
        .query("USE NS probe DB main;\nSELECT * FROM person;")
        .await
        .expect("select runs");
    let rows: Vec<serde_json::Value> = resp.take(1).expect("rows come back");
    assert_eq!(rows.len(), 1, "the viewer must still see the seeded row");
}

#[tokio::test]
async fn a_viewer_create_does_not_land() {
    let db = root().await;
    let r = reader(&db).await;
    let _ = r
        .query("USE NS probe DB main;\nCREATE person:b SET name = 'b';")
        .await;
    assert_eq!(
        names(&db).await,
        vec!["a".to_string()],
        "a viewer's CREATE must not add a row"
    );
}

#[tokio::test]
async fn a_viewer_update_does_not_land() {
    let db = root().await;
    let r = reader(&db).await;
    let _ = r
        .query("USE NS probe DB main;\nUPDATE person:a SET name = 'zzz';")
        .await;
    assert_eq!(
        names(&db).await,
        vec!["a".to_string()],
        "a viewer's UPDATE must not change the row"
    );
}

#[tokio::test]
async fn a_viewer_delete_does_not_land() {
    let db = root().await;
    let r = reader(&db).await;
    let _ = r.query("USE NS probe DB main;\nDELETE person;").await;
    assert_eq!(
        names(&db).await,
        vec!["a".to_string()],
        "a viewer's DELETE must not remove the row"
    );
}

#[tokio::test]
async fn a_viewer_cannot_remove_a_table() {
    let db = root().await;
    let r = reader(&db).await;
    let _ = r.query("USE NS probe DB main;\nREMOVE TABLE person;").await;
    assert_eq!(
        names(&db).await,
        vec!["a".to_string()],
        "a viewer's REMOVE TABLE must not drop the table"
    );
}

#[tokio::test]
async fn a_viewer_cannot_define_a_table_or_grant_itself_more() {
    let db = root().await;
    let r = reader(&db).await;
    // Redefining the table with permissive PERMISSIONS would be the obvious escalation.
    let _ = r
        .query("USE NS probe DB main;\nDEFINE TABLE person SCHEMALESS PERMISSIONS FULL;")
        .await;
    let _ = r
        .query("DEFINE USER probe_escalate ON ROOT PASSWORD 'x' ROLES OWNER;")
        .await;
    // If either landed, a write would now succeed. Prove it still does not.
    let _ = r
        .query("USE NS probe DB main;\nCREATE person:esc SET name = 'esc';")
        .await;
    assert_eq!(
        names(&db).await,
        vec!["a".to_string()],
        "a viewer must not be able to widen its own permissions"
    );
}

#[tokio::test]
async fn signing_in_the_clone_does_not_downgrade_the_original() {
    let db = root().await;
    let _r = reader(&db).await;
    let out = db
        .query("USE NS probe DB main;\nCREATE person:c SET name = 'c';")
        .await
        .and_then(surrealdb::IndexedResults::check);
    assert!(
        out.is_ok(),
        "the root handle must keep its privileges: {out:?}"
    );
    assert_eq!(names(&db).await, vec!["a".to_string(), "c".to_string()]);
}

/// What a refused write actually looks like to the caller — recorded, not asserted as a
/// requirement, so the shape is in the record when the runner reads it.
#[tokio::test]
async fn record_what_a_refused_write_returns() {
    let db = root().await;
    let r = reader(&db).await;
    let out = r
        .query("USE NS probe DB main;\nCREATE person:b SET name = 'b';")
        .await
        .and_then(surrealdb::IndexedResults::check);
    println!("REFUSED-WRITE-SHAPE: {out:?}");
}
