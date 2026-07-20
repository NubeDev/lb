//! The registry of extensions reachable from this node: name → where its tools run. An
//! extension is either **local** (a live wasm instance on this node) or **remote** (hosted on
//! another node, reached over a Zenoh queryable). `resolve` consults this to map `<ext>.<tool>`
//! to a dispatch target; `dispatch` then calls locally or routes — the seam shaped in S1/S2 and
//! made real in S3.
//!
//! Tool names are registered from the manifest (extensions scope) so `resolve`/`authorize`
//! work without instantiating — a denied call is refused without ever starting (or reaching)
//! the extension. Authorization runs on THIS (the calling) node, workspace-first, before any
//! routing — the remote node never sees an unauthorized call (mcp scope, §3.5).
//!
//! The map is behind an `RwLock` so the registry can be shared as one `Arc<Registry>` across
//! the local call path, the routed serve loop, AND `reload` (which swaps an instance in place)
//! — one source of truth, no clones that drift on reload.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

use lb_bus::NodeId;
use lb_runtime::{Instance, LocalDispatch};
use serde::Serialize;
use serde_json::Value;
use tokio::sync::Mutex;

/// A declared tool's **descriptor** — its bare name (no `<ext>.` prefix for an extension tool, the
/// qualified name for a host-native verb), a human title, a UI group bucket, and an optional
/// standard **JSON Schema** for its input object (channels-command-palette scope). Per-property
/// vendor hints live under an `x-lb` key inside `input_schema` (`x-lb-entity`, `x-lb-widget`).
///
/// `input_schema = None` is valid and additive: a tool with no declared schema still appears in the
/// catalog and dispatches — the palette renders a single free-text arg (an old extension needs no
/// rebuild). This widening of the registry from bare names (`Vec<String>`) to descriptors is the
/// SDK/manifest-adjacent change; it is versioned by absence (old manifests simply omit the field).
#[derive(Debug, Clone, Serialize)]
pub struct ToolDescriptor {
    /// The tool name. For an extension tool this is the BARE name (the `<ext>.` prefix is added by
    /// the catalog); for a host-native descriptor it is the qualified name (`federation.query`).
    pub name: String,
    /// A human title (the palette's menu label). Falls back to the name when empty.
    pub title: String,
    /// The UI group bucket (a verb-family prefix, or the contributing extension id).
    pub group: String,
    /// A standard JSON Schema (`type:"object"`, `properties`, `required`) describing the input, or
    /// `None` when the tool declares none (degrades to a single free-text arg).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<Value>,
    /// The tool **can transmit data off the node** (send a message, fetch a URL, call a webhook) —
    /// self-declared, exactly like every other descriptor field, so no core list of tool names
    /// exists (rule 10; agent-loop-hardening slice E). Consumed generically: a run flagged
    /// `exfiltration_guard` excludes tainted tools from its advertised menu AND denies them at
    /// dispatch. Versioned by absence — an old manifest/descriptor simply omits it (false). Trust
    /// model: a tool that lies is not caught; the guard is defense-in-depth over the capability
    /// wall, not a replacement.
    #[serde(default, skip_serializing_if = "core::ops::Not::not")]
    pub emits_external: bool,
    /// The response render envelope (`x-lb-render`) this command's answer mounts as — the v2 rich-result
    /// shape (`{ v, view, source?, options?, action?, tools? }`). When set, the palette POSTS this render
    /// (interpolating the collected args into `source.args`) instead of showing a raw tool result; the
    /// channel mounts it through the shipped `WidgetView`. `None` → the command is a plain call. This is
    /// the OUTPUT contract that keeps the frontend generic (it never hardcodes what a command renders as).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
}

impl ToolDescriptor {
    /// A name-only descriptor (no schema) — the backward-compatible shape a bare tool name maps to.
    pub fn name_only(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            title: String::new(),
            group: String::new(),
            input_schema: None,
            emits_external: false,
            result: None,
        }
    }
}

