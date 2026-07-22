//! `pack.apply` — the orchestration: decide, then drive every planned object through the SAME
//! internal seam the equivalent public verb calls, then write the receipt.
//!
//! **Authority.** Holding `mcp:pack.apply:call` gets you into this function and nothing more. Each
//! object below calls the very function `rules.save` / `dashboard.save` / `datasource.add` /
//! `channel.create` / `agent.memory.set` / `nav.hidden.set` call, and each of those re-runs its own
//! capability check under the caller's principal. A caller who cannot `rules.save` gets `denied` on
//! the rule objects and an honest partial receipt — there is no privileged path through a pack
//! (pack-core-scope §Caps: "a caller who couldn't `rules.save` can't smuggle a rule in via a pack";
//! likewise a caller who cannot `nav.save` cannot hide a surface via a pack's `sidebar` block).
//!
//! **Not a transaction.** Partial failure is a first-class outcome recorded in the receipt, not an
//! abort-and-rollback. The documented recovery is "grant the cap, re-run", which the refusal matrix
//! makes safe: a partial receipt at the same version re-applies.
//!
//! **Loud clobber.** A re-apply overwrites pack-owned objects, and every overwrite is listed in the
//! result. The agent context is the sharpest edge and is never silent.
//!
//! **Rules run on first apply only** — the receipt knows. Idempotence must not depend on every
//! rule's dedup key.
//!
//! Rule 10: nothing here names a pack. Every branch is on an object KIND, which is data.

use std::sync::Arc;

use lb_auth::Principal;
use lb_packs::{Decision, Kind, Pack, PlannedObject, Receipt, RetentionPolicy};
use serde_json::Value;

use super::error::PackError;
use super::store::write_receipt;
use crate::boot::Node;

/// The outcome of one object, as recorded in the receipt.
const APPLIED: &str = "applied";
const DENIED: &str = "denied";
const FAILED: &str = "failed";

/// The result of an apply — what happened, per object, plus what it clobbered.
pub struct Applied {
    /// Parallel to the plan.
    pub outcomes: Vec<String>,
    /// Every object this run overwrote (`kind:id`), for the loud-clobber listing.
    pub clobbered: Vec<String>,
    /// Non-fatal notes (a rule that failed to run, a missing required extension).
    pub warnings: Vec<String>,
}

/// Drive the plan. `run_rules` comes from the [`Decision`]; `clobbering` is true when a prior
/// receipt existed (so the caller is told what the re-apply cost).
#[allow(clippy::too_many_arguments)]
pub async fn apply_plan(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    pack: &Pack,
    plan: &[PlannedObject],
    run_rules: bool,
    clobbering: bool,
    upgrade: bool,
    ts: u64,
) -> Applied {
    let mut outcomes = Vec::with_capacity(plan.len());
    let mut clobbered = Vec::new();
    let mut warnings = Vec::new();

    // Required extensions are CHECKED, never installed — installing is the admin's act; the pack
    // only declares needs. A missing one warns and never blocks (the operator may install and the
    // pack's surfaces light up later).
    for ext in &pack.manifest.extensions {
        if !extension_present(node, principal, ws, ext).await {
            warnings.push(format!(
                "required extension '{ext}' is not installed — the pack applied, but the surfaces \
                 that need it stay dark until an admin installs it"
            ));
        }
    }

    for obj in plan {
        if clobbering {
            clobbered.push(format!("{}:{}", obj.kind.as_str(), obj.id));
        }
        let outcome = apply_object(
            node,
            principal,
            ws,
            pack,
            obj,
            run_rules,
            upgrade,
            ts,
            &mut warnings,
        )
        .await;
        outcomes.push(outcome);
    }

    Applied {
        outcomes,
        clobbered,
        warnings,
    }
}

