//! `pack.validate` — the dry run. Parse the bundle, derive the plan, lint it, and hand back what an
//! apply WOULD do. This IS the CI validator a pack author runs, and the `--dry-run` a caller runs
//! before committing to a write.
//!
//! Read-tier: it touches no object and needs no workspace state beyond the caller's own receipt (to
//! report what a re-apply would decide). Errors gate; dialect warnings do not.

use lb_auth::Principal;
use lb_packs::{content_checksum, decide, plan as build_plan, validate as lint, Bundle, Decision};
use lb_store::Store;
use serde_json::{json, Value};

use super::authorize::authorize_pack;
use super::error::PackError;
use super::store::read_receipt;

/// Validate `bundle` against `ws`'s current state. Returns the plan, the checksum, the lint
/// findings, and what a `pack.apply` of this bundle would decide right now.
pub async fn pack_validate(
    store: &Store,
    principal: &Principal,
    ws: &str,
    bundle: &Bundle,
) -> Result<Value, PackError> {
    authorize_pack(principal, ws, "pack.validate")?;

    let pack = bundle.resolve().map_err(PackError::BadInput)?;
    let plan = build_plan(&pack);
    let findings = lint(&pack, &plan);
    let checksum = content_checksum(&pack);

    // What would an apply do? The caller's own receipt decides — a read, so `pack.validate` stays
    // read-tier while still answering the question an author actually has.
    let prior = read_receipt(store, ws, &pack.manifest.pack)
        .await
        .map_err(|e| PackError::Internal(format!("reading receipt: {e}")))?;
    let decision = decide(pack.manifest.version, &checksum, prior.as_ref());

    Ok(json!({
        "pack": pack.manifest.pack,
        "title": pack.manifest.title,
        "version": pack.manifest.version,
        "manifest_checksum": checksum,
        "valid": !findings.iter().any(|f| f.error),
        "decision": decision_label(&decision),
        "reason": match &decision {
            Decision::Refuse(why) => Value::String(why.clone()),
            _ => Value::Null,
        },
        "findings": findings.iter().map(|f| json!({
            "severity": if f.error { "error" } else { "warning" },
            "message": f.message,
        })).collect::<Vec<_>>(),
        "plan": plan.iter().map(|o| json!({
            "kind": o.kind.as_str(),
            "id": o.id,
            "checksum": o.checksum,
        })).collect::<Vec<_>>(),
    }))
}

pub(super) fn decision_label(d: &Decision) -> &'static str {
    match d {
        Decision::Apply { .. } => "apply",
        Decision::NoOp => "noop",
        Decision::Refuse(_) => "refuse",
    }
}
