//! Parse `extension.toml` — the §13 forever contract (extensions scope). TOML, declared
//! tools (so the host can register + authorize without instantiating), requested caps (a
//! request, never a grant), and the WIT world major (checked against the host's SDK).

use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ManifestError {
    #[error("manifest is not valid TOML: {0}")]
    Toml(String),
    #[error("extension declares WIT world '{0}' incompatible with this host")]
    WorldMismatch(String),
    #[error("unknown runtime tier '{0}' (expected wasm | native)")]
    UnknownTier(String),
    /// A `tier="native"` manifest must carry a `[native]` block naming the `exec` to spawn — the
    /// supervisor has nothing to launch otherwise (native-tier scope). A wasm manifest must NOT.
    #[error("native tier requires a [native] block with exec; wasm tier must omit it")]
    NativeSpec,
    /// A `[[node]]` block is incoherent: its `tool` binds a non-existent `[[tools]]` entry, or its
    /// `[node.config]` is not a valid JSON-Schema 2020-12 document (flows node-descriptor scope). A
    /// node that cannot execute is a load-time reject — the manifest is incoherent.
    #[error("invalid [[node]] block: {0}")]
    InvalidNodeBlock(String),
    /// A `[[widget]]` declares two options (or two widgets) sharing one id/slug — the view key
    /// `ext:<id>/<widget-id>` (and per-widget option paths) must be unique per manifest, so a
    /// collision is a load-time reject (ext-widget-panel-options scope). Also raised for a malformed
    /// `options` def (empty id/label/path, or an empty `control`).
    #[error("invalid [[widget]] block: {0}")]
    InvalidWidgetBlock(String),
}

