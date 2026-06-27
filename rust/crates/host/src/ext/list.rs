//! `ext.list` — enumerate a workspace's installed extensions, both tiers, with live state joined
//! (lifecycle-management scope: the conspicuously-missing verb). Gated `mcp:ext.list:call`,
//! workspace-first. Reads `Install` records (the union) and joins the native `SidecarMap` for the
//! `running`/`restart_count` of native rows; wasm rows have no separate process so `running` follows
//! `enabled`.

use lb_assets::{list_installs, Tier};
use lb_auth::Principal;
use lb_mcp::authorize_tool;

use super::error::ExtError;
use super::row::ExtRow;
use crate::boot::Node;
use crate::native::read_status;

/// Return one [`ExtRow`] per installed extension in workspace `ws` for `caller`, sorted by id.
pub async fn ext_list(node: &Node, caller: &Principal, ws: &str) -> Result<Vec<ExtRow>, ExtError> {
    authorize_tool(caller, ws, "ext.list").map_err(|_| ExtError::Denied)?;
    let installs = list_installs(&node.store, ws).await?;
    let mut rows = Vec::with_capacity(installs.len());
    for install in &installs {
        let (running, restarts) = match install.tier {
            // Native: the live truth is the SidecarMap + the durable restart counter.
            Tier::Native => {
                let running = node.sidecars.is_running(ws, &install.ext_id);
                let restarts = read_status(&node.store, ws, &install.ext_id)
                    .await
                    .ok()
                    .flatten()
                    .map(|s| s.restart_count)
                    .unwrap_or(0);
                (running, restarts)
            }
            // Wasm: no OS process; an enabled component is loaded/runnable.
            Tier::Wasm => (install.enabled, 0),
        };
        rows.push(ExtRow::from_install(install, running, restarts));
    }
    rows.sort_by(|a, b| a.ext.cmp(&b.ext));
    Ok(rows)
}
