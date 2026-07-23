//! The verb-class policy — the ONE declarative place a host verb's cacheability is stated
//! (response-cache scope, Intent §2). The middleware never hardcodes a verb name in an `if`; it
//! asks this table, so swapping/adding a verb is a data change here, not a code change in the
//! cache path (rule 10).
//!
//! Two halves:
//!   - [`read_class`] — the v1 **allowlist**: the read verbs proven caller-independent by the
//!     build's grant-filtering audit (`viz.query` was audited SUBJECT-FILTERED and is DEFERRED —
//!     see the module `cache` doc and the session doc). Each maps to a [`Class`] whose generation
//!     it reads into its key.
//!   - [`dirties`] — the write→dirty map: which classes a write verb invalidates. A successful
//!     write bumps those classes' generations, so every stale entry becomes unreachable at once
//!     (Intent §3). Coarse **by design** — per class, not per entity.
//!
//! > Long-term home (named follow-up): declare `cache_class` / `dirties` on each verb's
//! > `ToolDescriptor` so the staleness sweep is mechanical over *every* registered verb and a
//! > write missing from the map is a compile-visible gap. v1 keeps the map here because the
//! > allowlist is five reads and a handful of writes — small enough to state and test by hand.

/// A cache class — a group of cached reads sharing one per-workspace generation counter. A write
/// that changes a class's underlying data bumps that class, invalidating exactly those reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Class {
    /// `datasource.list`.
    Datasource,
    /// `series.list`.
    Series,
    /// `flows.list` / `flows.get`.
    Flows,
    /// `ext.list`. NOTE: its liveness fields (`running`/`restart_count`) are process state that no
    /// store write covers — no MCP write verb dirties this class, so `ext.list` staleness is bounded
    /// by the list TTL (the documented operator expectation, like an external sqlite writer). An
    /// install/sidecar-transition generation bump is a hardening follow-up.
    Ext,
    /// `viz.query` — the SUBJECT-SCOPED class (dashboard-query-acceleration scope, slice 2). Unlike
    /// the caller-independent lists above, `viz.query` re-authorizes each panel target under the
    /// caller's grants (a denied target → empty frame), so its result varies by caller. It is safe to
    /// cache ONLY under a key that also folds a **capability fingerprint** ([`is_subject_scoped`]) — a
    /// stable hash of the caller's relevant grants — so a warm hit is provably the frame THAT caller
    /// would have computed and a denied target can never leak across the wall through a warm entry.
    /// Its underlying data is external (federation), so no MCP write dirties it: staleness is
    /// TTL/time-bucket-bounded (the quantiser), like the `Ext` class's external-writer case.
    VizSubjectScoped,
}

/// Every class — the target of a coarse "nuke this workspace" invalidation (a generic `store.write`
/// could touch any cached domain, and `cache.purge` clears the lot).
pub const ALL_CLASSES: &[Class] = &[
    Class::Datasource,
    Class::Series,
    Class::Flows,
    Class::Ext,
    Class::VizSubjectScoped,
];

/// Does this class require a per-caller **capability fingerprint** folded into its key? True only for
/// [`Class::VizSubjectScoped`] — the ONE subject-filtered read. The cache middleware asks this (never
/// an `if verb == "viz.query"`) to decide whether to take the fingerprinted + quantised path (rule 10:
/// the subject-scoping is a property of the class in this table, not a verb name in the cache code).
pub fn is_subject_scoped(class: Class) -> bool {
    matches!(class, Class::VizSubjectScoped)
}

/// The v1 read allowlist. `Some(class)` ⇒ this verb's response is cacheable under
/// `{ws, verb, canonical-args, generation(class)}`; `None` ⇒ uncacheable (dispatched every call).
///
/// Every verb here was proven a pure function of `{ws, args}` by the grant-filtering audit — a
/// coarse verb-level cap gate with no per-row/subject narrowing. `viz.query` is deliberately absent:
/// it re-authorizes each panel target under the caller's grants (a denied target → empty frame), so
/// its result varies by caller and a subject-free key would leak. It re-enters the allowlist only
/// once keyed safely (a `subject_scoped` class folding a capability fingerprint into the key).
pub fn read_class(verb: &str) -> Option<Class> {
    Some(match verb {
        "datasource.list" => Class::Datasource,
        "series.list" => Class::Series,
        "flows.list" | "flows.get" => Class::Flows,
        "ext.list" => Class::Ext,
        // The subject-scoped re-entry (slice 2): cacheable ONLY via the fingerprinted + quantised path
        // ([`is_subject_scoped`]). A subject-free key would leak a privileged caller's frames — hence
        // the dedicated class rather than a row in the caller-independent allowlist above.
        "viz.query" => Class::VizSubjectScoped,
        _ => return None,
    })
}

/// The classes a write verb invalidates. Empty ⇒ this write touches no cached read (the common
/// case — `dashboard.save`, `rules.save`, `channel.post` change data no cacheable verb reads, and
/// `viz.query` is keyed by the full panel arg so an edited panel is already a different key).
///
/// The generic `store.write` / `store.delete` can mutate any table, so they conservatively nuke
/// **all** classes — over-invalidation is safe (it only lowers the hit rate), under-invalidation
/// would serve stale pages (Risks: "invalidation completeness is the whole ballgame").
pub fn dirties(verb: &str) -> &'static [Class] {
    match verb {
        // Datasource CRUD → datasource.list. `federation.*` write verbs mutate the same records.
        "datasource.add" | "datasource.remove" | "datasource.test" | "federation.write"
        | "federation.delete" | "federation.migrate" => &[Class::Datasource],
        // A durable sample append can introduce a new series name → series.list.
        "ingest.write" => &[Class::Series],
        // Flow authoring → flows.list / flows.get.
        "flows.save" | "flows.delete" | "flows.enable" | "flows.node.update" => &[Class::Flows],
        // Generic per-table store mutation: any cached domain could be the table. Nuke all (coarse,
        // safe). A future per-table classifier could narrow this; v1 favours correctness.
        "store.write" | "store.delete" => ALL_CLASSES,
        _ => &[],
    }
}
