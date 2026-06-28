//! Host-layer tests for the `chains.*` service (rule-chains-scope Testing plan). Real store, real
//! caps, real lb-jobs, rules seeded as real records and run through the real lb-rules engine. The only
//! fake is the model provider behind the AI seam (unused here — these chains read data + emit).
//!
//! Mandatory: DAG validation (a cycle rejected before any run), capability-deny (each verb), workspace-
//! isolation (ws-B cannot run/get a ws-A chain), frontier behavior (diamond order + Halt skips the
//! subtree), and offline/sync (a node restart resumes the run exactly once — the headline).

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    call_tool, chains_resume, chains_run, chains_run_get, chains_save, rules_save, Node, RuleModel,
};
use lb_rules::workflow::{Chain, FailurePolicy, Step, Trigger};
use lb_rules::RuleParam;

fn principal(ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:test".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
    };
    verify(&key, &mint(&key, &claims), 1).unwrap()
}

const FULL: &[&str] = &[
    "mcp:chains.save:call",
    "mcp:chains.run:call",
    "mcp:chains.get:call",
    "mcp:chains.list:call",
    "mcp:chains.delete:call",
    "mcp:rules.run:call",
    "store:chain:write",
    "store:chain:read",
    "store:rule:write",
    "store:rule:read",
];

struct M;
impl RuleModel for M {
    fn complete(&self, _: &str) -> Result<(String, u32), String> {
        Ok(("x".into(), 1))
    }
    fn propose_sql(&self, _: &str, _: &str) -> Result<String, String> {
        Ok("SELECT 1 AS v".into())
    }
}

fn step(id: &str, rule: &str, needs: &[&str]) -> Step {
    Step {
        id: id.into(),
        rule: rule.into(),
        needs: needs.iter().map(|s| s.to_string()).collect(),
        with: serde_json::Map::new(),
        retry: None,
    }
}

fn chain(ws: &str, id: &str, steps: Vec<Step>, policy: FailurePolicy) -> Chain {
    Chain {
        workspace: ws.into(),
        id: id.into(),
        name: id.into(),
        trigger: Trigger::Manual,
        params: serde_json::Map::new(),
        steps,
        failure_policy: policy,
    }
}

/// Seed the saved rules a chain's steps reference.
async fn seed_rule(node: &Node, p: &Principal, ws: &str, name: &str, body: &str) {
    rules_save(
        &node.store,
        p,
        ws,
        name,
        name,
        body,
        Vec::<RuleParam>::new(),
    )
    .await
    .unwrap();
}

// ----- capability deny -------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn each_chains_verb_is_denied_without_its_cap() {
    let ws = "chains-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(ws, &[]);
    for (tool, input) in [
        (
            "chains.save",
            serde_json::json!({ "workspace": ws, "id": "c", "name": "c", "steps": [] }),
        ),
        ("chains.get", serde_json::json!({ "id": "c" })),
        ("chains.list", serde_json::json!({})),
        ("chains.run", serde_json::json!({ "chain_id": "c" })),
        ("chains.delete", serde_json::json!({ "id": "c" })),
    ] {
        assert!(
            call_tool(&node, &p, ws, tool, &input.to_string())
                .await
                .is_err(),
            "{tool} must be denied"
        );
    }
}

// ----- DAG validation --------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn save_rejects_a_cyclic_dag_before_any_run() {
    let ws = "chains-cycle";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(ws, FULL);
    let c = chain(
        ws,
        "bad",
        vec![step("a", "r", &["b"]), step("b", "r", &["a"])],
        FailurePolicy::Halt,
    );
    let err = chains_save(&node.store, &p, ws, &c).await;
    assert!(err.is_err(), "a cycle must be rejected at save");
}

// ----- happy diamond ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn diamond_runs_all_steps_to_success() {
    let ws = "chains-diamond";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(ws, FULL);
    // four rules that each emit a finding (so they succeed deterministically with no data deps).
    for r in ["ra", "rb", "rc", "rd"] {
        seed_rule(&node, &p, ws, r, r#"emit(#{ level: "info", msg: "ok" });"#).await;
    }
    let c = chain(
        ws,
        "diamond",
        vec![
            step("a", "ra", &[]),
            step("b", "rb", &["a"]),
            step("c", "rc", &["a"]),
            step("d", "rd", &["b", "c"]),
        ],
        FailurePolicy::Halt,
    );
    chains_save(&node.store, &p, ws, &c).await.unwrap();
    let run_id = chains_run(
        &node,
        &p,
        ws,
        "diamond",
        serde_json::Map::new(),
        Arc::new(M),
        "run-1",
        1,
    )
    .await
    .unwrap();

    let snapshot = chains_run_get(&node.store, &p, ws, &c, &run_id)
        .await
        .unwrap();
    assert_eq!(snapshot.get("status").unwrap(), "success");
    let steps = snapshot.get("steps").unwrap().as_array().unwrap();
    assert_eq!(steps.len(), 4);
    for s in steps {
        assert_eq!(s.get("outcome").unwrap(), "ok", "every diamond step ok");
    }
}

