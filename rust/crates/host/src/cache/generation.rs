//! Per-`{workspace, class}` generation counters — the invalidation engine (response-cache scope,
//! Intent §3). A counter is part of every cache key, so bumping it makes every entry of that
//! `{ws, class}` instantly **unreachable** (a subsequent lookup computes a new key); the orphaned
//! entries age out via TTL/eviction. No scan-and-delete, no stored state.
//!
//! In-memory only — correct precisely because the cache it keys is also in-memory: both die together
//! on restart, so there is no window where a persisted counter and a cold cache disagree (that
//! coupling is the blocker the scope records against any future persistent warm tier).

use dashmap::DashMap;

use super::policy::{Class, ALL_CLASSES};

/// The live generation map. Starts empty (every `{ws, class}` implicitly at generation 0); a bump
/// creates or increments the entry. Concurrent-safe via `DashMap` — bumps are rare (writes), reads
/// are on the hot lookup path but cheap.
#[derive(Default)]
pub struct Generations {
    map: DashMap<(String, Class), u64>,
}

impl Generations {
    /// The current generation for `{ws, class}` — 0 until the first bump. Read into every cache key.
    pub fn current(&self, ws: &str, class: Class) -> u64 {
        self.map
            .get(&(ws.to_string(), class))
            .map(|v| *v)
            .unwrap_or(0)
    }

    /// Bump `{ws, class}` — a write landed. Every existing entry of this class in this workspace
    /// becomes unreachable at once. Returns the new generation.
    pub fn bump(&self, ws: &str, class: Class) -> u64 {
        let mut e = self.map.entry((ws.to_string(), class)).or_insert(0);
        *e += 1;
        *e
    }

    /// Bump **every** class for `ws` — the coarse workspace-wide invalidation `cache.purge` and a
    /// generic `store.write` use. Leaves other workspaces' generations untouched (the wall holds:
    /// purging A never disturbs B).
    pub fn bump_all(&self, ws: &str) {
        for c in ALL_CLASSES {
            self.bump(ws, *c);
        }
    }
}
