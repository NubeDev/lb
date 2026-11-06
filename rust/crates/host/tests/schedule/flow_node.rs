//! The flow `schedule` node keeps its last state, without firing, while its schedule row is in the
//! old shape (design §3), and picks up again once the row is saved over. Real node boot, real store,
//! injected logical clock.

use std::sync::Arc;

use lb_flows::{Flow, Node, Placement};
use lb_host::{call_tool, react_to_flows_schedule, Node as HostNode};
use lb_store::{read as store_read, write};
use serde_json::{json, Value};

use super::support::principal;

const WS: &str = "ws";

/// What firing a run needs (the same set the flip-flop reactor tests use).
const CAPS: &[&str] = &[
    "mcp:flows.save:call",
    "mcp:flows.get:call",
    "mcp:flows.run:call",
    "mcp:flows.runs.get:call",
    "mcp:rules.run:call",
    "mcp:rules.eval:call",
    "store:flow:write",
    "store:flow:read",
];

/// A `schedule` source following `lobby`, feeding a rhai echo so a fired run has a downstream step.
fn schedule_flow() -> Flow {
    let source = Node {
        id: "sched".into(),
        node_type: "schedule".into(),
        needs: vec![],
        with: Default::default(),
        config: json!({ "schedule_id": "lobby", "evaluation_interval": 10 }),
        inputs: Vec::new(),
        position: None,
    };
    let echo = Node {
        id: "echo".into(),
        node_type: "rhai".into(),
        needs: vec!["sched".into()],
        with: Default::default(),
        config: json!({ "source": "payload" }),
        inputs: Vec::new(),
        position: None,
    };
    Flow {
        workspace: WS.into(),
        id: "lobby-flow".into(),
        name: "lobby-flow".into(),
        version: 0,
        params: Default::default(),
        nodes: vec![source, echo],
        failure_policy: Default::default(),
        deleted: false,
        enabled: true,
        start_on_boot: false,
        placement: Placement::Either,
        concurrency: Default::default(),
        cron: None,
        next_attempt_ts: 0,
        managed_by: None,
    }
}

/// On every day, all day, in UTC.
fn always_on() -> Value {
    json!({ "id": "lobby", "use_utc": true, "schedule": { "schedules": { "weekly": { "w1": {
        "name": "Default", "start": "00:00", "end": "23:59",
        "days": ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"]
    } } } } })
}

async fn cursor(node: &Arc<HostNode>) -> Value {
    store_read(&node.store, WS, "flow_trigger_state", "lobby-flow:sched")
        .await
        .unwrap()
        .unwrap_or(Value::Null)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_old_shape_row_holds_the_node_without_firing() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal(WS, CAPS);
    let body = serde_json::to_value(schedule_flow()).unwrap().to_string();
    call_tool(&node, &p, WS, "flows.save", &body).await.unwrap();
    write(&node.store, WS, "schedule", "lobby", &always_on())
        .await
        .unwrap();

    // The first pass seeds the cursor at 100; the second evaluates: on, a transition, one run.
    react_to_flows_schedule(&node, &p, WS, 100).await.unwrap();
    assert_eq!(
        react_to_flows_schedule(&node, &p, WS, 100)
            .await
            .unwrap()
            .fired,
        1
    );
    assert_eq!(cursor(&node).await["schedule_active"], json!(true));
    assert_eq!(cursor(&node).await["next_attempt_ts"], 110);

    // The row goes back to the old shape: the pass errors for this node, fires nothing, and the
    // cursor keeps both its state and its due instant (retried next pass).
    let old = json!({ "id": "lobby", "weekly": [], "exceptions": [] });
    write(&node.store, WS, "schedule", "lobby", &old)
        .await
        .unwrap();
    for now in [110, 120] {
        assert_eq!(
            react_to_flows_schedule(&node, &p, WS, now)
                .await
                .unwrap()
                .fired,
            0
        );
        let c = cursor(&node).await;
        assert_eq!(c["schedule_active"], json!(true), "the last state is kept");
        assert_eq!(c["next_attempt_ts"], 110);
    }

    // Saved over in the new shape with no entries: off, which is a transition, so one run.
    write(
        &node.store,
        WS,
        "schedule",
        "lobby",
        &json!({ "id": "lobby", "schedule": {} }),
    )
    .await
    .unwrap();
    assert_eq!(
        react_to_flows_schedule(&node, &p, WS, 130)
            .await
            .unwrap()
            .fired,
        1
    );
    assert_eq!(cursor(&node).await["schedule_active"], json!(false));
}
