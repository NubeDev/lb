//! DAG validation + binding resolution — port rubix-cube's tests. Cycle / dangling / duplicate /
//! self-edge / size cap rejected before any step runs; a valid diamond schedules; `${...}` references
//! substitute by value; embedded `${` is a literal; a missing/failed upstream resolves to null.

use lb_rules::workflow::{Chain, DagError, Outcome, RunContext, Step, StepRecord};
use lb_rules::{Finding, RuleOutput};

fn step(id: &str, needs: &[&str]) -> Step {
    Step {
        id: id.into(),
        rule: format!("rule-{id}"),
        needs: needs.iter().map(|s| s.to_string()).collect(),
        with: serde_json::Map::new(),
        retry: None,
    }
}

fn chain(steps: Vec<Step>) -> Chain {
    Chain {
        workspace: "acme".into(),
        id: "c1".into(),
        name: "c".into(),
        trigger: Default::default(),
        params: serde_json::Map::new(),
        steps,
        failure_policy: Default::default(),
    }
}

#[test]
fn diamond_is_valid_and_has_one_frontier() {
    let c = chain(vec![
        step("a", &[]),
        step("b", &["a"]),
        step("c", &["a"]),
        step("d", &["b", "c"]),
    ]);
    c.validate(100).unwrap();
    assert_eq!(c.frontier(), vec!["a".to_string()]);
}

#[test]
fn cycle_is_rejected() {
    let c = chain(vec![step("a", &["b"]), step("b", &["a"])]);
    assert_eq!(c.validate(100), Err(DagError::Cycle));
}

#[test]
fn dangling_dependency_is_rejected() {
    let c = chain(vec![step("a", &["ghost"])]);
    assert!(matches!(
        c.validate(100),
        Err(DagError::UnknownDependency(_, _))
    ));
}

#[test]
fn duplicate_id_is_rejected() {
    let c = chain(vec![step("a", &[]), step("a", &[])]);
    assert!(matches!(c.validate(100), Err(DagError::DuplicateStep(_))));
}

#[test]
fn self_edge_is_rejected() {
    let c = chain(vec![step("a", &["a"])]);
    assert!(matches!(c.validate(100), Err(DagError::SelfDependency(_))));
}

#[test]
fn size_cap_is_rejected() {
    let c = chain(vec![step("a", &[]), step("b", &[]), step("c", &[])]);
    assert!(matches!(c.validate(2), Err(DagError::TooManySteps(3, 2))));
}

#[test]
fn empty_is_rejected() {
    let c = chain(vec![]);
    assert_eq!(c.validate(100), Err(DagError::Empty));
}

#[test]
fn binding_resolves_param_and_step_output_by_value() {
    let mut params = serde_json::Map::new();
    params.insert("threshold".into(), serde_json::json!(5.0));
    let mut ctx = RunContext::new(params);
    ctx.record(
        "upstream".into(),
        StepRecord {
            outcome: Outcome::Ok(RuleOutput::Scalar(serde_json::json!(42)), vec![]),
            attempts: 1,
            ms: 10,
        },
    );

    let mut with = serde_json::Map::new();
    with.insert("t".into(), serde_json::json!("${params.threshold}"));
    with.insert("u".into(), serde_json::json!("${steps.upstream.output}"));
    with.insert("lit".into(), serde_json::json!("plain"));
    with.insert(
        "embedded".into(),
        serde_json::json!("a ${params.threshold} b"),
    );

    let resolved = ctx.resolve_bindings(&with).unwrap();
    assert_eq!(resolved.get("t").unwrap().as_float().unwrap(), 5.0);
    assert_eq!(resolved.get("u").unwrap().as_int().unwrap(), 42);
    assert_eq!(
        resolved.get("lit").unwrap().clone().into_string().unwrap(),
        "plain"
    );
    // embedded ${ is NOT a reference — passes through as a literal string
    assert_eq!(
        resolved
            .get("embedded")
            .unwrap()
            .clone()
            .into_string()
            .unwrap(),
        "a ${params.threshold} b"
    );
}

#[test]
fn failed_upstream_resolves_to_null() {
    let mut ctx = RunContext::new(serde_json::Map::new());
    ctx.record(
        "bad".into(),
        StepRecord {
            outcome: Outcome::Err("boom".into()),
            attempts: 2,
            ms: 1,
        },
    );
    let mut with = serde_json::Map::new();
    with.insert("x".into(), serde_json::json!("${steps.bad.output}"));
    let resolved = ctx.resolve_bindings(&with).unwrap();
    assert!(resolved.get("x").unwrap().is_unit());
}

#[test]
fn step_findings_reference_resolves() {
    let mut ctx = RunContext::new(serde_json::Map::new());
    ctx.record(
        "f".into(),
        StepRecord {
            outcome: Outcome::Ok(
                RuleOutput::Findings,
                vec![Finding {
                    level: "info".into(),
                    data: serde_json::json!({ "msg": "hi" }),
                }],
            ),
            attempts: 1,
            ms: 1,
        },
    );
    let mut with = serde_json::Map::new();
    with.insert("ff".into(), serde_json::json!("${steps.f.findings}"));
    let resolved = ctx.resolve_bindings(&with).unwrap();
    assert!(resolved.get("ff").unwrap().is_array());
}