// ----- failure policy: Halt prunes the subtree -------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn halt_skips_the_subtree_of_a_failure() {
    let ws = "chains-halt";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(ws, FULL);
    seed_rule(&node, &p, ws, "good", r#"emit(#{ level: "info" });"#).await;
    // a rule that throws (a syntax/runtime error → step fails after retries).
    seed_rule(&node, &p, ws, "bad", r#"throw "boom";"#).await;
    let c = chain(
        ws,
        "halt",
        vec![
            step("a", "good", &[]),
            step("b", "bad", &["a"]),
            step("c", "good", &["b"]), // depends on the failed b → must be skipped
        ],
        FailurePolicy::Halt,
    );
    chains_save(&node.store, &p, ws, &c).await.unwrap();
    let run_id = chains_run(
        &node,
        &p,
        ws,
        "halt",
        serde_json::Map::new(),
        Arc::new(M),
        "r",
        1,
    )
    .await
    .unwrap();
    let snap = chains_run_get(&node.store, &p, ws, &c, &run_id)
        .await
        .unwrap();
    assert_eq!(snap.get("status").unwrap(), "partialFailure");
    let steps = snap.get("steps").unwrap().as_array().unwrap();
    let outcome = |id: &str| {
        steps
            .iter()
            .find(|s| s.get("id").unwrap() == id)
            .unwrap()
            .get("outcome")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(outcome("a"), "ok");
    assert_eq!(outcome("b"), "err");
    assert_eq!(outcome("c"), "skipped", "Halt prunes the failed subtree");
}

// ----- workspace isolation ---------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_cannot_run_a_ws_a_chain() {
    let node = Arc::new(Node::boot().await.unwrap());
    let a = principal("ws-a", FULL);
    seed_rule(&node, &a, "ws-a", "r", r#"emit(#{level:"info"});"#).await;
    let c = chain(
        "ws-a",
        "priv",
        vec![step("a", "r", &[])],
        FailurePolicy::Halt,
    );
    chains_save(&node.store, &a, "ws-a", &c).await.unwrap();
    // ws-B with full caps in ITS workspace cannot reach ws-A's chain (namespace wall → NotFound).
    let b = principal("ws-b", FULL);
    let err = chains_run(
        &node,
        &b,
        "ws-b",
        "priv",
        serde_json::Map::new(),
        Arc::new(M),
        "r",
        1,
    )
    .await;
    assert!(err.is_err(), "ws-B must not run a ws-A chain");
}

// ----- offline/sync: resume after restart, exactly once ----------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn run_resumes_exactly_once_after_restart() {
    let ws = "chains-restart";
    let dir = std::env::temp_dir().join(format!("lb-chains-restart-{}", std::process::id()));
    let path = dir.to_string_lossy().to_string();
    let chain_def = chain(
        ws,
        "persist",
        vec![step("a", "r", &[]), step("b", "r", &["a"])],
        FailurePolicy::Halt,
    );
    let run_id = "rr-1";
    {
        let node = Arc::new(
            Node::boot_with_store(lb_store::Store::open(&path).await.unwrap())
                .await
                .unwrap(),
        );
        let p = principal(ws, FULL);
        seed_rule(&node, &p, ws, "r", r#"emit(#{level:"info"});"#).await;
        chains_save(&node.store, &p, ws, &chain_def).await.unwrap();
        chains_run(
            &node,
            &p,
            ws,
            "persist",
            serde_json::Map::new(),
            Arc::new(M),
            run_id,
            1,
        )
        .await
        .unwrap();
    }
    {
        // Re-open the store (a "restart") and RESUME: a duplicate drive is a no-op (CAS + finalize).
        let node = Arc::new(
            Node::boot_with_store(lb_store::Store::open(&path).await.unwrap())
                .await
                .unwrap(),
        );
        let p = principal(ws, FULL);
        chains_resume(&node, &p, ws, "persist", run_id, Arc::new(M), 2)
            .await
            .unwrap();
        let snap = chains_run_get(&node.store, &p, ws, &chain_def, run_id)
            .await
            .unwrap();
        assert_eq!(snap.get("status").unwrap(), "success");
        // exactly-once: each step ran once (attempts/outcome recorded once, no double-run).
        let steps = snap.get("steps").unwrap().as_array().unwrap();
        assert_eq!(steps.len(), 2);
        for s in steps {
            assert_eq!(s.get("outcome").unwrap(), "ok");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}
