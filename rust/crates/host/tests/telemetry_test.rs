//! The telemetry console headless, against the REAL stack (telemetry-console scope, testing §0/§2):
//! real embedded SurrealDB + in-proc Zenoh, the REAL `SurrealCappedLayer` writing the REAL capped
//! ring, the REAL `telemetry.*` verbs reading it. No mocks (CLAUDE §9).
//!
//! Mandatory categories covered:
//!   - **capability-deny per verb** — `telemetry.query`/`trace`/`tail` denied without `telemetry:read`;
//!   - **workspace-isolation** — a ws-B query returns ONLY ws-B rows (the read-surface wall);
//!   - **redaction (the #1 risk)** — a known secret planted through a tool param reaches the stored
//!     ring as a DIGEST only; it appears in ZERO stored rows and ZERO query output;
//!   - **query filter narrowing** — source/level/text filters narrow the page;
//!   - **trace correlation** — `telemetry.trace` returns the rows sharing a trace_id in this ws.

use std::sync::{Arc, OnceLock};

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_bus::Bus;
use lb_host::{call_tool, telemetry_query, telemetry_trace, Node};
use lb_mcp::ToolError;
use lb_store::Store;
use lb_telemetry::{record_dispatch, Level, Outcome, SurrealCappedLayer, TABLE};
use serde_json::{json, Value};
use tracing_subscriber::prelude::*;

fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    verify(&key, &mint(&key, &claims), 1).unwrap()
}

const READ: &str = "mcp:telemetry.read:call";

/// The shared ring store + bus the Once-installed `SurrealCappedLayer` writes to and the tests read
/// from. One global subscriber per process (tracing), so all Layer-driven tests share it; each uses a
/// distinct `ws` so they do not interfere.
///
/// **Why a dedicated leaked runtime.** `Store::memory()` and `Bus::peer()` spawn background tasks onto
/// whatever runtime first builds them. If that runtime is one `#[tokio::test]`'s, it is torn down when
/// that test ends — and every *later* test that touches the shared store then fails with "sending into
/// a closed channel" (the SurrealDB engine task is gone). The Layer's fire-and-forget writes have the
/// same problem: they must land on a runtime that outlives the test that triggered them. So the harness
/// owns its OWN multi-thread runtime (leaked → `'static`), builds the store/bus/subscriber on it, and
/// runs every shared-store operation (emit + read) via `rt.block_on`. Each test stays a plain `#[test]`
/// that drives the harness; the shared infra never dies under it.
struct Harness {
    store: Store,
    bus: Bus,
    rt: tokio::runtime::Runtime,
}

static H: OnceLock<Harness> = OnceLock::new();

/// Serialize the Layer-driven tests. They share ONE process-wide capped ring + ONE harness runtime;
/// each uses a distinct `ws` for its isolation *assertions*, but the shared infra wants one driver at
/// a time so reads and writes don't interleave across tests.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn harness() -> &'static Harness {
    H.get_or_init(|| {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()
            .expect("harness runtime");
        let (store, bus) = rt.block_on(async {
            // The store + bus are the SAME handles the tests read through (one datastore, rule #2).
            let store = Store::memory().await.expect("open store");
            let bus = Bus::peer().await.expect("bus peer");
            (store, bus)
        });
        Harness { store, bus, rt }
    })
}

/// Run a future on the harness's surviving runtime (NOT a per-test runtime), so shared-store ops
/// never outlive their engine task.
fn on_harness<F, T>(fut: F) -> T
where
    F: std::future::Future<Output = T>,
{
    harness().rt.block_on(fut)
}

/// One shared bootstrapped `Node` for the deny/bridge tests, booted ONCE on the harness runtime. Each
/// `Node::boot()` opens a Zenoh peer + a SurrealDB instance; booting four separately (one per test)
/// leaves background peers that flood discovery and saturate the runtime where the harness's capped
/// writes run — the repo's documented many-peers problem. One shared node on the surviving runtime
/// removes that interference. The deny tests only read authorization decisions, so they share it
/// safely (each uses a distinct `ws`/cap set).
static NODE: OnceLock<Arc<Node>> = OnceLock::new();

