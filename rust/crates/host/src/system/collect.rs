//! Gather the live status of every subsystem for one workspace into a `Vec<ServiceStatus>` — the
//! shared body both verbs (`overview`/`topology`) project from. One responsibility: read raw
//! subsystem state and roll it into cards.
//!
//! The reads here are **ungated on purpose**: the caller (`system_overview`/`system_topology`) has
//! already passed the single `mcp:system.*:call` gate, exactly as `dbview` runs its admin gate once
//! and then calls the raw `lb_store::tables`. We deliberately do NOT call the gated host wrappers
//! (`ext_list`, `outbox_status`) — those re-check *their own* caps (`mcp:ext.list:call`, …), which a
//! `system.*` caller need not hold; the snapshot is one capability, not the union of every verb it
//! summarizes. Every read is namespace-bound to `ws`, so a snapshot physically cannot see another
//! workspace's state (the hard wall, §7).

use lb_assets::{list_installs, Tier};
use lb_bus::bus_stats;
use lb_outbox::{dead_lettered, delivered, pending};
use lb_store::{tables, TableCount};

use super::model::{Health, Metric, ServiceStatus};
use crate::boot::Node;
use crate::role::Role;

use super::SystemError;

/// The node posture as a stable label (config, not a code branch — §3.1).
fn role_str(role: Role) -> &'static str {
    match role {
        Role::Edge => "edge",
        Role::Hub => "hub",
        Role::Solo => "solo",
    }
}

/// Zenoh bus card — real transport liveness from the live session (peer/router counts + this node's
/// zid), NOT mere handle-presence. `Idle` when nothing else is on the mesh (a solo node with 0 peers
/// is honest, not a fault); `Ok` once it is connected to at least one peer or router.
async fn collect_bus(node: &Node) -> ServiceStatus {
    let stats = bus_stats(&node.bus).await;
    let connected = stats.peer_count + stats.router_count;
    let health = if connected > 0 {
        Health::Ok
    } else {
        Health::Idle
    };
    let detail = if connected > 0 {
        format!(
            "connected to {} peer(s) + {} router(s) on the Zenoh mesh",
            stats.peer_count, stats.router_count
        )
    } else {
        "peer session open; no other nodes on the mesh (solo)".into()
    };
    ServiceStatus {
        id: "bus".into(),
        label: "Zenoh Bus".into(),
        group: "motion".into(),
        health,
        detail,
        metrics: vec![
            Metric::new("peers", stats.peer_count.to_string()),
            Metric::new("routers", stats.router_count.to_string()),
            Metric::new("node zid", short_zid(&stats.zid)),
        ],
    }
}

/// The subsystem-specific `extra` detail blob for `system.subsystem` — the per-subsystem facts the
/// status grid has no room for. Today only `bus` has any: the live zids of the peers/routers it is
/// connected to (the detail behind its `peers`/`routers` counts). Every other subsystem returns an
/// empty object. Read from the same live session as the bus card, so the counts and the lists agree.
pub(crate) async fn collect_extra(node: &Node, id: &str) -> serde_json::Value {
    match id {
        "bus" => {
            let stats = bus_stats(&node.bus).await;
            serde_json::json!({
                "peer_zids": stats.peer_zids,
                "router_zids": stats.router_zids,
            })
        }
        _ => serde_json::json!({}),
    }
}

/// Trim a Zenoh id to a short, readable prefix for the card (the full id is long hex).
fn short_zid(zid: &str) -> String {
    if zid.len() > 12 {
        format!("{}…", &zid[..12])
    } else {
        zid.to_string()
    }
}

/// Build the per-subsystem status cards for workspace `ws`. The set is fixed (the platform always
/// has these subsystems); the numbers are live. A subsystem with nothing flowing reports `Idle`, not
/// a fault — an empty queue is healthy.
pub(crate) async fn collect_services(
    node: &Node,
    ws: &str,
) -> Result<Vec<ServiceStatus>, SystemError> {
    let tbls = tables(&node.store, ws).await?;
    let total_rows: u64 = tbls.iter().map(|t| t.count).sum();

    let mut out = Vec::new();

    // ── motion: state in flight (§3.3) ──────────────────────────────────────────────────────────
    out.push(ServiceStatus {
        id: "gateway".into(),
        label: "API / SSE Gateway".into(),
        group: "motion".into(),
        health: Health::Ok,
        detail: "HTTP + SSE ingress; derives principal + workspace from the token".into(),
        metrics: vec![Metric::new("role", role_str(node.role))],
    });
    out.push(collect_bus(node).await);

    // ── runtime: the capability + extension surface ─────────────────────────────────────────────
    let reg = node.registry.summary();
    out.push(ServiceStatus {
        id: "mcp".into(),
        label: "MCP Service".into(),
        group: "runtime".into(),
        health: Health::Ok,
        detail: "the universal contract — every capability is a host-mediated MCP tool".into(),
        metrics: vec![
            Metric::new("extensions", reg.extensions.to_string()),
            Metric::new("tools", reg.tools.to_string()),
        ],
    });
    out.push(collect_extensions(node, ws).await?);
    out.push(table_service(
        "registry",
        "Extension Registry",
        "runtime",
        "cached signed artifacts available to install offline",
        "artifacts",
        &tbls,
        &["registry", "catalog"],
    ));

    // ── state: the one datastore + the ingest path into it ──────────────────────────────────────
    out.push(ServiceStatus {
        id: "store".into(),
        label: "Datastore (SurrealDB)".into(),
        group: "state".into(),
        health: Health::Ok,
        detail: "the single embedded store — workspace-isolated by namespace".into(),
        metrics: vec![
            Metric::new("tables", tbls.len().to_string()),
            Metric::new("rows", total_rows.to_string()),
        ],
    });
    out.push(table_service(
        "ingest",
        "Ingest Server",
        "state",
        "generic exactly-once buffer draining into committed series",
        "rows",
        &tbls,
        &["sample", "series", "staging"],
    ));

    // ── workflow: the durable inbox / outbox / jobs primitives ──────────────────────────────────
    out.push(table_service(
        "inbox",
        "Inbox Service",
        "workflow",
        "durable approvals + triage awaiting a decision",
        "items",
        &tbls,
        &["inbox"],
    ));
    out.push(collect_outbox(node, ws).await?);
    out.push(table_service(
        "jobs",
        "Jobs Service",
        "workflow",
        "durable, resumable agent/coding sessions",
        "jobs",
        &tbls,
        &["job"],
    ));

    Ok(out)
}

