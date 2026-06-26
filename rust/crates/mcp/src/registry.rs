//! The in-process registry of extensions hosted on this node: name → hosting instance + its
//! declared tools. `resolve` consults this to map `<ext>.<tool>` to a dispatch target.
//!
//! Tool names are registered from the manifest (extensions scope) so `resolve`/`authorize`
//! work without instantiating — a denied call is refused without ever starting the extension.
//! In S3 a `Hosted` gains a "remote node" variant; the registry shape already anticipates it.

use std::collections::HashMap;
use std::sync::Arc;

use lb_runtime::Instance;
use tokio::sync::Mutex;

/// A locally hosted extension: its declared tool names and its live instance.
pub struct Hosted {
    /// Tool names this extension declares (without the `<ext>.` prefix).
    pub tools: Vec<String>,
    /// The live WASM instance, behind a mutex (a tool call needs `&mut` on the wasm store).
    pub instance: Arc<Mutex<Instance>>,
}

/// All extensions hosted on this node, keyed by extension id.
#[derive(Default)]
pub struct Registry {
    hosted: HashMap<String, Hosted>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an extension by id with its declared tools and instance.
    pub fn register(&mut self, ext_id: impl Into<String>, tools: Vec<String>, instance: Instance) {
        self.hosted.insert(
            ext_id.into(),
            Hosted {
                tools,
                instance: Arc::new(Mutex::new(instance)),
            },
        );
    }

    /// Look up a hosted extension by id.
    pub(crate) fn get(&self, ext_id: &str) -> Option<&Hosted> {
        self.hosted.get(ext_id)
    }

    /// Is an extension with this id currently hosted? Lets the host distinguish a *reload*
    /// (swap an existing id) from a fresh install (stateless-extension hot-reload, §3.4).
    pub fn is_hosted(&self, ext_id: &str) -> bool {
        self.hosted.contains_key(ext_id)
    }
}
