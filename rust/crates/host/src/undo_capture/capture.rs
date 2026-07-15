//! Auto-capture a dispatched tool call into the undo journal — the dispatch-seam half of the undo
//! scope ("classification is runtime transaction taint, not trusted dispatch metadata").
//!
//! Wraps the actual dispatch in a runtime **taint scope** and, after it runs, journals the call:
//!   - **tainted (reached the outbox)** → not-undoable, classified by [`lb_undo::classify`] from
//!     the taint (`irreversible`, or `compensable` if a compensation is declared). The taint wins
//!     over the static plan, so a verb planned `Reversible` whose *nested* call reached the outbox
//!     is journaled irreversible — the composition `max` rule enforced from what actually ran.
//!   - **reversible & single-record-capturable** → an undoable before-image entry via
//!     [`lb_undo::record_captured`] (snapshot taken *before* the call, after-image read after).
//!   - **mutated but non-generic** (wrote the store, no nameable touched set) → a not-undoable
//!     marker, per the scope's hard rule (never a partial raw restore).
//!   - **no mutation at all** → nothing journaled (a pure read).
//!
//! Nested host-callback calls share the enclosing taint scope (same task), so this wrapper is
//! applied only at the **outermost** dispatch entry; deeper hops just contribute taint upward.

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_store::{read_versioned, taint_scope, Store, TaintVerdict};
use lb_undo::{
    classify, record_captured, record_irreversible, Class, RecordCaptured, RecordIrreversible,
};
use serde_json::Value;

use super::decide::{decide, BeforeRead, Decision};
use super::plan::{plan_capture, CapturePlan};

/// Run `call` (the real dispatch future) under a taint scope and auto-journal it. `out` is returned
/// unchanged; a journaling failure is swallowed (capture is best-effort — it must never turn a
/// successful tool call into a failure, scope "failure direction"). `group` threads a batch/job id
/// through for grouped-undo groundwork (`None` = standalone step).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn capture_dispatch<F>(
    store: &Store,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
    group: Option<String>,
    declared_compensation: Option<&str>,
    call: F,
) -> Result<String, ToolError>
where
    F: std::future::Future<Output = Result<String, ToolError>>,
{
    let plan = plan_capture(qualified_tool, input);

    // Snapshot the before-image FIRST for a capturable reversible plan (we cannot read it after the
    // tool has overwritten the record). A read ERROR is kept distinct from a successful absent read
    // — `decide` maps it to not-undoable, never to a journaled "absent" (which a later undo would
    // restore by deleting real data; see `decide.rs`).
    let before = match &plan {
        CapturePlan::Reversible { table, id } => Some((
            table.clone(),
            id.clone(),
            match read_versioned(store, ws, table, id).await {
                Ok(v) => BeforeRead::Read(v),
                Err(_) => BeforeRead::Failed,
            },
        )),
        _ => None,
    };

    let (result, verdict) = taint_scope(call).await;

    // Only journal a call that actually succeeded — a failed call mutated nothing durable worth an
    // undo entry (and a rolled-back tx left no record).
    let out = match result {
        Ok(out) => out,
        Err(e) => return Err(e),
    };

    journal(
        store,
        principal,
        ws,
        qualified_tool,
        group,
        declared_compensation,
        plan,
        before,
        verdict,
    )
    .await;

    Ok(out)
}

#[allow(clippy::too_many_arguments)]
async fn journal(
    store: &Store,
    principal: &Principal,
    ws: &str,
    tool: &str,
    group: Option<String>,
    declared_compensation: Option<&str>,
    plan: CapturePlan,
    before: Option<(String, String, BeforeRead)>,
    verdict: TaintVerdict,
) {
    let actor = principal.sub();
    // Logical ts: no wall-clock in core (same discipline as the other host verbs). 0 is fine; the
    // journal's ordering is the monotonic `seq`, not `ts`.
    let ts = 0;
    let trace_id = "";

    // Split the snapshot into the target (table, id) and the read outcome the decision consumes.
    let (target, before_read) = match before {
        Some((table, id, read)) => (Some((table, id)), Some(read)),
        None => (None, None),
    };

    // The whole outcome table lives in `decide` (pure, unit-tested): taint wins; a read ERROR is
    // not-undoable, never "absent"; a non-generic mutation gets a marker; a pure read journals
    // nothing.
    match decide(
        &plan,
        before_read,
        verdict.reached_outbox,
        verdict.wrote_store,
    ) {
        Decision::Tainted => {
            let class = classify(true, declared_compensation);
            let _ = record_irreversible(
                store,
                RecordIrreversible {
                    ws,
                    actor,
                    surface: "",
                    tool,
                    trace_id,
                    ts,
                    class,
                    group,
                    depth_cap: None,
                },
            )
            .await;
        }
        Decision::Undoable { before, before_rev } => {
            if let Some((table, id)) = target {
                let _ = record_captured(
                    store,
                    RecordCaptured {
                        ws,
                        actor,
                        surface: "",
                        tool,
                        trace_id,
                        ts,
                        table: &table,
                        id: &id,
                        before,
                        before_rev,
                        group,
                        depth_cap: None,
                    },
                )
                .await;
            }
        }
        // A real mutation with no safe before-image (non-generic, or the before read FAILED): a
        // not-undoable marker, so undo refuses honestly instead of restoring an unobserved state.
        Decision::NotUndoable => {
            let _ = record_irreversible(
                store,
                RecordIrreversible {
                    ws,
                    actor,
                    surface: "",
                    tool,
                    trace_id,
                    ts,
                    class: Class::Irreversible,
                    group,
                    depth_cap: None,
                },
            )
            .await;
        }
        Decision::Nothing => {}
    }
}