/// A locally hosted extension: its declared tool descriptors and its live dispatch target.
#[derive(Clone)]
pub struct Hosted {
    /// The tools this extension declares (descriptors; bare names without the `<ext>.` prefix).
    pub tools: Vec<ToolDescriptor>,
    /// The live dispatch target, behind a mutex (a tool call needs `&mut` on it). This is Tier-
    /// agnostic: a wasm `Instance` and a native-sidecar adapter both impl [`LocalDispatch`], so
    /// `dispatch`/`serve_call` reach either through the ONE trait — no per-Tier branch (§3.1).
    pub instance: Arc<Mutex<dyn LocalDispatch>>,
}

/// Where an extension's tools run, as seen from this node.
#[derive(Clone)]
pub enum Target {
    /// On this node — call the live instance directly (no bus hop).
    Local(Hosted),
    /// On another node — route the call over the bus queryable. Holds the declared tool
    /// descriptors (for `resolve`) and **which node hosts them**; the actual call is dispatched
    /// over `query` (no instance here).
    ///
    /// `node` is what makes a multiply-hosted extension REPRESENTABLE (routed-node-dispatch, #81).
    /// Before it, `register_remote_descriptors` overwrote the prior entry for an ext id, so a
    /// second host was not merely unaddressable — it could not be recorded at all, and the
    /// ambiguity was invisible to the calling node.
    ///
    /// Deliberately **singular** (scope, open question 3): a plural `Target` would be a target
    /// that is still ambiguous *after* resolve, which defeats the guard by construction. The
    /// registry holds a `Vec<Target>` per ext instead; ambiguity is resolved at resolve or not
    /// at all.
    Remote {
        node: NodeId,
        tools: Vec<ToolDescriptor>,
    },
}

impl Target {
    /// The tool NAMES this target declares (local or remote) — what `resolve` matches against.
    /// Kept for callers that need only names (`collect_tools`, `summary`); the full descriptors are
    /// available via [`Target::descriptors`].
    pub fn tools(&self) -> Vec<String> {
        match self {
            Target::Local(h) => h.tools.iter().map(|d| d.name.clone()).collect(),
            Target::Remote { tools, .. } => tools.iter().map(|d| d.name.clone()).collect(),
        }
    }

    /// The full tool descriptors this target declares (names + schemas). The catalog walks these.
    pub fn descriptors(&self) -> &[ToolDescriptor] {
        match self {
            Target::Local(h) => &h.tools,
            Target::Remote { tools, .. } => tools,
        }
    }

    /// The node hosting this target, or `None` when it is local to this node. `resolve` uses it to
    /// match an explicit target and to name candidates in [`ToolError::Ambiguous`].
    pub fn node(&self) -> Option<&NodeId> {
        match self {
            Target::Local(_) => None,
            Target::Remote { node, .. } => Some(node),
        }
    }
}

/// All extensions reachable from this node, keyed by extension id. Local instances live here;
/// remote ones are routing entries so a call resolves and routes transparently.
///
/// **One ext id maps to N targets** (routed-node-dispatch, #81), because a fleet can run the same
/// extension on many nodes — ten gateways each hosting `modbus`. Before #81 this was
/// `HashMap<String, Target>`, so registering a second host silently *overwrote* the first: the
/// multiply-hosted case was not representable, and therefore not detectable. Holding a `Vec` is
/// what lets `resolve` see the ambiguity and refuse it instead of coin-flipping on the bus.
///
/// Invariant: **at most one `Target::Local` per ext**, and a local target is authoritative — if
/// this node hosts the ext itself, that is where a call runs (no bus hop). Remote targets are
/// keyed by node id; re-registering the same node replaces its entry rather than duplicating it.
#[derive(Default)]
pub struct Registry {
    reachable: RwLock<HashMap<String, Vec<Target>>>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a **local** extension by id with its declared tool names and instance. Each bare
    /// name maps to a schema-less descriptor (backward compatible — an old caller still works); a
    /// caller with manifests carrying `input_schema` uses [`Registry::register_descriptors`].
    pub fn register(&self, ext_id: impl Into<String>, tools: Vec<String>, instance: Instance) {
        let descriptors = tools.into_iter().map(ToolDescriptor::name_only).collect();
        self.register_descriptors(ext_id, descriptors, instance);
    }

