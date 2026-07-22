//! `pack.apply` — the verb: gate, resolve, lint, decide, drive, receipt.
//!
//! The order is the whole contract, and each step exists to stop a specific bad outcome:
//!   1. **gate** `mcp:pack.apply:call` — before anything is read or parsed;
//!   2. **resolve** the bundle — a missing referenced file is loud, never a silent skip;
//!   3. **lint** — an ERROR gates (a self-inconsistent pack must not half-apply); warnings ride along;
//!   4. **decide** the refusal matrix against the prior receipt — this is where idempotence,
//!      "bump the version", and the partial-recovery re-apply live;
//!   5. **drive** the plan through the internal seams, each re-checking its own capability;
//!   6. **receipt**, always — including a partial, which IS the recovery signal.

use std::sync::Arc;

use lb_auth::Principal;
use lb_packs::{content_checksum, decide, plan as build_plan, validate as lint, Bundle};
use serde_json::{json, Value};

use super::apply::{apply_plan, finish, resolve_decision};
use super::authorize::authorize_pack;
use super::error::PackError;
use super::store::read_receipt;
use crate::boot::Node;

/// Apply `bundle` to `ws` as `principal`, stamping `ts` as the logical apply time.
pub async fn pack_apply(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    bundle: &Bundle,
    ts: u64,
) -> Result<Value, PackError> {
    authorize_pack(principal, ws, "pack.apply")?;

    let pack = bundle.resolve().map_err(PackError::BadInput)?;
    let plan = build_plan(&pack);

    let findings = lint(&pack, &plan);
    let errors: Vec<String> = findings
        .iter()
        .filter(|f| f.error)
        .map(|f| f.message.clone())
        .collect();
    if !errors.is_empty() {
        return Err(PackError::Invalid(errors));
    }

    let checksum = content_checksum(&pack);
    let prior = read_receipt(&node.store, ws, &pack.manifest.pack)
        .await
        .map_err(|e| PackError::Internal(format!("reading receipt: {e}")))?;

    let decision = decide(pack.manifest.version, &checksum, prior.as_ref());
    let Some(run) = resolve_decision(decision, prior.is_some())? else {
        // The idempotent no-op: same version, same content, nothing partial. Change nothing, and
        // say so — a caller must be able to tell "already applied" from "just applied".
        return Ok(json!({
            "pack": pack.manifest.pack,
            "version": pack.manifest.version,
            "outcome": "noop",
            "manifest_checksum": checksum,
        }));
    };

    let applied = apply_plan(
        node,
        principal,
        ws,
        &pack,
        &plan,
        run.run_rules,
        run.clobbering,
        run.upgrade,
        ts,
    )
    .await;

    let receipt = finish(node, ws, &pack, checksum.clone(), &plan, &applied, ts).await?;

    Ok(json!({
        "pack": pack.manifest.pack,
        "version": pack.manifest.version,
        "outcome": if receipt.is_complete() { "applied" } else { "partial" },
        // An upgrade is a distinct, loud outcome — the caller learns "vN → vM", not just "applied".
        "upgraded": run.version_bump.map(|(from, to)| json!({ "from": from, "to": to })),
        "manifest_checksum": checksum,
        "ran_rules": run.run_rules,
        // Every pack-owned object this run overwrote. Loud by contract — an admin who tuned a
        // dashboard or the agent context learns exactly what the re-apply cost.
        "clobbered": applied.clobbered,
        "warnings": applied.warnings,
        "objects": receipt.objects.iter().map(|o| json!({
            "kind": o.kind,
            "id": o.id,
            "checksum": o.checksum,
            "outcome": o.outcome,
        })).collect::<Vec<_>>(),
    }))
}
