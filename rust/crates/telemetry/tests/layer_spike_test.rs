use lb_store::Store;
use lb_telemetry::{record_dispatch, Level, Outcome, SurrealCappedLayer, TABLE};
use serde_json::json;
use tracing_subscriber::prelude::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spike_layer_writes_and_reads() {
    let store = Store::memory().await.unwrap();
    let bus = lb_bus::Bus::peer().await.unwrap();
    let layer = SurrealCappedLayer::new(store.clone()).with_bus(bus);
    let _ = tracing::subscriber::set_global_default(tracing_subscriber::registry().with(layer));

    record_dispatch(
        Level::Info,
        "spike",
        "user:a",
        "doc.read",
        "host",
        "t1",
        Outcome::Allow,
        &json!({"x":1}),
        1,
        "hi",
    );
    eprintln!("emitted; waiting for spawn to land");

    for i in 0..100 {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let mut r = store
            .query_ws(
                "spike",
                "SELECT count() AS c FROM type::table($tb) GROUP ALL",
                vec![("tb".into(), json!(TABLE))],
            )
            .await
            .unwrap();
        let v: Vec<serde_json::Value> = r.take(0).unwrap();
        let n = v
            .first()
            .and_then(|r| r.get("c"))
            .and_then(|c| c.as_u64())
            .unwrap_or(0);
        eprintln!("poll {i}: count={n}");
        if n >= 1 {
            return;
        }
    }
    panic!("layer never wrote");
}
