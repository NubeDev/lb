//! The saved-rule record `rule:{ws}:{id}` (rules-engine-scope: "saved rules are SurrealDB records,
//! one datastore"). Body is Rhai source; declared params are a typed list. Workspace-walled like any
//! record — the `ws` is the store namespace, the `id` is the record key.

use serde::{Deserialize, Serialize};

use lb_rules::{RuleParam, RuleSchedule};

/// The persisted shape of a saved rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedRule {
    pub id: String,
    pub name: String,
    pub body: String,
    #[serde(default)]
    pub params: Vec<RuleParam>,
    /// The compiled `#[schedule(...)]` directive (scheduled-rules-scope), if the body carries one.
    /// Additive serde default: a rule written before scheduling deserialises as `None` (run-on-demand).
    /// The directive is parsed at **save**, never executed — it is the source of truth the syncer
    /// compiles into a managed `cron → rule` flow. `None` ⇒ any managed flow is torn down on save.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule: Option<RuleSchedule>,
    /// The subject that last saved this rule WITH a `#[schedule(...)]` directive — the **owner** a
    /// headless fire runs as (scheduled-rules-scope; the run-as-owner slot `ext-store-nodes-scope`
    /// §348 reserved). Stored, never trusted as a credential: it is an IDENTITY, and the fire path
    /// re-resolves that identity's caps from the live grant store on every run
    /// (`resolve_caps_live`). So a demoted/removed author's schedule loses the same reach they did,
    /// on the next fire — there is no standing credential frozen into the record.
    ///
    /// Additive serde default: a rule saved before this field deserialises as `None`, which means
    /// "fall back to the reactor's own principal" — the pre-existing behaviour, no migration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduled_by: Option<String>,
    /// Soft-delete tombstone (idempotent delete; §6.8 sync-safe). A tombstoned rule reads as absent.
    #[serde(default)]
    pub deleted: bool,
}

/// The store table for saved rules.
pub const RULE_TABLE: &str = "rule";
