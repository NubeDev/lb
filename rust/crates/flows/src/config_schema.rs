//! Compile + validate a node's inline JSON-Schema config (Decision 3 — JSON-Schema 2020-12). The
//! manifest's `[node.config]` table is compiled as a schema at load (a `config` that isn't valid
//! JSON-Schema is a reject), and a node's saved config **instance** is validated against it at save
//! + at `flows.patch_run` (against the pinned descriptor, Decision 12). One dialect, host-side
//! (`jsonschema`/Boon) and editor-side (`ajv`), so a bad config is caught both before save and run.

use jsonschema::Validator;
use thiserror::Error;

/// A config-schema error: either the schema itself does not compile (a malformed `[[node]].config`)
/// or a config instance violates its schema (a precise, node+rule-tagged validation failure).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigSchemaError {
    /// The schema table is not a valid JSON-Schema 2020-12 document (a load-time manifest reject).
    #[error("config schema is not valid JSON-Schema: {0}")]
    InvalidSchema(String),
    /// A config instance violated its schema. The message names the failing rule precisely so an
    /// author reconciles mechanically (node-descriptor-scope "the precise-error requirement").
    #[error("config does not match schema: {0}")]
    InvalidInstance(String),
}

/// Compile a schema document. Returns the cached [`Validator`] or an error if it isn't valid
/// JSON-Schema 2020-12. Used at manifest load to reject a non-schema `config` up front.
pub fn compile_schema(schema: &serde_json::Value) -> Result<Validator, ConfigSchemaError> {
    jsonschema::validator_for(schema).map_err(|e| ConfigSchemaError::InvalidSchema(e.to_string()))
}

/// Validate `instance` against `schema`. A `None` schema (or `{}`) accepts anything (a node with no
/// config). Returns the precise first failing rule on a mismatch.
pub fn validate_config(schema: &serde_json::Value, instance: &serde_json::Value) -> Result<(), ConfigSchemaError> {
    // `{}` and a missing schema are "accept anything" — a node with no config form. Compile once
    // and let the validator decide; an empty object compiles to a pass-everything schema.
    let validator = compile_schema(schema)?;
    let result = validator.validate(instance);
    if let Err(e) = result {
        return Err(ConfigSchemaError::InvalidInstance(e.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn empty_schema_accepts_anything() {
        validate_config(&json!({}), &json!({"anything": 1})).expect("empty schema = pass");
        validate_config(&json!({}), &json!(null)).expect("empty schema = pass");
    }

    #[test]
    fn rejects_a_non_schema_config() {
        // A config whose `type` is not a real JSON-Schema type does not compile.
        let err = compile_schema(&json!({"type": "not-a-type"})).unwrap_err();
        assert!(matches!(err, ConfigSchemaError::InvalidSchema(_)));
    }

    #[test]
    fn validates_a_required_field() {
        let schema = json!({
            "type": "object",
            "required": ["topic"],
            "additionalProperties": false,
            "properties": {
                "topic": {"type": "string"},
                "qos": {"type": "integer", "enum": [0, 1, 2], "default": 0}
            }
        });
        validate_config(&schema, &json!({"topic": "sensors/x", "qos": 1})).expect("valid instance");
        // missing required `topic`
        let err = validate_config(&schema, &json!({"qos": 1})).unwrap_err();
        assert!(matches!(err, ConfigSchemaError::InvalidInstance(_)));
    }

    #[test]
    fn rejects_a_wrong_enum_value() {
        let schema = json!({"type": "integer", "enum": [0, 1, 2]});
        let err = validate_config(&schema, &json!(9)).unwrap_err();
        assert!(matches!(err, ConfigSchemaError::InvalidInstance(_)));
    }

    #[test]
    fn rejects_additional_properties() {
        let schema = json!({"type": "object", "additionalProperties": false, "properties": {"a": {"type": "string"}}});
        let err = validate_config(&schema, &json!({"a": "x", "b": 1})).unwrap_err();
        assert!(matches!(err, ConfigSchemaError::InvalidInstance(_)));
    }
}
