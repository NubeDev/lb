//! The `versions.*` tool descriptors — real JSON Schemas so a model (or the command palette) can
//! FORM the call instead of guessing arg names. The name-only-row failure mode is documented in
//! `debugging/agent/dashboard-save-cells-sent-as-json-string.md`: a verb advertised without a schema
//! gets called with invented arguments, turn after turn.
//!
//! `kind` carries its enum in the schema AND in the `x-lb` description, because the two audiences
//! differ: a validator reads `enum`, a model reads prose.

use lb_mcp::ToolDescriptor;
use serde_json::{json, Value};

use super::cap::{MAX_VERSION_CAP, MIN_VERSION_CAP};
use super::plan::KIND_PLANS;

/// The kinds, as a schema `enum` — derived from the plan table, so adding a kind cannot leave the
/// descriptors advertising a stale list (the drift that makes a catalog lie).
fn kind_enum() -> Vec<Value> {
    KIND_PLANS.iter().map(|p| json!(p.kind)).collect()
}

fn kind_prose() -> String {
    let names: Vec<&str> = KIND_PLANS.iter().map(|p| p.kind).collect();
    format!("The entity family: {}", names.join(", "))
}

fn kind_prop() -> Value {
    json!({
        "type": "string",
        "enum": kind_enum(),
        "x-lb": { "label": "Kind", "description": kind_prose() }
    })
}

fn id_prop() -> Value {
    json!({
        "type": "string",
        "x-lb": { "label": "Entity id", "description": "The id of the dashboard / flow / rule — the same id its own get/save verb takes" }
    })
}

fn version_id_prop() -> Value {
    json!({
        "type": "string",
        "x-lb": { "label": "Version id", "description": "A `version_id` from versions.list (a ULID). Versions are per-entity; one entity's id is never valid for another" }
    })
}

pub fn list_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        emits_external: false,
        name: "versions.list".to_string(),
        title: "List an entity's saved versions (newest first, metadata only)".to_string(),
        group: "versions".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "kind": kind_prop(),
                "id": id_prop(),
                "limit": {
                    "type": "integer",
                    "x-lb": { "label": "Limit", "description": "Max rows to return (default: the whole ring)" }
                }
            },
            "required": ["kind", "id"]
        })),
        result: None,
    }
}

pub fn get_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        emits_external: false,
        name: "versions.get".to_string(),
        title: "Read one saved version's full snapshot".to_string(),
        group: "versions".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "kind": kind_prop(),
                "id": id_prop(),
                "version_id": version_id_prop()
            },
            "required": ["kind", "id", "version_id"]
        })),
        result: None,
    }
}

pub fn restore_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        emits_external: false,
        name: "versions.restore".to_string(),
        title: "Restore a saved version (re-saves it as the live record)".to_string(),
        group: "versions".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "kind": kind_prop(),
                "id": id_prop(),
                "version_id": version_id_prop(),
                "now": {
                    "type": "integer",
                    "x-lb": { "label": "Timestamp", "description": "Logical time of the restore — unix epoch seconds. Omit to let the node stamp it" }
                }
            },
            "required": ["kind", "id", "version_id"]
        })),
        result: None,
    }
}

pub fn config_get_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        emits_external: false,
        name: "versions.config.get".to_string(),
        title: "Read how many versions this workspace keeps".to_string(),
        group: "versions".to_string(),
        input_schema: Some(json!({ "type": "object", "properties": {} })),
        result: None,
    }
}

pub fn config_set_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        emits_external: false,
        name: "versions.config.set".to_string(),
        title: "Set how many versions this workspace keeps (admin)".to_string(),
        group: "versions".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "cap": {
                    "type": "integer",
                    "x-lb": { "label": "Versions kept", "description": format!("Workspace-wide cap, {MIN_VERSION_CAP}–{MAX_VERSION_CAP}. Omit to leave unchanged") }
                },
                "per_kind": {
                    "type": "object",
                    "x-lb": { "label": "Per-kind overrides", "description": format!("e.g. {{\"dashboard\": 40}}, each {MIN_VERSION_CAP}–{MAX_VERSION_CAP}. Merged with what is stored; an explicit null clears one kind") }
                }
            }
        })),
        result: None,
    }
}

/// Every `versions.*` descriptor, for the host descriptor collector.
pub fn descriptors() -> Vec<ToolDescriptor> {
    vec![
        list_descriptor(),
        get_descriptor(),
        restore_descriptor(),
        config_get_descriptor(),
        config_set_descriptor(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The advertised kinds are DERIVED from the plan table — a new kind cannot leave the catalog
    /// advertising a stale enum (which is how a caller concludes a real kind does not exist).
    #[test]
    fn the_kind_enum_tracks_the_plan_table() {
        let d = list_descriptor();
        let schema = d.input_schema.expect("versions.list declares a schema");
        let advertised = schema["properties"]["kind"]["enum"]
            .as_array()
            .expect("kind is an enum")
            .len();
        assert_eq!(advertised, KIND_PLANS.len());
    }

    /// The catalog's cardinal rule in miniature: every verb the dispatcher routes declares a
    /// descriptor, and every descriptor is well-formed enough to validate an argument.
    #[test]
    fn every_descriptor_is_well_formed() {
        for d in descriptors() {
            assert!(d.name.starts_with("versions."), "{} is misnamed", d.name);
            assert!(!d.title.is_empty(), "{} has no title", d.name);
            assert_eq!(d.group, "versions");
            let s = d
                .input_schema
                .expect("every versions verb declares a schema");
            assert_eq!(s["type"], "object", "{} must take an object", d.name);
        }
    }

    /// A bad `kind` must be a typed argument error — the scope's "bad kind → typed error, not a
    /// store miss" catalog requirement, enforced at the schema layer before dispatch.
    #[test]
    fn a_non_string_kind_fails_validation() {
        let s = list_descriptor().input_schema.unwrap();
        let err =
            crate::tools::validate_args(Some(&s), &serde_json::json!({ "kind": 7, "id": "x" }))
                .unwrap_err();
        assert!(format!("{err:?}").contains("kind"));
    }

    #[test]
    fn a_missing_version_id_names_where_it_comes_from() {
        let s = get_descriptor().input_schema.unwrap();
        let err = crate::tools::validate_args(
            Some(&s),
            &serde_json::json!({ "kind": "dashboard", "id": "d" }),
        )
        .unwrap_err();
        assert!(
            format!("{err:?}").contains("versions.list"),
            "the miss must say where a version_id comes from"
        );
    }
}
