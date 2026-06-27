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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Public,
    Private,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Tool {
    pub name: String,
    #[serde(default)]
    pub description: String,
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
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
}