/// Build a card whose only live signal is a count of matching tables (substring match, so the card
/// degrades gracefully to `0`/`Idle` if a table does not exist yet rather than erroring).
fn table_service(
    id: &str,
    label: &str,
    group: &str,
    detail: &str,
    unit: &str,
    tbls: &[TableCount],
    needles: &[&str],
) -> ServiceStatus {
    let count: u64 = tbls
        .iter()
        .filter(|t| {
            let name = t.table.to_lowercase();
            needles.iter().any(|n| name.contains(n))
        })
        .map(|t| t.count)
        .sum();
    ServiceStatus {
        id: id.into(),
        label: label.into(),
        group: group.into(),
        health: if count == 0 { Health::Idle } else { Health::Ok },
        detail: detail.into(),
        metrics: vec![Metric::new(unit, count.to_string())],
    }
}

/// Extensions card — durable installs joined with live process state (mirrors `ext_list`, ungated).
/// `Degraded` if an extension is enabled but not running (the actionable case an operator debugs).
async fn collect_extensions(node: &Node, ws: &str) -> Result<ServiceStatus, SystemError> {
    let installs = list_installs(&node.store, ws).await?;
    let (mut running, mut native, mut wasm, mut stopped_enabled) = (0u32, 0u32, 0u32, 0u32);
    for ins in &installs {
        match ins.tier {
            Tier::Native => {
                native += 1;
                if node.sidecars.is_running(ws, &ins.ext_id) {
                    running += 1;
                } else if ins.enabled {
                    stopped_enabled += 1;
                }
            }
            Tier::Wasm => {
                wasm += 1;
                if ins.enabled {
                    running += 1;
                }
            }
        }
    }
    let health = if stopped_enabled > 0 {
        Health::Degraded
    } else if installs.is_empty() {
        Health::Idle
    } else {
        Health::Ok
    };
    let detail = if stopped_enabled > 0 {
        format!("{stopped_enabled} enabled extension(s) are not running")
    } else {
        "loaded extensions (wasm + native sidecars) under the host supervisor".into()
    };
    Ok(ServiceStatus {
        id: "extensions".into(),
        label: "Extension Service".into(),
        group: "runtime".into(),
        health,
        detail,
        metrics: vec![
            Metric::new("installed", installs.len().to_string()),
            Metric::new("running", running.to_string()),
            Metric::new("native", native.to_string()),
            Metric::new("wasm", wasm.to_string()),
        ],
    })
}

/// Outbox card — the transactional-effect delivery snapshot. `Degraded` the moment anything is
/// dead-lettered (retries exhausted — the operator's first stop); `Idle` when the outbox is empty.
async fn collect_outbox(node: &Node, ws: &str) -> Result<ServiceStatus, SystemError> {
    let pending = pending(&node.store, ws).await?.len();
    let delivered = delivered(&node.store, ws).await?.len();
    let dead = dead_lettered(&node.store, ws).await?.len();
    let health = if dead > 0 {
        Health::Degraded
    } else if pending == 0 && delivered == 0 {
        Health::Idle
    } else {
        Health::Ok
    };
    let detail = if dead > 0 {
        format!("{dead} effect(s) dead-lettered after exhausting retries")
    } else {
        "must-deliver effects (PRs, comments, sync) with backoff + dead-letter".into()
    };
    Ok(ServiceStatus {
        id: "outbox".into(),
        label: "Outbox Service".into(),
        group: "workflow".into(),
        health,
        detail,
        metrics: vec![
            Metric::new("pending", pending.to_string()),
            Metric::new("delivered", delivered.to_string()),
            Metric::new("dead-letter", dead.to_string()),
        ],
    })
}
