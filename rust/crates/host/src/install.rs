//! Install an extension into a workspace — the S4 durable install flow (README §6.4,
//! extensions scope). S1 passed `admin_approved` in on every `load_extension`; S4 **persists**
//! the computed grant set as an [`Install`] record so it survives a restart and becomes the
//! workspace's durable answer to "what is this extension allowed here?" (the extensions-scope
//! open question: "where the admin-approval set is stored").
//!
//! Flow: parse the manifest, compute `granted = requested ∩ admin_approved` (the loader's
//! enforcement point), **record the install** in the workspace namespace, then load the
//! component into the runtime. Persist-before-load mirrors the channel persist-before-publish
//! discipline — the durable approval is the source of truth, the running instance follows it.

use lb_assets::{record_install, ExtUi, Install};
use lb_ext_loader::{grant, Manifest, UiPage, Widget};
use lb_store::StoreError;

use crate::boot::Node;
use crate::load::{load_extension, LoadError, Loaded};

/// Project a manifest `[ui]`/`[widget]` contribution onto the durable [`ExtUi`] stored on the
/// install, **narrowing its declared `scope` to the granted caps** — a page/widget can never claim
/// a tool the admin didn't approve (ui-federation: "a gated caller, never a trusted decider"). The
/// bridge re-filters and the host re-checks regardless; this is the durable, narrowed truth.
fn ui_from(
    scope: &[String],
    granted: &[String],
    entry: String,
    label: String,
    icon: String,
) -> ExtUi {
    let allowed: Vec<String> = scope
        .iter()
        .filter(|t| granted.iter().any(|g| g == &format!("mcp:{t}:call")))
        .cloned()
        .collect();
    ExtUi {
        entry,
        label,
        icon,
        scope: allowed,
    }
}

/// Install `wasm_bytes` (described by `manifest_toml`) into `node` for workspace `ws`: persist
/// the `requested ∩ admin_approved` grant set as a durable install record, then load. `ts` is a
/// caller-injected logical timestamp (testing §3 determinism). Returns the loaded result.
pub async fn install_extension(
    node: &Node,
    ws: &str,
    manifest_toml: &str,
    wasm_bytes: &[u8],
    admin_approved: &[String],
    ts: u64,
) -> Result<Loaded, LoadError> {
    let manifest =
        Manifest::parse(manifest_toml).map_err(|e| LoadError::Manifest(e.to_string()))?;
    let granted = grant(&manifest, admin_approved);

    // Project the manifest's page/widget contributions onto the install, scope-narrowed to `granted`.
    let ui = manifest.ui.as_ref().map(|u: &UiPage| {
        ui_from(
            &u.scope,
            &granted,
            u.entry.clone(),
            u.label.clone(),
            u.icon.clone(),
        )
    });
    let widget = manifest.widget.as_ref().map(|w: &Widget| {
        ui_from(
            &w.scope,
            &granted,
            w.entry.clone(),
            w.label.clone(),
            w.icon.clone(),
        )
    });

    // STATE: persist the approved grant set first — the durable record of what was allowed.
    let install = Install::new(
        manifest.id.clone(),
        manifest.version.clone(),
        granted.clone(),
        ts,
    )
    .with_ui(ui, widget);
    record_install(&node.store, ws, &install)
        .await
        .map_err(store_to_load)?;

    // Then bring the component online with exactly that approved set.
    load_extension(node, manifest_toml, wasm_bytes, admin_approved).await
}

fn store_to_load(e: StoreError) -> LoadError {
    LoadError::Manifest(format!("persisting install record: {e}"))
}
