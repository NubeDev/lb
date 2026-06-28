//! The saved-rule record `rule:{ws}:{id}` (rules-engine-scope: "saved rules are SurrealDB records,
//! one datastore"). Body is Rhai source; declared params are a typed list. Workspace-walled like any
//! record — the `ws` is the store namespace, the `id` is the record key.

use serde::{Deserialize, Serialize};

use lb_rules::RuleParam;

/// The persisted shape of a saved rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedRule {
    pub id: String,
    pub name: String,
    pub body: String,
    #[serde(default)]
    pub params: Vec<RuleParam>,
    /// Soft-delete tombstone (idempotent delete; §6.8 sync-safe). A tombstoned rule reads as absent.
    #[serde(default)]
    pub deleted: bool,
}

/// The store table for saved rules.
pub const RULE_TABLE: &str = "rule";