/// Apply one object through its seam. Every arm maps a seam error to the receipt's outcome
/// vocabulary: a capability refusal is `denied` (the recoverable partial), anything else is
/// `failed`. Neither aborts the run — the receipt records the whole picture.
#[allow(clippy::too_many_arguments)]
async fn apply_object(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    pack: &Pack,
    obj: &PlannedObject,
    run_rules: bool,
    upgrade: bool,
    ts: u64,
    warnings: &mut Vec<String>,
) -> String {
    match obj.kind {
        // `run_rules` is the FIRST-APPLY signal (true iff no prior receipt). Seeded rows are starting
        // data — applied once, never re-clobbered — so the datasource seeds on first apply OR an
        // upgrade (into EMPTY store tables only; the per-table guard preserves seed-ownership), the
        // same run-once model as rules (pack-entity-binding-scope.md §seed-ownership). A plain re-apply
        // leaves the operator's rows intact; an UPGRADE additionally reconciles the schema additively
        // AND runs the sqlite→store migration (`migrate_from`) — the in-place version-bump path.
        Kind::Datasource => {
            apply_datasource(node, principal, ws, pack, run_rules, upgrade, ts, warnings).await
        }
        Kind::Rule => apply_rule(node, principal, ws, pack, &obj.id, run_rules, ts, warnings).await,
        Kind::Dashboard => apply_dashboard(node, principal, ws, pack, &obj.id, ts).await,
        Kind::Channel => apply_channel(node, principal, ws, &obj.id, ts).await,
        Kind::Agent => apply_agent(node, principal, ws, pack, ts).await,
        Kind::Sidebar => apply_sidebar(node, principal, ws, pack, ts).await,
        Kind::Retention => apply_retention(node, principal, ws, pack, &obj.id).await,
    }
}

