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

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use lb_runtime::Instance;
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
}

impl ToolDescriptor {
    /// A name-only descriptor (no schema) — the backward-compatible shape a bare tool name maps to.
    pub fn name_only(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            title: String::new(),
            group: String::new(),
            input_schema: None,
        }
    }
}

/// A locally hosted extension: its declared tool descriptors and its live instance.
#[derive(Clone)]
pub struct Hosted {
    /// The tools this extension declares (descriptors; bare names without the `<ext>.` prefix).
    pub tools: Vec<ToolDescriptor>,
    /// The live WASM instance, behind a mutex (a tool call needs `&mut` on the wasm store).
    pub instance: Arc<Mutex<Instance>>,
}

/// Where an extension's tools run, as seen from this node.
#[derive(Clone)]
pub enum Target {
    /// On this node — call the live instance directly (no bus hop).
    Local(Hosted),
    /// On another node — route the call over the bus queryable. Holds only the declared tool
    /// descriptors (for `resolve`); the actual call is dispatched over `query` (no instance here).
    Remote { tools: Vec<ToolDescriptor> },
}

impl Target {
    /// The tool NAMES this target declares (local or remote) — what `resolve` matches against.
    /// Kept for callers that need only names (`collect_tools`, `summary`); the full descriptors are
    /// available via [`Target::descriptors`].
    pub fn tools(&self) -> Vec<String> {
        match self {
            Target::Local(h) => h.tools.iter().map(|d| d.name.clone()).collect(),
            Target::Remote { tools } => tools.iter().map(|d| d.name.clone()).collect(),
        }
    }

    /// The full tool descriptors this target declares (names + schemas). The catalog walks these.
    pub fn descriptors(&self) -> &[ToolDescriptor] {
        match self {
            Target::Local(h) => &h.tools,
            Target::Remote { tools } => tools,
        }
    }
}

/// All extensions reachable from this node, keyed by extension id. Local instances live here;
/// remote ones are routing entries so a call resolves and routes transparently.
#[derive(Default)]
pub struct Registry {
    reachable: RwLock<HashMap<String, Target>>,
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
        self.reachable.write().unwrap().insert(
            ext_id.into(),
            Target::Local(Hosted {
                tools,
                instance: Arc::new(Mutex::new(instance)),
            }),
        );
    }

    /// Register a **remote** extension by id with its declared tool names. A call to one of these
    /// tools resolves here and dispatches over the bus to the hosting node (S3 routing seam).
    pub fn register_remote(&self, ext_id: impl Into<String>, tools: Vec<String>) {
        let descriptors = tools.into_iter().map(ToolDescriptor::name_only).collect();
        self.register_remote_descriptors(ext_id, descriptors);
    }

    /// Register a **remote** extension by id with its full tool descriptors (names + schemas).
    pub fn register_remote_descriptors(
        &self,
        ext_id: impl Into<String>,
        tools: Vec<ToolDescriptor>,
    ) {
        self.reachable
            .write()
            .unwrap()
            .insert(ext_id.into(), Target::Remote { tools });
    }

    /// Look up a reachable extension by id, returning a clone of its [`Target`]. Cloning is
    /// cheap — a local target shares the instance `Arc`, so the returned target dispatches to
    /// the very same instance (and reflects a reload that swapped it).
    pub(crate) fn get(&self, ext_id: &str) -> Option<Target> {
        self.reachable.read().unwrap().get(ext_id).cloned()
    }

    /// Is an extension with this id currently hosted **locally**? Lets the host distinguish a
    /// *reload* (swap an existing local id) from a fresh install (§3.4).
    pub fn is_hosted(&self, ext_id: &str) -> bool {
        matches!(
            self.reachable.read().unwrap().get(ext_id),
            Some(Target::Local(_))
        )
    }

    /// Every reachable extension and the tool names it declares, as `(ext_id, names)` pairs — what
    /// `system.tools` walks for the names-only console table. Cloned out under the read lock so the
    /// caller holds no lock; ordering is unspecified (the caller sorts). The schema-bearing catalog
    /// uses [`Registry::descriptor_entries`].
    pub fn entries(&self) -> Vec<(String, Vec<String>)> {
        self.reachable
            .read()
            .unwrap()
            .iter()
            .map(|(id, target)| (id.clone(), target.tools()))
            .collect()
    }

    /// Every reachable extension and its full tool descriptors, as `(ext_id, descriptors)` pairs —
    /// what `tools.catalog` walks to surface the schema-bearing palette (channels-command-palette
    /// scope). Local and remote targets both appear; ordering is unspecified (the caller sorts).
    pub fn descriptor_entries(&self) -> Vec<(String, Vec<ToolDescriptor>)> {
        self.reachable
            .read()
            .unwrap()
            .iter()
            .map(|(id, target)| (id.clone(), target.descriptors().to_vec()))
            .collect()
    }

    /// A live count of reachable extensions and the total tools they expose — the real numbers a
    /// system map shows for the MCP/runtime card (it answers "how big is the tool surface right
    /// now", not "does the registry handle exist"). Cheap: a read-locked walk of the routing map.
    pub fn summary(&self) -> RegistrySummary {
        let map = self.reachable.read().unwrap();
        let tools = map.values().map(|t| t.tools().len()).sum();
        RegistrySummary {
            extensions: map.len(),
            tools,
        }
    }
}

/// A live rollup of the registry: how many extensions are reachable and how many tools they expose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegistrySummary {
    pub extensions: usize,
    pub tools: usize,
}
