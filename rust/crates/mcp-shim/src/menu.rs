//! Read the pre-baked menu file (the `tools/list` answer). The role crate writes this JSON into
//! the scratch dir at run start — it is the **narrowed** tool menu: `reachable_tools(caller) ∩
//! persona.granted_tools ∩ agent_caps`. Advertisement only: the wall is `caps::check` on the
//! gateway, which the shim cannot bypass (every `tools/call` is re-checked). A menu that lies
//! (names a tool the caller lacks) just produces a `403` on the call — honest by construction.
//!
//! The file is `[{name, description?, input_schema?}, ...]` — the MCP `Tool` shape minus the
//! `annotations` noise an external agent ignores. `description`/`input_schema` are optional so a
//! legacy menu (names only) still works.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One entry in the advertised menu. Maps 1:1 to an MCP `Tool`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MenuEntry {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "inputSchema"
    )]
    pub input_schema: Option<Value>,
}

impl MenuEntry {
    /// A name-only entry — the minimal menu shape (a tool with no schema/description).
    pub fn name_only(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            input_schema: None,
        }
    }
}

/// Read the menu JSON at `path`. A missing/unreadable/invalid file is a fatal misconfiguration
/// (the role crate always writes it) — the error reaches the bin's `main` and the shim exits
/// non-zero, so the agent sees a fast, clear failure rather than an empty menu that hides it.
pub fn load_menu(path: &Path) -> Result<Vec<MenuEntry>, String> {
    let bytes = fs::read(path).map_err(|e| format!("read menu {}: {e}", path.display()))?;
    let entries: Vec<MenuEntry> = serde_json::from_slice(&bytes)
        .map_err(|e| format!("parse menu {}: {e}", path.display()))?;
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_only_menu_round_trips() {
        let e = MenuEntry::name_only("tools.catalog");
        let j = serde_json::to_string(&e).unwrap();
        assert_eq!(j, "{\"name\":\"tools.catalog\"}");
    }

    #[test]
    fn full_entry_round_trips() {
        let j = r#"{"name":"devkit.scaffold","description":"Scaffold","inputSchema":{"type":"object"}}"#;
        let e: MenuEntry = serde_json::from_str(j).unwrap();
        assert_eq!(e.name, "devkit.scaffold");
        assert_eq!(e.description.as_deref(), Some("Scaffold"));
        assert!(e.input_schema.is_some());
        let back = serde_json::to_string(&e).unwrap();
        assert_eq!(back, j);
    }
}