    /// Register a **local** extension by id with its full tool descriptors (names + input schemas)
    /// and instance. This is the widened path the catalog reads (channels-command-palette scope).
    pub fn register_descriptors(
        &self,
        ext_id: impl Into<String>,
        tools: Vec<ToolDescriptor>,
        instance: Instance,
    ) {
        // Box the wasm instance as the generic dispatch target — the registry is Tier-agnostic.
        self.register_local_dispatch(ext_id, tools, Arc::new(Mutex::new(instance)));
    }

    /// Register a **local** extension by id with its tool descriptors and an arbitrary
    /// [`LocalDispatch`] target. This is the Tier-agnostic entry: a wasm instance goes through
    /// [`register_descriptors`](Registry::register_descriptors) (which boxes it); a native-sidecar
    /// adapter goes here directly. `resolve`/`dispatch`/`serve_call` reach both identically.
    pub fn register_local_dispatch(
        &self,
        ext_id: impl Into<String>,
        tools: Vec<ToolDescriptor>,
        dispatch: Arc<Mutex<dyn LocalDispatch>>,
    ) {
        let local = Target::Local(Hosted {
            tools,
            instance: dispatch,
        });
        let mut map = self.reachable.write().unwrap();
        let targets = map.entry(ext_id.into()).or_default();
        // At most one local target per ext: a reload SWAPS the instance in place rather than
        // stacking a second one (`is_hosted` distinguishes reload from fresh install). Remote
        // entries for the same ext are left untouched — this node hosting `modbus` says nothing
        // about whether a gateway also does.
        match targets.iter_mut().find(|t| matches!(t, Target::Local(_))) {
            Some(existing) => *existing = local,
            None => targets.push(local),
        }
    }

    /// Register a **remote** extension by id, hosted on `node`, with its declared tool names. A
    /// call to one of these tools resolves here and dispatches over the bus to that node.
    pub fn register_remote(&self, ext_id: impl Into<String>, node: NodeId, tools: Vec<String>) {
        let descriptors = tools.into_iter().map(ToolDescriptor::name_only).collect();
        self.register_remote_descriptors(ext_id, node, descriptors);
    }

    /// Register a **remote** extension by id, hosted on `node`, with its full tool descriptors.
    ///
    /// Registering a *different* node for an ext that already has one ADDS a target — that is the
    /// multiply-hosted fleet case, and it is precisely what makes an untargeted call ambiguous.
    /// Registering the SAME node again replaces its entry (a re-announce, not a second host);
    /// keyed on node id, so an announce arriving twice cannot inflate the candidate list and
    /// manufacture a phantom ambiguity.
    pub fn register_remote_descriptors(
        &self,
        ext_id: impl Into<String>,
        node: NodeId,
        tools: Vec<ToolDescriptor>,
    ) {
        let remote = Target::Remote {
            node: node.clone(),
            tools,
        };
        let mut map = self.reachable.write().unwrap();
        let targets = map.entry(ext_id.into()).or_default();
        match targets.iter_mut().find(|t| t.node() == Some(&node)) {
            Some(existing) => *existing = remote,
            None => targets.push(remote),
        }
    }

    /// Forget the remote target for `ext_id` on `node` — the calling-side reaction to a hosting
    /// announce being retracted (the node dropped, or stopped hosting the ext). Removing a host
    /// is what lets a formerly-ambiguous ext become unambiguous again, so a fleet that shrinks to
    /// one node stops refusing untargeted calls without needing a restart.
    pub fn forget_remote(&self, ext_id: &str, node: &NodeId) {
        let mut map = self.reachable.write().unwrap();
        if let Some(targets) = map.get_mut(ext_id) {
            targets.retain(|t| t.node() != Some(node));
            if targets.is_empty() {
                map.remove(ext_id);
            }
        }
    }

