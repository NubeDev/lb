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
    /// Soft-delete tombstone (idempotent delete; §6.8 sync-safe). A tombstoned rule reads as absent.
    #[serde(default)]
    pub deleted: bool,
}

/// The store table for saved rules.
pub const RULE_TABLE: &str = "rule";