fn shared_node() -> Arc<Node> {
    NODE.get_or_init(|| Arc::new(on_harness(async { Node::boot().await.unwrap() })))
        .clone()
}

/// Spawned Layer writes are fire-and-forget; poll the ring until `pred(row count)` holds (bounded).
fn until<F: Fn(usize) -> bool>(ws: &str, pred: F) {
    for _ in 0..200 {
        if pred(count_rows(ws)) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!(
        "until() timed out for ws={ws}: ring never reached the expected count (last={})",
        count_rows(ws)
    );
}

fn count_rows(ws: &str) -> usize {
    on_harness(async move {
        let mut resp = harness()
            .store
            .query_ws(
                ws,
                "SELECT count() AS c FROM type::table($tb) GROUP ALL",
                vec![("tb".into(), json!(TABLE))],
            )
            .await
            .unwrap();
        let rows: Vec<Value> = resp.take(0).unwrap();
        rows.first()
            .and_then(|r| r.get("c"))
            .and_then(|c| c.as_u64())
            .unwrap_or(0) as usize
    })
}

/// A capturing `tracing` Layer: on each event it drives the REAL `collect_event` (target filter +
/// sampling + field collection + redaction + cap-key selection) and stashes the `(cap_key, record)`
/// in a slot. The harness then AWAITS the real `write_event` with that record — so `emit` exercises
/// the full Layer collection AND the real capped write, **deterministically** (no fire-and-forget
/// race). Production still uses the Layer's own fire-and-forget `on_event`; this only changes the
/// *delivery* of the collected record so a test can await the write instead of polling for it.
type Slot = Arc<std::sync::Mutex<Option<(String, Value)>>>;

struct Capture {
    layer: SurrealCappedLayer,
    slot: Slot,
}

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Capture {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        if let Some(collected) = lb_telemetry::collect_event(&self.layer, event) {
            *self.slot.lock().unwrap() = Some(collected);
        }
    }
}

/// Emit a dispatch event through the REAL Layer collection path, then AWAIT the real capped write +
/// tail publish — deterministic (the production Layer fire-and-forgets the write; here we await it so
/// a following read sees the row without racing a spawn).
fn emit(
    ws: &str,
    tool: &str,
    source: &str,
    trace_id: &str,
    outcome: Outcome,
    level: Level,
    params: Value,
    msg: &str,
) {
    let h = harness();
    let slot: Slot = Arc::new(std::sync::Mutex::new(None));
    let capture = Capture {
        layer: SurrealCappedLayer::new(h.store.clone()).with_bus(h.bus.clone()),
        slot: slot.clone(),
    };
    let dispatch = tracing::Dispatch::new(tracing_subscriber::registry().with(capture));
    tracing::dispatcher::with_default(&dispatch, || {
        record_dispatch(
            level, ws, "user:ada", tool, source, trace_id, outcome, &params, 1, msg,
        );
    });
    let collected = slot.lock().unwrap().take();
    if let Some((cap_key, record)) = collected {
        on_harness(lb_telemetry::write_event(
            h.store.clone(),
            Some(h.bus.clone()),
            lb_telemetry::DEFAULT_CAP,
            cap_key,
            record,
        ));
    }
}

// ---------------------------------------------------------------------------------------------
// Capability deny per verb
// ---------------------------------------------------------------------------------------------

#[test]
fn query_denied_without_read_cap() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let node = shared_node();
    let p = principal("user:ada", "td", &[]); // no telemetry.read
    let err = on_harness(call_tool(
        &node,
        &p,
        "td",
        "telemetry.query",
        &json!({}).to_string(),
    ))
    .unwrap_err();
    assert!(
        matches!(err, ToolError::Denied),
        "opaque deny, no leak: {err:?}"
    );
}