/// Slugify a display label into a stable id — lowercased, runs of non-alphanumerics collapsed to a
/// single `-`, trimmed. The default `[[widget]]` id and the fallback view-key segment; the UI's
/// `widgetIdOf` (source-picker) computes the SAME slug so picker, renderer, and host agree on one key.
pub fn slug(label: &str) -> String {
    let mut out = String::with_capacity(label.len());
    let mut prev_dash = false;
    for ch in label.chars() {
        if ch.is_ascii_alphanumeric() {
            out.extend(ch.to_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

/// One declarative panel option a `[[widget]]` understands (ext-widget-panel-options scope). The host
/// is a **relay, not an interpreter**: it shape-validates these at parse and passes them through
/// (`ExtUi`/`ExtRow`/`ExtWidget`) verbatim — no host code branches on an option's meaning. The shape
/// mirrors `widget_catalog.json`'s built-in option-def shape so one vocabulary drives every host's
/// editor. `choices`/`default` are opaque JSON the editor renders; `control` is the host's control
/// vocabulary (`text`/`number`/`toggle`/`select`/`unit`/`thresholds`/…) — an unknown value is NOT
/// rejected here (an older host degrades it to a labeled raw row), only an EMPTY control is.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct WidgetOption {
    pub id: String,
    pub label: String,
    /// `"options"` (per-viz `options.<path>`) | `"fieldConfig"` (`fieldConfig.defaults.<path>`) — the
    /// two cell roots. Not enum-validated (kept a forward-compatible string; the editor interprets it).
    pub scope: String,
    pub path: String,
    pub control: String,
    #[serde(default)]
    pub choices: Option<serde_json::Value>,
    #[serde(default)]
    pub default: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Public,
    Private,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Tool {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// An optional standard **JSON Schema** (`type:"object"`, `properties`, `required`) declaring
    /// the tool's input, surfaced to the command palette via `tools.catalog` (channels-command-
    /// palette scope). Additive + versioned: a tool that omits it is still registered and callable —
    /// the palette renders a single free-text arg, so an old extension needs no rebuild. Per-property
    /// vendor hints live under an `x-lb` key (`x-lb-entity`, `x-lb-widget`). Deserialized straight
    /// from the TOML `[tools]` table (TOML ↔ JSON values are compatible for schema shapes).
    #[serde(default)]
    pub input_schema: Option<serde_json::Value>,
    /// This tool **can transmit data off the node** (send a message, fetch a URL, call a webhook) —
    /// the self-declared exfiltration taint (agent-loop-hardening slice E), carried onto the
    /// registered `ToolDescriptor` and consumed generically by `exfiltration_guard`-flagged runs.
    /// Additive + versioned by absence: an old manifest omits it (false), nothing else changes.
    /// (The `lb-ext-sdk` manifest authoring type gains the same optional field — flagged there.)
    #[serde(default)]
    pub emits_external: bool,
}

/// The `[native]` block — present iff `tier="native"` (native-tier scope, the extensions-scope
/// deferred "Native (`tier="native"`) manifest fields (exec, supervision, socket) — S7"). It is the
/// recipe the host turns into a `lb_supervisor::Spec`: which binary to spawn, its args, the platform
/// target the binary is built for (a native binary is NOT portable like a `.wasm`, platform-targets
/// scope), and the restart policy. Health/grace/backoff timings stay host-defaults this slice.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct Native {
    /// The executable the supervisor spawns. Resolved by the host against the install dir.
    pub exec: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// The target triple the binary is built for (platform-targets scope). Empty = host/unspecified.
    #[serde(default)]
    pub target: String,
    /// `"on-crash"` (default) | `"never"` — the crash-restart policy (operator restart is separate).
    #[serde(default)]
    pub restart: String,
}

/// The `[ui]` block — an extension that contributes a **full page** to the shell's sidebar
/// (ui-federation scope, README §6.13). Frozen v1 fields. Serde-defaulted: an extension without a
/// `[ui]` block contributes no page (the lifecycle/console story is unchanged). The **trust tier is
/// NOT here** — it is the publisher key's allow-list status (the registry `TrustedKeys`), never the
/// manifest's claim. A trusted page is module-federated in-process; an untrusted one sandboxes.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct UiPage {
    /// The ESM bundle entry (relative to the extension's served UI dir) exposing `mount(el, ctx,
    /// bridge)` for the in-process tier, or the iframe entry document for the sandboxed tier.
    pub entry: String,
    /// The sidebar nav-slot label.
    pub label: String,
    /// A lucide icon name for the nav slot (empty = a default).
    #[serde(default)]
    pub icon: String,
    /// The read-only MCP tool scope the page may call through the host-mediated bridge — bounded by
    /// the install's `granted` (= `requested ∩ admin_approved`). Empty = the page calls nothing.
    #[serde(default)]
    pub scope: Vec<String>,
}

/// A `[[widget]]` table — an extension that contributes a **dashboard tile** droppable into a grid
/// cell (dashboard-widgets scope). Frozen v1 fields. An extension may declare **several** widgets
/// (array-of-tables, `widgets: Vec<Widget>`), each its own palette tile. A widget is read-only on
/// series and far more constrained than a page; the `scope` here is a subset of the four series read
/// verbs.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct Widget {
    /// The ESM bundle entry exposing `mount(el, ctx, bridge)` (in-process) / iframe doc (sandboxed).
    pub entry: String,
    /// The widget-palette label.
    pub label: String,
    #[serde(default)]
    pub icon: String,
    /// The read-only series verbs the widget may call (subset of `series.read|latest|find|watch`),
    /// bounded by the install grant. Validated at install: a non-series/write verb is rejected.
    #[serde(default)]
    pub scope: Vec<String>,
    /// Frames-in opt-in (ext-widget-source-binding scope). `true` = this tile is a first-class
    /// **view** over the v3 panel model: the editor shows the Query + Field tabs, and the shell
    /// resolves the cell's `sources[]` through `viz.query` under the viewer's grant and hands the
    /// tile resolved frames (`ctx.data`). Default `false` = a v2 self-fetching tile, unchanged.
    /// Additive + serde-defaulted: a manifest written before this field parses as `false`.
    #[serde(default)]
    pub data: bool,
    /// Stable widget id (ext-widget-panel-options scope). The view key is `ext:<ext>/<id>`; `id`
    /// defaults to `slug(label)` at parse (via [`Widget::widget_id`]) so every existing manifest keeps
    /// its label-slug key. An author who SETS an `id` differing from the old slug renames that widget's
    /// existing cells — documented as a breaking rename. Additive + serde-defaulted.
    #[serde(default)]
    pub id: Option<String>,
    /// The declarative panel options this widget understands (ext-widget-panel-options scope). Opaque
    /// per-widget data the host relays verbatim to the editor — the host never interprets a def. Empty
    /// (the default) means the editor offers only the standard field set (iff `data = true`). Additive
    /// + serde-defaulted.
    #[serde(default)]
    pub options: Vec<WidgetOption>,
}

impl Widget {
    /// The resolved widget id — the explicit `id`, else `slug(label)`. The view-key segment
    /// (`ext:<ext>/<widget_id>`) and the manifest-uniqueness key.
    pub fn widget_id(&self) -> String {
        self.id.clone().unwrap_or_else(|| slug(&self.label))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Manifest {
    pub id: String,
    pub version: String,
    pub tier: String,
    pub world: String,
    pub placement: String,
    /// Capabilities the extension REQUESTS — intersected with admin approval by `grant`.
    pub requested_caps: Vec<String>,
    pub tools: Vec<Tool>,
    pub visibility: Visibility,
    /// The native supervision recipe — `Some` iff `tier="native"` (validated at parse). `None` for a
    /// wasm extension (it has no child process).
    pub native: Option<Native>,
    /// A full page contributed to the shell's sidebar — `Some` iff the manifest declares `[ui]`
    /// (ui-federation scope). Independent of `tier`: a wasm/native extension may also ship a page.
    pub ui: Option<UiPage>,
    /// The dashboard widget tiles — one per `[[widget]]` table the manifest declares (dashboard-widgets
    /// scope). Empty if the manifest declares none. An extension may ship several palette tiles.
    pub widgets: Vec<Widget>,
    /// The flow node types this extension contributes — one validated `[[node]]` block each (flows
    /// node-descriptor scope, the only manifest addition). Empty if the manifest declares none. Each
    /// block's `tool` was verified to bind a declared `[[tools]]` entry and its `config` compiled as
    /// JSON-Schema 2020-12 at parse, so an install carries already-validated node blocks. Additive +
    /// serde-defaulted: a manifest (or a host) written before this field deserialises as empty.
    pub nodes: Vec<lb_flows::NodeBlock>,
}

// Raw TOML shape, mapped to the flat `Manifest` after validation.
#[derive(Deserialize)]
struct Raw {
    extension: RawExt,
    runtime: RawRuntime,
    #[serde(default)]
    capabilities: RawCaps,
    #[serde(default)]
    tools: Vec<Tool>,
    visibility: RawVisibility,
    #[serde(default)]
    native: Option<Native>,
    #[serde(default)]
    ui: Option<UiPage>,
    /// `[[widget]]` array-of-tables — zero or more widget tiles.
    #[serde(default)]
    widget: Vec<Widget>,
    /// `[[node]]` array-of-tables — zero or more flow node types (flows node-descriptor scope).
    #[serde(default)]
    node: Vec<lb_flows::NodeBlock>,
}
#[derive(Deserialize)]
struct RawExt {
    id: String,
    version: String,
}
#[derive(Deserialize)]
struct RawRuntime {
    tier: String,
    world: String,
    placement: String,
}
#[derive(Deserialize, Default)]
struct RawCaps {
    #[serde(default)]
    request: Vec<String>,
}
#[derive(Deserialize)]
struct RawVisibility {
    class: Visibility,
}

impl Manifest {
    /// Parse + validate a manifest's TOML text. Rejects an unknown tier and a WIT world whose
    /// major does not match this host's SDK (the load-time ABI check, crate-layout scope).
    pub fn parse(text: &str) -> Result<Self, ManifestError> {
        let raw: Raw = toml::from_str(text).map_err(|e| ManifestError::Toml(e.to_string()))?;

        if raw.runtime.tier != "wasm" && raw.runtime.tier != "native" {
            return Err(ManifestError::UnknownTier(raw.runtime.tier));
        }
        if !lb_sdk::world_major_matches(&raw.runtime.world) {
            return Err(ManifestError::WorldMismatch(raw.runtime.world));
        }

        // The `[native]` block is required for and exclusive to the native tier: the supervisor must
        // know what to spawn, and a wasm extension has no child (native-tier scope).
        let is_native = raw.runtime.tier == "native";
        let native = match (is_native, raw.native) {
            (true, Some(n)) if !n.exec.is_empty() => Some(n),
            (true, _) => return Err(ManifestError::NativeSpec),
            (false, Some(_)) => return Err(ManifestError::NativeSpec),
            (false, None) => None,
        };

        // Validate every `[[node]]` block: its `tool` must bind a declared `[[tools]]` entry and its
        // `config` must compile as JSON-Schema 2020-12 (flows node-descriptor scope). A node that
        // cannot execute is a load-time reject. The global type is `<ext_id>.<type>`; validation
        // happens here (where the tools list is known) so an install carries already-trusted blocks.
        let tool_names: Vec<String> = raw.tools.iter().map(|t| t.name.clone()).collect();
        for block in &raw.node {
            lb_flows::validate_node_block(block, &raw.extension.id, &tool_names)
                .map_err(|e| ManifestError::InvalidNodeBlock(e.to_string()))?;
        }

        // Validate the `[[widget]]` blocks (ext-widget-panel-options scope): each declared option def is
        // shape-checked (non-empty id/label/path/control), option ids are unique WITHIN a widget, and
        // resolved widget ids (`id` or `slug(label)`) are unique ACROSS the manifest — the view key
        // `ext:<ext>/<widget-id>` must be unambiguous. The host validates SHAPE + UNIQUENESS only; it
        // never interprets an option's meaning (relay, not interpreter — rule 10). A widget with no
        // entry is dropped below (mirrors `[ui]`), so validate only the tiles that survive.
        let mut seen_widget_ids: Vec<String> = Vec::new();
        for w in raw.widget.iter().filter(|w| !w.entry.is_empty()) {
            let wid = w.widget_id();
            if wid.is_empty() {
                return Err(ManifestError::InvalidWidgetBlock(format!(
                    "widget '{}' has no usable id (empty label and no id)",
                    w.label
                )));
            }
            if seen_widget_ids.contains(&wid) {
                return Err(ManifestError::InvalidWidgetBlock(format!(
                    "duplicate widget id '{wid}' — each tile's view key must be unique"
                )));
            }
            seen_widget_ids.push(wid.clone());

            let mut seen_opt_ids: Vec<&str> = Vec::new();
            for opt in &w.options {
                if opt.id.is_empty() || opt.label.is_empty() || opt.path.is_empty() {
                    return Err(ManifestError::InvalidWidgetBlock(format!(
                        "widget '{wid}' option has an empty id/label/path"
                    )));
                }
                if opt.control.is_empty() {
                    return Err(ManifestError::InvalidWidgetBlock(format!(
                        "widget '{wid}' option '{}' has an empty control",
                        opt.id
                    )));
                }
                if seen_opt_ids.contains(&opt.id.as_str()) {
                    return Err(ManifestError::InvalidWidgetBlock(format!(
                        "widget '{wid}' declares duplicate option id '{}'",
                        opt.id
                    )));
                }
                seen_opt_ids.push(&opt.id);
            }
        }

        Ok(Manifest {
            id: raw.extension.id,
            version: raw.extension.version,
            tier: raw.runtime.tier,
            world: raw.runtime.world,
            placement: raw.runtime.placement,
            requested_caps: raw.capabilities.request,
            tools: raw.tools,
            visibility: raw.visibility.class,
            native,
            ui: raw.ui.filter(|u| !u.entry.is_empty()),
            // Drop any half-written tile with no entry (defensive, mirrors `[ui]`).
            widgets: raw
                .widget
                .into_iter()
                .filter(|w| !w.entry.is_empty())
                .collect(),
            nodes: raw.node,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NATIVE_TOML: &str = r#"
[extension]
id = "echo-sidecar"
version = "0.1.0"

[runtime]
tier = "native"
world = "lazybones:ext/extension@0.1.0"
placement = "either"

[native]
exec = "echo-sidecar"
args = ["--serve"]
restart = "on-crash"

[[tools]]
name = "echo"

[visibility]
class = "private"
"#;

    fn with_runtime(tier: &str, native_block: &str) -> String {
        format!(
            r#"
[extension]
id = "x"
version = "0.1.0"
[runtime]
tier = "{tier}"
world = "lazybones:ext/extension@0.1.0"
placement = "either"
{native_block}
[visibility]
class = "private"
"#
        )
    }

    #[test]
    fn parses_native_block() {
        let m = Manifest::parse(NATIVE_TOML).expect("native manifest parses");
        assert_eq!(m.tier, "native");
        let n = m.native.expect("native tier carries a [native] block");
        assert_eq!(n.exec, "echo-sidecar");
        assert_eq!(n.args, vec!["--serve".to_string()]);
        assert_eq!(n.restart, "on-crash");
    }

    #[test]
    fn native_tier_without_exec_is_rejected() {
        // tier=native but no [native] block → NativeSpec (the supervisor has nothing to spawn).
        let toml = with_runtime("native", "");
        assert_eq!(Manifest::parse(&toml), Err(ManifestError::NativeSpec));
    }

    #[test]
    fn wasm_tier_with_native_block_is_rejected() {
        // A wasm extension must not carry supervision fields (it has no child).
        let toml = with_runtime("wasm", "[native]\nexec = \"oops\"");
        assert_eq!(Manifest::parse(&toml), Err(ManifestError::NativeSpec));
    }

    #[test]
    fn wasm_tier_omits_native() {
        let toml = with_runtime("wasm", "");
        let m = Manifest::parse(&toml).expect("wasm manifest parses");
        assert!(m.native.is_none());
    }

    #[test]
    fn no_ui_or_widget_by_default() {
        // An extension that declares neither block contributes no page and no widget.
        let m = Manifest::parse(&with_runtime("wasm", "")).expect("parses");
        assert!(m.ui.is_none());
        assert!(m.widgets.is_empty());
    }

    #[test]
    fn parses_ui_page_block() {
        let toml = with_runtime(
            "wasm",
            "[ui]\nentry = \"entry.mjs\"\nlabel = \"Reports\"\nicon = \"chart-bar\"\nscope = [\"channel.list\"]",
        );
        let m = Manifest::parse(&toml).expect("parses");
        let ui = m.ui.expect("a [ui] block yields Some");
        assert_eq!(ui.entry, "entry.mjs");
        assert_eq!(ui.label, "Reports");
        assert_eq!(ui.icon, "chart-bar");
        assert_eq!(ui.scope, vec!["channel.list".to_string()]);
        assert!(m.widgets.is_empty());
    }

    #[test]
    fn parses_widget_block() {
        let toml = with_runtime(
            "wasm",
            "[[widget]]\nentry = \"widget.mjs\"\nlabel = \"Temp\"\nscope = [\"series.read\", \"series.watch\"]",
        );
        let m = Manifest::parse(&toml).expect("parses");
        assert_eq!(m.widgets.len(), 1);
        let w = &m.widgets[0];
        assert_eq!(w.entry, "widget.mjs");
        assert_eq!(w.label, "Temp");
        assert_eq!(
            w.scope,
            vec!["series.read".to_string(), "series.watch".to_string()]
        );
    }

    #[test]
    fn parses_multiple_widget_blocks() {
        // An extension may declare several `[[widget]]` tiles — each its own palette entry.
        let toml = with_runtime(
            "wasm",
            "[[widget]]\nentry = \"a.mjs\"\nlabel = \"A\"\n[[widget]]\nentry = \"b.mjs\"\nlabel = \"B\"",
        );
        let m = Manifest::parse(&toml).expect("parses");
        assert_eq!(m.widgets.len(), 2);
        assert_eq!(m.widgets[0].label, "A");
        assert_eq!(m.widgets[1].label, "B");
    }

    #[test]
    fn ui_and_widget_together() {
        // One extension may ship BOTH a page and one-or-more widgets.
        let toml = with_runtime(
            "wasm",
            "[ui]\nentry = \"p.mjs\"\nlabel = \"Page\"\n[[widget]]\nentry = \"w.mjs\"\nlabel = \"Tile\"",
        );
        let m = Manifest::parse(&toml).expect("parses");
        assert!(m.ui.is_some());
        assert_eq!(m.widgets.len(), 1);
    }

    #[test]
    fn empty_entry_is_treated_as_absent() {
        // A `[ui]` block with no entry is not a contribution (defensive against a half-written block).
        let toml = with_runtime("wasm", "[ui]\nentry = \"\"\nlabel = \"x\"");
        assert!(Manifest::parse(&toml).expect("parses").ui.is_none());
    }

    #[test]
    fn parses_node_blocks_and_validates_tool_binding() {
        let toml = with_runtime(
            "wasm",
            r#"
[[tools]]
name = "publish"
[[tools]]
name = "subscribe"

[[node]]
type = "out"
kind = "sink"
tool = "publish"
inputs = ["payload"]
[node.config]
type = "object"
required = ["topic"]
properties.topic = { type = "string" }

[[node]]
type = "in"
kind = "source"
tool = "subscribe"
[node.config]
type = "object"
required = ["broker"]
properties.broker = { type = "string" }
"#,
        );
        let m = Manifest::parse(&toml).expect("parses node blocks");
        assert_eq!(m.nodes.len(), 2);
        assert_eq!(m.nodes[0].r#type, "out");
        assert_eq!(m.nodes[0].tool, "publish");
        assert_eq!(m.nodes[1].kind, lb_flows::NodeKind::Source);
    }

    #[test]
    fn rejects_node_block_with_dangling_tool() {
        // A [[node]] whose `tool` names no [[tools]] entry is a load-time reject.
        let toml = with_runtime(
            "wasm",
            "[[tools]]\nname = \"publish\"\n[[node]]\ntype = \"x\"\nkind = \"sink\"\ntool = \"nope\"",
        );
        let err = Manifest::parse(&toml).unwrap_err();
        assert!(matches!(err, ManifestError::InvalidNodeBlock(_)));
    }

    #[test]
    fn rejects_node_block_with_non_schema_config() {
        let toml = with_runtime(
            "wasm",
            "[[tools]]\nname = \"publish\"\n[[node]]\ntype = \"x\"\nkind = \"sink\"\ntool = \"publish\"\n[node.config]\ntype = \"not-a-type\"",
        );
        let err = Manifest::parse(&toml).unwrap_err();
        assert!(matches!(err, ManifestError::InvalidNodeBlock(_)));
    }

    #[test]
    fn no_nodes_by_default() {
        let m = Manifest::parse(&with_runtime("wasm", "")).expect("parses");
        assert!(m.nodes.is_empty());
    }

    // --- ext-widget-panel-options scope: widget id + declarative options ---

    #[test]
    fn slug_matches_ui_widget_id_of() {
        // Must agree with the UI's `widgetIdOf` (source-picker): lowercased, non-alnum runs → single
        // `-`, trimmed. Picker, renderer, and host all key on this one slug.
        assert_eq!(slug("Zone Comfort"), "zone-comfort");
        assert_eq!(slug("AHU Status"), "ahu-status");
        assert_eq!(slug("  Host CPU + Mem  "), "host-cpu-mem");
        assert_eq!(slug("already-slug"), "already-slug");
    }

    #[test]
    fn widget_id_defaults_to_label_slug() {
        // Legacy manifest (no `id`) — the additive guarantee: today's exact label-slug key, no options.
        let toml = with_runtime(
            "wasm",
            "[[widget]]\nentry = \"w.mjs\"\nlabel = \"Zone Comfort\"",
        );
        let m = Manifest::parse(&toml).expect("parses");
        assert_eq!(m.widgets[0].widget_id(), "zone-comfort");
        assert!(m.widgets[0].id.is_none());
        assert!(m.widgets[0].options.is_empty());
    }

    #[test]
    fn parses_explicit_id_and_options() {
        let toml = with_runtime(
            "wasm",
            r#"
[[widget]]
entry = "w.mjs"
label = "Zone Comfort"
id = "zone-comfort"
data = true
options = [
  { id = "setpointField", label = "Setpoint field", scope = "options", path = "setpointField", control = "field-name" },
  { id = "band", label = "Comfort band", scope = "options", path = "band", control = "number", default = 1.5 },
]
"#,
        );
        let m = Manifest::parse(&toml).expect("parses");
        let w = &m.widgets[0];
        assert_eq!(w.widget_id(), "zone-comfort");
        assert_eq!(w.options.len(), 2);
        assert_eq!(w.options[0].id, "setpointField");
        assert_eq!(w.options[0].control, "field-name");
        assert_eq!(w.options[1].default, Some(serde_json::json!(1.5)));
    }

    #[test]
    fn rejects_duplicate_widget_ids() {
        // Two tiles that resolve to the same view key — ambiguous, a load-time reject.
        let toml = with_runtime(
            "wasm",
            "[[widget]]\nentry = \"a.mjs\"\nlabel = \"Zone Comfort\"\n[[widget]]\nentry = \"b.mjs\"\nlabel = \"Zone   Comfort\"",
        );
        let err = Manifest::parse(&toml).unwrap_err();
        assert!(
            matches!(err, ManifestError::InvalidWidgetBlock(_)),
            "{err:?}"
        );
    }

    #[test]
    fn rejects_duplicate_option_ids_within_a_widget() {
        let toml = with_runtime(
            "wasm",
            r#"
[[widget]]
entry = "w.mjs"
label = "W"
options = [
  { id = "band", label = "A", scope = "options", path = "a", control = "number" },
  { id = "band", label = "B", scope = "options", path = "b", control = "number" },
]
"#,
        );
        let err = Manifest::parse(&toml).unwrap_err();
        assert!(
            matches!(err, ManifestError::InvalidWidgetBlock(_)),
            "{err:?}"
        );
    }

    #[test]
    fn rejects_malformed_option_def() {
        // Empty control → reject loudly at publish (the scope's "malformed def rejected loudly").
        let toml = with_runtime(
            "wasm",
            "[[widget]]\nentry = \"w.mjs\"\nlabel = \"W\"\noptions = [ { id = \"x\", label = \"X\", scope = \"options\", path = \"x\", control = \"\" } ]",
        );
        let err = Manifest::parse(&toml).unwrap_err();
        assert!(
            matches!(err, ManifestError::InvalidWidgetBlock(_)),
            "{err:?}"
        );
    }

    #[test]
    fn unknown_control_is_allowed_at_parse() {
        // An unknown control is NOT a parse error — an older host degrades it to a raw row. Only an
        // EMPTY control is rejected.
        let toml = with_runtime(
            "wasm",
            "[[widget]]\nentry = \"w.mjs\"\nlabel = \"W\"\noptions = [ { id = \"x\", label = \"X\", scope = \"options\", path = \"x\", control = \"spinner\" } ]",
        );
        let m = Manifest::parse(&toml).expect("unknown control parses");
        assert_eq!(m.widgets[0].options[0].control, "spinner");
    }
}
