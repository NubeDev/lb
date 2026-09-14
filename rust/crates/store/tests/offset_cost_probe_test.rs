//! PROBE: what does OFFSET (`START n`) cost at depth, versus the keyset cursor?
//! The pager needs random access (jump to page N, last page), which a cursor cannot serve. Since
//! `ORDER BY` keeps a sort in every plan anyway, offset may be no worse here — measure, don't assume.
use lb_store::Store;

async fn timed(store: &Store, label: &str, sql: &str) {
    let t = std::time::Instant::now();
    let n = match store.query_ws("nube", sql, vec![]).await {
        Ok(mut r) => r
            .take::<Vec<serde_json::Value>>(0)
            .map(|v| v.len())
            .unwrap_or(0),
        Err(e) => {
            println!(
                "  {label:<34} ERROR {}",
                e.to_string().lines().next().unwrap_or("").trim()
            );
            return;
        }
    };
    println!(
        "  {label:<34} {:>8.2} ms  rows={n}",
        t.elapsed().as_secs_f64() * 1000.0
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn offset_versus_cursor_at_depth() {
    let store = Store::memory().await.expect("mem store");
    store
        .query_ws("nube", "DEFINE TABLE ins SCHEMALESS;", vec![])
        .await
        .expect("table");
    const N: u64 = 20_000;
    for chunk in 0..(N / 1000) {
        let mut sql = String::new();
        for i in (chunk * 1000)..((chunk + 1) * 1000) {
            sql.push_str(&format!(
                "CREATE ins:id{i} SET data = {{ last_ts: {}, title: 'Flatline MDB-{i}' }}, rev = 1;",
                1788000000000u64 + i));
        }
        store.query_ws("nube", &sql, vec![]).await.expect("seed");
    }
    store
        .query_ws(
            "nube",
            "DEFINE INDEX ins_ts ON ins FIELDS data.last_ts;",
            vec![],
        )
        .await
        .expect("index");

    let sel = "SELECT data, data.last_ts AS _ts FROM ins ORDER BY _ts DESC LIMIT 20";
    println!("\n--- {N} rows, page size 20 ---");
    timed(&store, "page 1      (START 0)", &format!("{sel} START 0")).await;
    timed(
        &store,
        "page 10     (START 180)",
        &format!("{sel} START 180"),
    )
    .await;
    timed(
        &store,
        "page 100    (START 1980)",
        &format!("{sel} START 1980"),
    )
    .await;
    timed(
        &store,
        "page 500    (START 9980)",
        &format!("{sel} START 9980"),
    )
    .await;
    timed(
        &store,
        "LAST page   (START 19980)",
        &format!("{sel} START 19980"),
    )
    .await;
    println!("--- the cursor equivalent, for comparison ---");
    timed(&store, "keyset near newest", "SELECT data, data.last_ts AS _ts FROM ins WHERE data.last_ts < 1788000019900 ORDER BY _ts DESC LIMIT 20").await;
    timed(&store, "keyset near oldest", "SELECT data, data.last_ts AS _ts FROM ins WHERE data.last_ts < 1788000000100 ORDER BY _ts DESC LIMIT 20").await;
    println!("--- and the total the pager needs for `page N of M` ---");
    timed(
        &store,
        "count() GROUP ALL",
        "SELECT count() AS n FROM ins GROUP ALL",
    )
    .await;
}
