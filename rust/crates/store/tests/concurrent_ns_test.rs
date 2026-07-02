//! REGRESSION (debugging/store/concurrent-use-ns-namespace-race): the workspace wall must hold
//! under *concurrency*, not just serially. `Store` shares ONE SurrealDB connection whose session
//! namespace is selected by `use_ws(ws)` — if that selection is a separate step from the query it
//! guards, two tasks targeting different workspaces on a multi-thread runtime can interleave
//! (`use_ns(A)` … `use_ns(B)` … A's query runs against B), so a write lands in — or a read is served
//! from — the WRONG namespace. That surfaced as the flaky login "not a member of any workspace":
//! `membership_login_resolve` wrote the bootstrap membership into one namespace and read it back from
//! another. This test forces the interleave: N workspaces each write their own record concurrently,
//! then every workspace must read back exactly its own value and nothing else.

use std::sync::Arc;

use lb_store::{list, read, write, Store};
use serde_json::json;

/// Multi-thread runtime (the gateway's posture) so concurrent tasks genuinely run on different
/// worker threads — a current-thread runtime would serialize them and hide the race.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_writes_never_cross_namespaces() {
    let store = Arc::new(Store::memory().await.expect("open in-memory store"));

    // Enough concurrent workspaces to reliably force the `use_ns` interleave on a 4-thread runtime.
    const N: usize = 64;

    // Every workspace writes its OWN record concurrently. A namespace flip mid-write would land a
    // row in a sibling's namespace (or drop this one), which the read-back below then catches.
    let mut writers = Vec::new();
    for i in 0..N {
        let store = Arc::clone(&store);
        writers.push(tokio::spawn(async move {
            let ws = format!("ws-{i}");
            write(&store, &ws, "note", "row", &json!({ "owner": ws }))
                .await
                .expect("write own record");
        }));
    }
    for w in writers {
        w.await.expect("writer task panicked");
    }

    // Every workspace must read back EXACTLY its own record — and its `note` table must hold exactly
    // one row (a leaked cross-namespace write would show up as a foreign owner or an extra row).
    let mut readers = Vec::new();
    for i in 0..N {
        let store = Arc::clone(&store);
        readers.push(tokio::spawn(async move {
            let ws = format!("ws-{i}");
            let value = read(&store, &ws, "note", "row")
                .await
                .expect("read own record")
                .unwrap_or_else(|| panic!("{ws}: own record missing (write landed elsewhere?)"));
            assert_eq!(
                value["owner"], ws,
                "{ws}: read back a foreign owner (namespace leaked under concurrency)"
            );
            let rows = list(&store, &ws, "note", "owner", &ws)
                .await
                .expect("list own rows");
            assert_eq!(
                rows.len(),
                1,
                "{ws}: expected exactly its own row, saw {}",
                rows.len()
            );
        }));
    }
    for r in readers {
        r.await.expect("reader task panicked");
    }
}