#[test]
fn trace_denied_without_read_cap() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let node = shared_node();
    let p = principal("user:ada", "td", &[]);
    let err = on_harness(call_tool(
        &node,
        &p,
        "td",
        "telemetry.trace",
        &json!({"trace_id":"x"}).to_string(),
    ))
    .unwrap_err();
    assert!(matches!(err, ToolError::Denied));
}

#[test]
fn tail_denied_without_read_cap() {
    // Capability-first (rule #5): the dispatch chokepoint authorizes `mcp:telemetry.read:call` BEFORE
    // it ever reaches the bridge, so a principal without the grant gets an opaque `Denied` — it never
    // learns that `telemetry.tail`'s live feed rides the SSE route (a granted caller hitting the
    // bridge gets `NotFound`, asserted in `tail_via_bridge_is_notfound_when_granted`). The deny path
    // is the security-relevant one and it must be `Denied`, not a verb-shape leak.
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let node = shared_node();
    let p = principal("user:ada", "td", &[]);
    let err = on_harness(call_tool(
        &node,
        &p,
        "td",
        "telemetry.tail",
        &json!({}).to_string(),
    ))
    .unwrap_err();
    assert!(
        matches!(err, ToolError::Denied),
        "opaque deny before the bridge: {err:?}"
    );
}

#[test]
fn tail_via_bridge_is_notfound_when_granted() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    // A GRANTED caller reaching `telemetry.tail` through the MCP bridge gets `NotFound`: the live feed
    // is the SSE route (which calls `telemetry_tail` directly), not an in-band tool result. This
    // proves the verb is not a silent fall-through to a data read once past the cap gate.
    let node = shared_node();
    let p = principal("user:ada", "td", &[READ]);
    let err = on_harness(call_tool(
        &node,
        &p,
        "td",
        "telemetry.tail",
        &json!({}).to_string(),
    ))
    .unwrap_err();
    assert!(
        matches!(err, ToolError::NotFound),
        "granted bridge call is NotFound: {err:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// Workspace isolation — the read-surface wall
// ---------------------------------------------------------------------------------------------

#[test]
fn query_returns_only_callers_workspace_rows() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    // Seed ws-A and ws-B rows through the real Layer (the dispatch emission path).
    emit(
        "iso-a",
        "doc.read",
        "host",
        "t-a",
        Outcome::Allow,
        Level::Info,
        json!({}),
        "A row",
    );
    emit(
        "iso-a",
        "doc.read",
        "host",
        "t-a2",
        Outcome::Deny,
        Level::Warn,
        json!({}),
        "A deny",
    );
    emit(
        "iso-b",
        "doc.read",
        "host",
        "t-b",
        Outcome::Allow,
        Level::Info,
        json!({}),
        "B row",
    );
    until("iso-b", |n| n >= 1);
    until("iso-a", |n| n >= 2);

    let pb = principal("user:b", "iso-b", &[READ]);
    // NOTE: telemetry_query reads through the SHARED harness store (where the Layer wrote), with the
    // caller's ws hard-appended to the filter — so a ws-B caller gets ONLY iso-B rows.
    let page = on_harness(async {
        telemetry_query(
            &harness().store,
            &pb,
            "iso-b",
            &Default::default(),
            50,
            None,
        )
        .await
        .expect("granted")
    });
    let tools: Vec<&str> = page
        .rows
        .iter()
        .map(|r| r.get("ws").and_then(|v| v.as_str()).unwrap_or(""))
        .collect();
    assert!(
        tools.iter().all(|w| *w == "iso-b"),
        "ws-B sees only ws-B: {tools:?}"
    );
    assert_eq!(page.rows.len(), 1, "exactly the one ws-B row");
}

// ---------------------------------------------------------------------------------------------
// Redaction — the #1 risk: a planted secret reaches the ring as a DIGEST only
// ---------------------------------------------------------------------------------------------

