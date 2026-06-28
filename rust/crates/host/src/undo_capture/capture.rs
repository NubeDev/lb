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
    // tool has overwritten the record). For other plans there is nothing to snapshot up front.
    let before = match &plan {
        CapturePlan::Reversible { table, id } => Some((
            table.clone(),
            id.clone(),
            read_versioned(store, ws, table, id).await.ok(),
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
    before: Option<(String, String, Option<lb_store::Versioned>)>,
    verdict: TaintVerdict,
) {
    let actor = principal.sub();
    // Logical ts: no wall-clock in core (same discipline as the other host verbs). 0 is fine; the
    // journal's ordering is the monotonic `seq`, not `ts`.
    let ts = 0;
    let trace_id = "";

    // Taint wins over the static plan: anything that reached the outbox is not-undoable, classified
    // from the taint (the max-composition rule — derived, never trusted from a manifest).
    if verdict.reached_outbox {
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
            },
        )
        .await;
        return;
    }

    match plan {
        // A capturable single-record reversible mutation → an undoable before-image entry.
        CapturePlan::Reversible { .. } => {
            if let Some((table, id, before_v)) = before {
                let (before_val, before_rev) = match before_v {
                    Some(v) => (v.value, v.rev),
                    None => (None, 0),
                };
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
                        before: before_val,
                        before_rev,
                        group,
                    },
                )
                .await;
            }
        }
        // Mutated the store but we cannot generically name what it touched → not-undoable marker.
        // (Untainted, so `classify` yields Reversible; we OVERRIDE to Irreversible because the
        // record is real but not safely restorable — the honest "non-generic" floor.)
        CapturePlan::NonGeneric if verdict.wrote_store => {
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
                },
            )
            .await;
        }
        // Non-generic but wrote nothing (a pure read extension tool), or a known read verb: nothing
        // to journal.
        CapturePlan::NonGeneric | CapturePlan::NotMutating => {}
    }
}