#[allow(clippy::too_many_arguments)]
async fn apply_datasource(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    pack: &Pack,
    first_apply: bool,
    upgrade: bool,
    ts: u64,
    warnings: &mut Vec<String>,
) -> String {
    let Some(ds) = &pack.manifest.datasource else {
        return FAILED.to_string();
    };

    // STORE SEED (pack-store-datasource-scope.md): a pack's entity rows live in the ONE application
    // store, not a sqlite file. `seed_rows` (structured `{table: [rows]}`, O-1) is UPSERT'd as
    // SurrealDB records at `{table}:{pk}` through the same `store.write` verb every other workspace
    // record rides — Data-browser-visible, graph-linkable, caps-scopable. This is INDEPENDENT of the
    // engine below: a pack may seed entity tables into the store AND keep a sqlite/federation
    // datasource for append-heavy time-series (`point_reading`) in the same manifest (the scope's
    // "entities in the store, time-series in federation" line).
    //
    // WHEN THIS RUNS: first apply OR an upgrade. The upgrade case is REQUIRED — a workspace that
    // applied a sqlite-entity vN and bumps to a store-backed vN+1 must have its `migrate_from` sqlite
    // rows carried into the store ON THE BUMP (an in-place upgrade has a prior receipt, so
    // `first_apply`/`run_rules` is false there — gating on it alone silently skips the one real
    // migration path). Seed-ownership still holds WITHOUT that gate: both the migration and the seed
    // only ever write into an EMPTY store table (`store_table_empty` per table), so a later
    // upgrade over already-owned tables is a safe no-op and a plain re-apply never enters here.
    let seed_this_run = first_apply || upgrade;
    if seed_this_run && (!pack.seed_rows.is_empty() || pack.manifest.migrate_from.is_some()) {
        // The pk COLUMN for a store-seed table comes from the entity binding that names it (rule 10:
        // the seed reads the binding, never a table name it special-cases). An entity is bound to a
        // table iff `entity.table == table`; its `pk` is the id column.
        let pk_for = |table: &str| -> Option<String> {
            pack.manifest
                .entities
                .values()
                .find(|e| e.table.as_deref() == Some(table))
                .and_then(|e| e.pk.clone())
        };

        // MIGRATION (pack-store-datasource-scope.md §Migration) — BEFORE the seed. A pack that names a
        // prior sqlite `migrate_from` datasource carries the operator's LIVE sqlite entity rows into
        // the store (into empty store tables only — never clobbering), so a workspace on the old
        // sqlite era keeps its edits. It runs before the seed so an operator-edited row wins over the
        // fresh seed for the same id (the seed UPSERT would otherwise overwrite it — but a migrated,
        // now-non-empty store table's rows are the operator's, and the seed re-UPSERT at the same id
        // is idempotent to the migrated value only if unchanged; the ordering keeps operator intent).
        if let Some(old_ds) = &pack.manifest.migrate_from {
            // The store-bound (table, pk) pairs the migration sweeps — the same bindings the seed uses.
            let pairs: Vec<(String, String)> = pack
                .manifest
                .entities
                .values()
                .filter_map(|e| match (&e.table, &e.pk) {
                    (Some(t), Some(pk)) => Some((t.clone(), pk.clone())),
                    _ => None,
                })
                .collect();
            let sqlite_path = super::sqlite::db_path(ws, &pack.manifest.pack, old_ds);
            match super::store_seed::migrate_sqlite_entities(
                node,
                principal,
                ws,
                &sqlite_path,
                &pairs,
            )
            .await
            {
                Ok(migrated) => {
                    let total: usize = migrated.iter().map(|(_, n)| n).sum();
                    if total > 0 {
                        warnings.push(format!(
                            "migrated {total} entity row(s) from the prior sqlite '{old_ds}' into the \
                             store — {}",
                            migrated
                                .iter()
                                .map(|(t, n)| format!("{t}:{n}"))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    }
                }
                Err(PackError::Denied) => return DENIED.to_string(),
                // A failed migration leaves the sqlite file in place (no half-move) — but it IS a
                // failure of the datasource object (the operator's data did not move), so surface it.
                Err(e) => {
                    warnings.push(format!("sqlite→store migration failed: {e}"));
                    return FAILED.to_string();
                }
            }
        }

        match super::store_seed::seed_rows(node, principal, ws, &pack.seed_rows, pk_for).await {
            Ok(_) => {}
            // A missing `store:<table>:write` on the applier is the recoverable partial — no pack
            // grants a smuggled write. Anything else (a bad seed row) is a hard failure.
            Err(PackError::Denied) => return DENIED.to_string(),
            Err(_) => return FAILED.to_string(),
        }
    }

    // A pure store pack (engine `store`, no sqlite schema/seed) has NO external source to register —
    // its rows ARE the store. Return applied once the seed lands; there is no `datasource_add`.
    if ds.engine == "store" {
        return APPLIED.to_string();
    }

    // Only sqlite is materialized node-locally (see `sqlite.rs` for the tradeoff); every other
    // engine registers as a pointer to a source the operator already stood up.
    //
    // SEED OWNERSHIP (pack-entity-binding-scope.md): the seed is run-once. On FIRST apply we build
    // the db fresh (schema + seed). On a re-apply the db already exists and its rows may be operator-
    // edited — so we DO NOT rebuild or re-seed; we resolve the existing file and re-register it. This
    // is what makes "an operator CRUDs the seeded sites, then the pack ships v4" safe: the data is the
    // operator's, and re-applying the pack never clobbers it.
    //
    // UPGRADE (pack-upgrade-scope.md): a version bump additionally reconciles the schema ADDITIVELY —
    // any table/column the new `schema.sql` declares that the live db lacks is created / added
    // (nullable, empty on existing rows). This is what lets a pack EVOLVE without abandoning the
    // operator's workspace. Additive-only by construction (v1 first step): a removed/retyped column is
    // NOT dropped (the safe direction) — a destructive migration stays an explicit future act.
    let dsn = if ds.engine == "sqlite" && (pack.schema_sql.is_some() || pack.seed_sql.is_some()) {
        // Re-apply resolves the existing db (preserving operator rows); a first apply — or a re-apply
        // whose db was purged (`resolve_existing` → None) — builds it fresh from the authored SQL.
        let existing = if first_apply {
            None
        } else {
            match super::sqlite::resolve_existing(ws, &pack.manifest.pack, &ds.name) {
                Ok(p) => p,
                Err(_) => return FAILED.to_string(),
            }
        };
        let path = match existing {
            Some(p) => Ok(p),
            None => super::sqlite::materialize(
                ws,
                &pack.manifest.pack,
                &ds.name,
                pack.schema_sql.as_deref(),
                pack.seed_sql.as_deref(),
            ),
        };
        let path = match path {
            Ok(p) => p,
            Err(_) => return FAILED.to_string(),
        };
        // On an upgrade of an EXISTING db, bring its schema up to the new `schema.sql` additively.
        // (A fresh materialize already ran the new schema, so this only matters when `existing` was
        // Some — but `reconcile_schema` is a no-op when nothing is missing, so calling it either way
        // is safe and keeps the branch simple.)
        if upgrade {
            if let Some(schema) = pack.schema_sql.as_deref() {
                match super::sqlite::reconcile_schema(&path, schema) {
                    Ok(added) if !added.is_empty() => warnings.push(format!(
                        "datasource '{}' upgraded: added {} column/table(s) — {}",
                        ds.name,
                        added.len(),
                        added.join(", ")
                    )),
                    Ok(_) => {}
                    Err(e) => {
                        warnings.push(format!(
                            "datasource '{}' schema reconcile failed: {e}",
                            ds.name
                        ));
                        return FAILED.to_string();
                    }
                }
            }
        }
        Some(path.to_string_lossy().into_owned())
    } else {
        None
    };

    // `127.0.0.1:0` is the node-local convention for a file-backed source — there is no network
    // endpoint to reach, and the `net:*` grant is satisfied by the loopback entry.
    match crate::federation::datasource_add(
        node,
        principal,
        ws,
        &ds.name,
        &ds.engine,
        "127.0.0.1:0",
        None,
        dsn.as_deref(),
        ts,
    )
    .await
    {
        Ok(()) => APPLIED.to_string(),
        Err(e) if is_denied(&format!("{e:?}")) => DENIED.to_string(),
        Err(_) => FAILED.to_string(),
    }
}

#[allow(clippy::too_many_arguments)]
async fn apply_rule(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    pack: &Pack,
    id: &str,
    run: bool,
    ts: u64,
    warnings: &mut Vec<String>,
) -> String {
    let Some(rule) = pack.rules.iter().find(|r| r.id == id) else {
        return FAILED.to_string();
    };

    match crate::rules::rules_save(
        &node.store,
        principal,
        ws,
        &rule.id,
        &rule.name,
        &rule.body,
        Vec::new(),
    )
    .await
    {
        Ok(_) => {}
        Err(e) if is_denied(&format!("{e:?}")) => return DENIED.to_string(),
        Err(_) => return FAILED.to_string(),
    }

    // Rules run on first apply only, so "second run changes nothing" never depends on a dedup key.
    // A run failure is NOT fatal to the object — the rule is saved, and that is what the object is.
    if run {
        if let Err(e) = super::run::run_saved_rule(node, principal, ws, &rule.id, ts).await {
            warnings.push(format!("rule '{}' saved but did not run: {e}", rule.id));
        }
    }
    APPLIED.to_string()
}

async fn apply_dashboard(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    pack: &Pack,
    id: &str,
    ts: u64,
) -> String {
    let Some(d) = pack.dashboards.iter().find(|d| d.id == id) else {
        return FAILED.to_string();
    };
    let title = d
        .json
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or(id)
        .to_string();
    let cells = match d.json.get("cells").cloned() {
        Some(c) => serde_json::from_value(c).unwrap_or_default(),
        None => Vec::new(),
    };
    let variables = match d.json.get("variables").cloned() {
        Some(v) => serde_json::from_value(v).unwrap_or_default(),
        None => Vec::new(),
    };

    match crate::dashboard::dashboard_save_meta(
        &node.store,
        principal,
        ws,
        id,
        &title,
        d.json
            .get("description")
            .and_then(Value::as_str)
            .map(String::from),
        d.json.get("icon").and_then(Value::as_str).map(String::from),
        d.json
            .get("color")
            .and_then(Value::as_str)
            .map(String::from),
        d.json
            .get("timezone")
            .and_then(Value::as_str)
            .map(String::from),
        None,
        cells,
        variables,
        ts,
    )
    .await
    {
        Ok(_) => APPLIED.to_string(),
        Err(e) if is_denied(&format!("{e:?}")) => DENIED.to_string(),
        Err(_) => FAILED.to_string(),
    }
}

async fn apply_channel(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    id: &str,
    ts: u64,
) -> String {
    match crate::channel_registry::channel_create(&node.store, principal, ws, id, ts).await {
        Ok(_) => APPLIED.to_string(),
        Err(e) if is_denied(&format!("{e:?}")) => DENIED.to_string(),
        Err(_) => FAILED.to_string(),
    }
}

async fn apply_retention(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    pack: &Pack,
    id: &str,
) -> String {
    // `id` is the policy's PREFIX (its plan key). Find the inline policy and set it via the SAME
    // `series.retention.set` the public verb dispatches to — which re-checks
    // `mcp:series.retention.set:call` under the caller's principal (pack-core §Caps: no privileged
    // path through a pack). Pure LWW upsert, so it applies on every run; no run-once gate.
    let Some(policy) = pack.manifest.retention.iter().find(|p| p.prefix == id) else {
        return FAILED.to_string();
    };
    match crate::ingest::series_retention_set(&node.store, principal, ws, &to_ingest_policy(policy))
        .await
    {
        Ok(_) => APPLIED.to_string(),
        Err(crate::ingest::IngestError::Denied) => DENIED.to_string(),
        Err(_) => FAILED.to_string(),
    }
}

/// Convert the pack manifest's [`RetentionPolicy`] into the ingest [`lb_ingest::Policy`] the setter
/// takes. The two share field names by design (the manifest struct is the verb's mirror), so this is
/// a field-for-field move — the one place a shape drift between them would surface.
fn to_ingest_policy(p: &RetentionPolicy) -> lb_ingest::Policy {
    lb_ingest::Policy {
        prefix: p.prefix.clone(),
        raw_for_ms: p.raw_for_ms,
        max_samples: p.max_samples,
        tiers: p
            .tiers
            .iter()
            .map(|t| lb_ingest::Tier {
                width_ms: t.width_ms,
                keep_for_ms: t.keep_for_ms,
            })
            .collect(),
    }
}

async fn apply_agent(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    pack: &Pack,
    ts: u64,
) -> String {
    let Some(ctx) = &pack.agent_context else {
        return FAILED.to_string();
    };
    // The pack's domain context is durable, workspace-shared agent MEMORY — the shipped home for
    // authored domain facts (the config record carries runtime/endpoint only, no prose). One fact
    // per pack, keyed by a stable slug, so a re-apply upserts rather than duplicates. Workspace
    // scope is admin-gated, which matches an applier's identity; a member token is denied and
    // recorded as a partial. This is the sharpest clobber edge — announced before it is written.
    let slug = format!("pack-{}-context", pack.manifest.pack);
    let description = format!("Domain context for the '{}' pack", pack.manifest.title);
    match crate::agent::memory_set(
        &node.store,
        principal,
        ws,
        Some("workspace"),
        &slug,
        &description,
        "reference",
        ctx,
        ts,
    )
    .await
    {
        Ok(_) => APPLIED.to_string(),
        Err(lb_mcp::ToolError::Denied) => DENIED.to_string(),
        Err(_) => FAILED.to_string(),
    }
}

async fn apply_sidebar(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    pack: &Pack,
    ts: u64,
) -> String {
    let Some(sidebar) = &pack.manifest.sidebar else {
        return FAILED.to_string();
    };
    // The workspace hidden-set is full-set LWW: the pack's declared refs REPLACE whatever is set,
    // so a re-apply clobbers loudly (the object is listed in the run's clobber section — the same
    // contract as the dashboard/agent objects). The refs are opaque data passed straight through;
    // `nav_hidden_set` re-checks `nav.save` under the caller's principal, so a caller who cannot
    // shape the workspace's menus by hand cannot hide via a pack either — no privileged path
    // (pack-core-scope §Caps). Declutter, never authz: hiding blocks no route.
    match crate::nav::nav_hidden_set(&node.store, principal, ws, sidebar.hidden.clone(), ts).await {
        Ok(_) => APPLIED.to_string(),
        Err(crate::nav::NavError::Denied) => DENIED.to_string(),
        Err(_) => FAILED.to_string(),
    }
}

/// Is this extension installed in `ws`? Best-effort — an unreadable list degrades to "present" so a
/// discovery hiccup never blocks an apply.
async fn extension_present(node: &Arc<Node>, principal: &Principal, ws: &str, ext: &str) -> bool {
    match crate::ext::ext_list(node, principal, ws).await {
        Ok(list) => list.iter().any(|e| e.ext == ext),
        // `ext.list` is admin-only. A non-admin applier legitimately cannot enumerate extensions,
        // and a discovery hiccup must never block an apply — so an unreadable list degrades to
        // "assume present" rather than emitting a warning the caller cannot act on.
        Err(_) => true,
    }
}

/// Every seam error type is distinct, and every one of them collapses a capability refusal to a
/// `Denied` variant. Matching on the debug string keeps this one small function generic over all of
/// them rather than five near-identical match arms — the outcome vocabulary is what matters here.
fn is_denied(debug: &str) -> bool {
    debug.starts_with("Denied") || debug == "Denied"
}

/// Build the receipt and persist it. Written even for a partial apply — the partial IS the recovery
/// signal, and a receipt that only appeared on success would strand "grant the cap, re-run".
pub async fn finish(
    node: &Arc<Node>,
    ws: &str,
    pack: &Pack,
    manifest_checksum: String,
    plan: &[PlannedObject],
    applied: &Applied,
    ts: u64,
) -> Result<Receipt, PackError> {
    let receipt = Receipt::from_outcomes(pack, manifest_checksum, ts, plan, &applied.outcomes);
    write_receipt(&node.store, ws, &receipt)
        .await
        .map_err(|e| PackError::Internal(format!("writing receipt: {e}")))?;
    Ok(receipt)
}

/// How an apply should run, once the decision says it should run at all.
pub struct RunPlan {
    /// Run the pack's rules (true only on the very first apply).
    pub run_rules: bool,
    /// A prior receipt existed — the run clobbers pack-owned objects (loud-listed).
    pub clobbering: bool,
    /// A version bump — additionally reconcile the materialized schema additively.
    pub upgrade: bool,
    /// `Some((from, to))` on an upgrade, for the loud "upgraded pack: vN → vM" note.
    pub version_bump: Option<(u32, u32)>,
}

/// The decision → a refusal, or the [`RunPlan`] an apply runs with (`None` = a NoOp).
pub fn resolve_decision(decision: Decision, had_prior: bool) -> Result<Option<RunPlan>, PackError> {
    match decision {
        Decision::Apply { run_rules } => Ok(Some(RunPlan {
            run_rules,
            clobbering: had_prior,
            upgrade: false,
            version_bump: None,
        })),
        // An upgrade re-drives every object (rules never re-run), clobbers pack-owned objects to the
        // new version, and reconciles the schema additively.
        Decision::Upgrade { from, to } => Ok(Some(RunPlan {
            run_rules: false,
            clobbering: true,
            upgrade: true,
            version_bump: Some((from, to)),
        })),
        Decision::NoOp => Ok(None),
        Decision::Refuse(why) => Err(PackError::Refused(why)),
    }
}