    /// Every target reachable for `ext_id`, cloned out. Cloning is cheap — a local target shares
    /// the instance `Arc`, so the returned target dispatches to the very same instance (and
    /// reflects a reload that swapped it). Empty vec (not `None`) when nothing hosts the ext.
    pub(crate) fn targets(&self, ext_id: &str) -> Vec<Target> {
        self.reachable
            .read()
            .unwrap()
            .get(ext_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Is an extension with this id currently hosted **locally**? Lets the host distinguish a
    /// *reload* (swap an existing local id) from a fresh install (§3.4).
    pub fn is_hosted(&self, ext_id: &str) -> bool {
        self.reachable
            .read()
            .unwrap()
            .get(ext_id)
            .is_some_and(|targets| targets.iter().any(|t| matches!(t, Target::Local(_))))
    }

    /// Every reachable extension and the tool names it declares, as `(ext_id, names)` pairs — what
    /// `system.tools` walks for the names-only console table. Cloned out under the read lock so the
    /// caller holds no lock; ordering is unspecified (the caller sorts). The schema-bearing catalog
    /// uses [`Registry::descriptor_entries`].
    /// With a multiply-hosted ext the tool NAMES are deduplicated across its targets: two gateways
    /// running `modbus` expose one `modbus.device.add` to a caller, not two. Which node runs it is
    /// an addressing question (`Ambiguous` / an explicit target), not a catalog one — showing the
    /// same tool twice would misrepresent the surface.
    pub fn entries(&self) -> Vec<(String, Vec<String>)> {
        self.reachable
            .read()
            .unwrap()
            .iter()
            .map(|(id, targets)| (id.clone(), dedup_names(targets)))
            .collect()
    }

    /// Every reachable extension and its full tool descriptors, as `(ext_id, descriptors)` pairs —
    /// what `tools.catalog` walks to surface the schema-bearing palette (channels-command-palette
    /// scope). Local and remote targets both appear; ordering is unspecified (the caller sorts).
    /// Descriptors are deduplicated by tool name across a multiply-hosted ext's targets, for the
    /// same reason as [`Registry::entries`]. **A local target's descriptors win** when both a local
    /// and a remote host declare the same tool: descriptors may legitimately differ across a fleet
    /// mid-rolling-upgrade (scope, open question 4 — a per-node fact, not an error), and this node's
    /// own copy is the one it can vouch for.
    pub fn descriptor_entries(&self) -> Vec<(String, Vec<ToolDescriptor>)> {
        self.reachable
            .read()
            .unwrap()
            .iter()
            .map(|(id, targets)| (id.clone(), dedup_descriptors(targets)))
            .collect()
    }

    /// A live count of reachable extensions and the total tools they expose — the real numbers a
    /// system map shows for the MCP/runtime card (it answers "how big is the tool surface right
    /// now", not "does the registry handle exist"). Cheap: a read-locked walk of the routing map.
    pub fn summary(&self) -> RegistrySummary {
        let map = self.reachable.read().unwrap();
        // Deduplicated per ext (see `entries`): the surface is "how many distinct tools can I
        // call", not "how many nodes could answer" — counting a fleet's copies would inflate the
        // number with fleet size and make the system map read as growth where there is none.
        let tools = map.values().map(|targets| dedup_names(targets).len()).sum();
        RegistrySummary {
            extensions: map.len(),
            tools,
        }
    }
}

/// The distinct tool names across an ext's targets, order-stable (first target wins position).
fn dedup_names(targets: &[Target]) -> Vec<String> {
    let mut seen = HashSet::new();
    targets
        .iter()
        .flat_map(|t| t.tools())
        .filter(|name| seen.insert(name.clone()))
        .collect()
}

/// The distinct tool descriptors across an ext's targets, LOCAL FIRST so a local declaration wins
/// a name collision (see [`Registry::descriptor_entries`]).
fn dedup_descriptors(targets: &[Target]) -> Vec<ToolDescriptor> {
    let mut seen = HashSet::new();
    targets
        .iter()
        .filter(|t| matches!(t, Target::Local(_)))
        .chain(targets.iter().filter(|t| matches!(t, Target::Remote { .. })))
        .flat_map(|t| t.descriptors().to_vec())
        .filter(|d| seen.insert(d.name.clone()))
        .collect()
}

/// A live rollup of the registry: how many extensions are reachable and how many tools they expose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegistrySummary {
    pub extensions: usize,
    pub tools: usize,
}
