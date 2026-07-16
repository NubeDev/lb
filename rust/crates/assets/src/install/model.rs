//! The install record shape (README §6.4, extensions scope).
//!
//! What an extension is durably allowed in a workspace: its `ext_id`, the `version` installed,
//! and `granted` — the `requested ∩ admin_approved` capability strings the host computed at
//! install. The running instance's token carries exactly `granted`; nothing the manifest asked
//! for is live unless an admin approved it.

use serde::{Deserialize, Serialize};

/// The extension tier an install belongs to (README §6.3). `Wasm` is a Tier-1 component (no OS
/// process); `Native` is a Tier-2 supervised sidecar. The lifecycle surface dispatches by this
/// (lifecycle-management scope) so one verb set serves both tiers — no `if tier` in the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Wasm,
    Native,
}

/// A durable copy of an extension's UI contribution — a **page** (`ui`) or a dashboard **widget**
/// (`widget`), as declared in the manifest's `[ui]`/`[widget]` block and persisted on the install so
/// `ext.list` can tell the shell what to mount without re-reading the manifest (ui-federation +
/// dashboard-widgets scopes). Structurally identical for both surfaces; one shape, two install fields.
// NOTE: no `Eq` — `ExtUiOption::{choices,default}` are `serde_json::Value`, which is not `Eq`. The
// struct keeps `PartialEq` (all `assert_eq!` / comparison sites use that; no `HashSet<ExtUi>` exists).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ExtUi {
    /// The ESM bundle entry (relative to the extension's served UI dir) exposing `mount(el, ctx,
    /// bridge)` in-process, or the iframe document for the sandboxed tier.
    pub entry: String,
    /// The nav-slot / palette label.
    pub label: String,
    /// A lucide icon name (empty = default).
    #[serde(default)]
    pub icon: String,
    /// The read-only MCP tool scope the page/widget may call through the host bridge — already
    /// intersected against the install grant when written.
    #[serde(default)]
    pub scope: Vec<String>,
    /// Frames-in opt-in for a widget (ext-widget-source-binding scope): `true` = a data tile that
    /// carries `sources[]` and receives shell-resolved frames; `false` = a v2 self-fetching tile.
    /// Always `false` for a page. Serde-defaulted so installs written before this field read as
    /// `false`.
    #[serde(default)]
    pub data: bool,
    /// The stable widget id (ext-widget-panel-options scope) — the `ext:<ext>/<id>` view-key segment.
    /// Persisted so `dashboard.catalog`/`ext.list` key on it without re-reading the manifest. `None`
    /// for a page and for installs written before this field. Serde-defaulted.
    #[serde(default)]
    pub id: Option<String>,
    /// The widget's declarative panel options (ext-widget-panel-options scope) — relayed verbatim from
    /// the manifest to the editor; the host never interprets a def. Empty for a page and for installs
    /// written before this field. Serde-defaulted.
    #[serde(default)]
    pub options: Vec<ExtUiOption>,
}

/// A persisted mirror of a manifest `WidgetOption` (ext-widget-panel-options scope). Carried on the
/// install so the editor can render the widget's option surface from `ext.list`/`dashboard.catalog`
/// without re-reading the manifest. Opaque relay data — the host stores and forwards it, never
/// branches on `control`/`scope`. Same shape as `widget_catalog.json`'s built-in option-def so one
/// vocabulary drives every host's editor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ExtUiOption {
    pub id: String,
    pub label: String,
    pub scope: String,
    pub path: String,
    pub control: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choices: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
}

/// The constant `kind` discriminant so `list_installs` can equality-filter every install row in a
/// workspace (the union both tiers share — lifecycle-management scope's `ext.list`).
pub(crate) const KIND: &str = "install";

/// A persisted extension install: the approved-and-granted capability set for `ext_id` in a
/// workspace, plus the durable lifecycle intent. Addressed by `ext_id` (one install per extension
/// per workspace).
// No `Eq` — carries `ExtUi` (whose option defs hold non-`Eq` `serde_json::Value`). `PartialEq` only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Install {
    pub ext_id: String,
    pub version: String,
    /// The granted caps = `requested ∩ admin_approved`, persisted so a restart re-grants
    /// exactly this set without re-asking the admin (extensions scope, the S4 open question).
    pub granted: Vec<String>,
    /// Which tier this install is — the lifecycle surface dispatches on it.
    #[serde(default = "wasm_tier")]
    pub tier: Tier,
    /// Durable **intent**, distinct from running: `disable` sets `false` (do-not-auto-start-on-boot);
    /// the boot reconciler honors `enabled ∧ started`. Defaults `true` for records written before
    /// this field existed (lifecycle-management scope).
    #[serde(default = "enabled_default")]
    pub enabled: bool,
    /// Constant discriminant so `list_installs` selects every row.
    #[serde(default = "install_kind")]
    pub kind: String,
    /// A full **page** this extension contributes to the shell sidebar — `Some` iff it declared
    /// `[ui]`. Serde-defaulted: records written before this field deserialize as `None`.
    #[serde(default)]
    pub ui: Option<ExtUi>,
    /// The dashboard **widget** tiles this extension contributes — one per `[[widget]]` table.
    /// Empty if it declared none. Serde-defaulted: records written before this field deserialize
    /// as an empty vec.
    #[serde(default)]
    pub widgets: Vec<ExtUi>,
    /// The flow node types this extension contributes — the validated `[[node]]` blocks from its
    /// manifest (flows node-descriptor scope). The `flows.nodes` registry is a **read-time union**
    /// of these across a workspace's installs + the built-ins, holding nothing new durable.
    /// Serde-defaulted: an install written before this field deserializes as empty (no nodes).
    #[serde(default)]
    pub nodes: Vec<lb_flows::NodeBlock>,
    pub ts: u64,
}

fn wasm_tier() -> Tier {
    Tier::Wasm
}
fn enabled_default() -> bool {
    true
}
fn install_kind() -> String {
    KIND.to_string()
}

impl Install {
    pub fn new(
        ext_id: impl Into<String>,
        version: impl Into<String>,
        granted: Vec<String>,
        ts: u64,
    ) -> Self {
        Self {
            ext_id: ext_id.into(),
            version: version.into(),
            granted,
            tier: Tier::Wasm,
            enabled: true,
            kind: KIND.to_string(),
            ui: None,
            widgets: Vec::new(),
            nodes: Vec::new(),
            ts,
        }
    }

    /// Set the tier (builder-style) — native installs call this so `ext.list` reports the row's tier.
    pub fn with_tier(mut self, tier: Tier) -> Self {
        self.tier = tier;
        self
    }

    /// Attach the extension's UI contributions (builder-style) — the page and/or widget tiles the
    /// manifest declared, so `ext.list` tells the shell what to mount (ui-federation +
    /// dashboard-widgets scopes).
    pub fn with_ui(mut self, ui: Option<ExtUi>, widgets: Vec<ExtUi>) -> Self {
        self.ui = ui;
        self.widgets = widgets;
        self
    }

    /// Attach the extension's flow node contributions (builder-style) — the validated `[[node]]`
    /// blocks, so `flows.nodes` is a read-time union over installs + the built-ins (flows scope).
    pub fn with_nodes(mut self, nodes: Vec<lb_flows::NodeBlock>) -> Self {
        self.nodes = nodes;
        self
    }
}
