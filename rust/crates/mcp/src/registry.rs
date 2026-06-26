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
use tokio::sync::Mutex;

/// A locally hosted extension: its declared tool names and its live instance.
#[derive(Clone)]
pub struct Hosted {
    /// Tool names this extension declares (without the `<ext>.` prefix).
    pub tools: Vec<String>,
    /// The live WASM instance, behind a mutex (a tool call needs `&mut` on the wasm store).
    pub instance: Arc<Mutex<Instance>>,
}

/// Where an extension's tools run, as seen from this node.
#[derive(Clone)]
pub enum Target {
    /// On this node — call the live instance directly (no bus hop).
    Local(Hosted),
    /// On another node — route the call over the bus queryable. Holds only the declared tool
    /// names (for `resolve`); the actual call is dispatched over `query` (no instance here).
    Remote { tools: Vec<String> },
}

impl Target {
    /// The tool names this target declares (local or remote) — what `resolve` matches against.
    pub fn tools(&self) -> &[String] {
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

    /// Register a **local** extension by id with its declared tools and instance.
    pub fn register(&self, ext_id: impl Into<String>, tools: Vec<String>, instance: Instance) {
        self.reachable.write().unwrap().insert(
            ext_id.into(),
            Target::Local(Hosted {
                tools,
                instance: Arc::new(Mutex::new(instance)),
            }),
        );
    }

    /// Register a **remote** extension by id with its declared tools. A call to one of these
    /// tools resolves here and dispatches over the bus to the hosting node (S3 routing seam).
    pub fn register_remote(&self, ext_id: impl Into<String>, tools: Vec<String>) {
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
}