#[test]
fn planted_secret_appears_in_zero_stored_rows_or_query_output() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let ws = "redact";
    let secret = "SUPER_SECRET_BEARER_lbk_x9876";
    // The secret travels as a tool PARAM — exactly the leak vector. record_dispatch digests params;
    // the raw value must never reach the ring.
    emit(
        ws,
        "secret.get",
        "host",
        "t-redact",
        Outcome::Allow,
        Level::Info,
        json!({ "token": secret, "scope": "github" }),
        "fetched a secret",
    );
    until(ws, |n| n >= 1);

    let p = principal("user:ada", ws, &[READ]);
    let (blob, out) = on_harness(async {
        // 1. The secret is in ZERO stored rows (scan the raw ring, not just the query projection).
        let mut resp = harness()
            .store
            .query_ws(
                ws,
                "SELECT * OMIT id FROM type::table($tb)",
                vec![("tb".into(), json!(TABLE))],
            )
            .await
            .unwrap();
        let rows: Vec<Value> = resp.take(0).unwrap();
        let blob = serde_json::to_string(&rows).unwrap();
        // 2. The secret is in ZERO query output.
        let page = telemetry_query(&harness().store, &p, ws, &Default::default(), 50, None)
            .await
            .unwrap();
        let out = serde_json::to_string(&page.rows).unwrap();
        (blob, out)
    });
    assert!(
        !blob.contains(secret),
        "the planted secret must NOT appear in any stored row\n{blob}"
    );
    // The digest + shape ARE present (proof the param was recorded, just redacted).
    assert!(
        blob.contains("params_digest"),
        "the digest field is recorded"
    );
    assert!(
        !out.contains(secret),
        "the planted secret must NOT appear in query output\n{out}"
    );
}

// ---------------------------------------------------------------------------------------------
// Query filter narrowing + trace correlation
// ---------------------------------------------------------------------------------------------

#[test]
fn filters_narrow_and_trace_correlates() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let ws = "filter";
    emit(
        ws,
        "doc.read",
        "host",
        "tr1",
        Outcome::Allow,
        Level::Info,
        json!({}),
        "all good here",
    );
    emit(
        ws,
        "doc.read",
        "host",
        "tr1",
        Outcome::Deny,
        Level::Warn,
        json!({}),
        "denied access",
    );
    emit(
        ws,
        "mqtt.publish",
        "mqtt",
        "tr2",
        Outcome::Error,
        Level::Error,
        json!({}),
        "broker down",
    );
    until(ws, |n| n >= 3);

    let p = principal("user:ada", ws, &[READ]);
    use lb_host::QueryFilter;

    on_harness(async {
        let store = &harness().store;

        // Filter: source = mqtt → only the mqtt row.
        let f = QueryFilter {
            source: Some("mqtt".into()),
            ..Default::default()
        };
        let page = telemetry_query(store, &p, ws, &f, 50, None).await.unwrap();
        assert_eq!(page.rows.len(), 1);
        assert_eq!(page.rows[0]["source"], "mqtt");

        // Filter: min level = error → only the error row.
        let f = QueryFilter {
            min_level: Some(Level::Error),
            ..Default::default()
        };
        let page = telemetry_query(store, &p, ws, &f, 50, None).await.unwrap();
        assert_eq!(page.rows.len(), 1);
        assert_eq!(page.rows[0]["level"], "error");

        // Filter: free-text "denied" → only the deny row.
        let f = QueryFilter {
            text: Some("denied".into()),
            ..Default::default()
        };
        let page = telemetry_query(store, &p, ws, &f, 50, None).await.unwrap();
        assert!(page.rows.iter().all(|r| {
            r.get("msg")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase()
                .contains("denied")
        }));
        assert_eq!(page.rows.len(), 1);

        // Trace correlation: tr1 has two rows; tr2 has one.
        let tr1 = telemetry_trace(store, &p, ws, "tr1").await.unwrap();
        assert_eq!(tr1.len(), 2, "tr1 correlates its two rows");
        let tr2 = telemetry_trace(store, &p, ws, "tr2").await.unwrap();
        assert_eq!(tr2.len(), 1);
    });
}
