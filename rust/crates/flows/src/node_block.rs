//! The additive `[[node]]` manifest block (node-descriptor-scope). One `[[node]]` per backend node
//! type an extension contributes; each binds to one `[[tools]]` entry that executes it. This is the
//! **only** manifest addition (the §11.2 forever-ish gate). The block is parsed by `lb-ext-loader`
//! (which already holds the `[[tools]]` list); here we own the [`NodeBlock`] raw shape + the
//! validation that lifts it into a [`NodeDescriptor`].
//!
//! Validation rules that bite (node-descriptor-scope "Field rules that bite"):
//! - **`tool` must name a `[[tools]]` entry in the same manifest.** A dangling `tool` is a
//!   load-time reject (the manifest is incoherent — a node that cannot execute).
//! - **`config` must compile as JSON-Schema 2020-12.** A `config` that isn't a schema is a reject.
//! - The global node type is `<ext_id>.<type>` (the ext-unique `type` namespaced by `ext_id`),
//!   mirroring how `mcp:<id>.*` namespaces caps — two extensions may both ship a `publish`.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config_schema::{compile_schema, ConfigSchemaError};
use crate::descriptor::{NodeDescriptor, NodeKind};

/// The raw `[[node]]` manifest table — the additive block `lb-ext-loader` deserialises alongside the
/// existing `[[tools]]`. Only `type`, `kind`, and `tool` are required; `title`/`category`/ports/
/// `config_version` default, and `config` defaults to `{}` (accept anything). Persisted verbatim on
/// the `Install` record (additive `nodes` field) so `flows.nodes` is a read-time union, never stored
/// twice. Serde-defaulted so an install written before this field deserialises as empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeBlock {
    pub r#type: String,
    pub kind: NodeKind,
    pub tool: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub inputs: Vec<String>,
    #[serde(default)]
    pub outputs: Vec<String>,
    #[serde(default = "default_config_version")]
    pub config_version: u32,
    #[serde(default = "default_config")]
    pub config: serde_json::Value,
}

fn default_config_version() -> u32 {
    1
}
fn default_config() -> serde_json::Value {
    serde_json::json!({})
}

/// A `[[node]]` block validation error — a load-time manifest reject.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum NodeBlockError {
    /// `tool` names a `[[tools]]` entry that does not exist in this manifest (a node that cannot run).
    #[error("node `{0}` binds tool `{1}` which is not a declared [[tools]] entry")]
    UnknownTool(String, String),
    /// The `[node.config]` table is not a valid JSON-Schema 2020-12 document.
    #[error("node `{0}` has an invalid config schema: {1}")]
    InvalidConfigSchema(String, String),
}

/// Validate a `[[node]]` block: the bound `tool` must exist in `manifest_tools` (the extension's
/// declared `[[tools]]` names) and the `config` must compile as JSON-Schema 2020-12. On success
/// returns the canonical [`NodeDescriptor`] with the global type `<ext_id>.<type>`.
pub fn validate_node_block(
    block: &NodeBlock,
    ext_id: &str,
    manifest_tools: &[String],
) -> Result<NodeDescriptor, NodeBlockError> {
    if !manifest_tools.iter().any(|t| t == &block.tool) {
        return Err(NodeBlockError::UnknownTool(
            block.r#type.clone(),
            block.tool.clone(),
        ));
    }
    compile_schema(&block.config).map_err(|e| match e {
        ConfigSchemaError::InvalidSchema(m) => {
            NodeBlockError::InvalidConfigSchema(block.r#type.clone(), m)
        }
        // compile_schema only ever returns InvalidSchema.
        other => NodeBlockError::InvalidConfigSchema(block.r#type.clone(), other.to_string()),
    })?;

    let global_type = format!("{ext_id}.{}", block.r#type);
    let mut desc = NodeDescriptor::new(global_type, block.kind, ext_tool(ext_id, &block.tool))
        .with_category(block.category.clone().unwrap_or_else(|| "General".into()));
    if let Some(title) = &block.title {
        desc = desc.with_title(title);
    }
    desc = desc.with_ports(block.inputs.clone(), block.outputs.clone());
    desc.config_version = block.config_version;
    desc.config = block.config.clone();
    Ok(desc)
}

/// The fully-qualified MCP tool an extension node dispatches: `<ext_id>.<tool>`.
fn ext_tool(ext_id: &str, tool: &str) -> String {
    format!("{ext_id}.{tool}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn block(kind: NodeKind, tool: &str) -> NodeBlock {
        NodeBlock {
            r#type: "publish".into(),
            kind,
            tool: tool.into(),
            title: None,
            category: None,
            inputs: vec![],
            outputs: vec![],
            config_version: 1,
            config: json!({}),
        }
    }

    #[test]
    fn valid_block_becomes_descriptor() {
        let b = block(NodeKind::Sink, "publish");
        let d = validate_node_block(&b, "mqtt", &["publish".into(), "subscribe".into()]).unwrap();
        assert_eq!(d.r#type, "mqtt.publish");
        assert_eq!(d.tool, "mqtt.publish");
        assert_eq!(d.kind, NodeKind::Sink);
        assert_eq!(d.config_version, 1);
    }

    #[test]
    fn rejects_a_dangling_tool_binding() {
        let b = block(NodeKind::Sink, "nope");
        let err = validate_node_block(&b, "mqtt", &["publish".into()]).unwrap_err();
        assert!(matches!(err, NodeBlockError::UnknownTool(_, _)));
    }

    #[test]
    fn rejects_a_non_schema_config() {
        let mut b = block(NodeKind::Sink, "publish");
        b.config = json!({"type": "not-a-type"});
        let err = validate_node_block(&b, "mqtt", &["publish".into()]).unwrap_err();
        assert!(matches!(err, NodeBlockError::InvalidConfigSchema(_, _)));
    }

    #[test]
    fn applies_optional_defaults() {
        let mut b = block(NodeKind::Source, "subscribe");
        b.inputs = vec![];
        b.outputs = vec!["sample".into()];
        b.title = Some("MQTT In".into());
        b.category = Some("Messaging".into());
        let d = validate_node_block(&b, "mqtt", &["subscribe".into()]).unwrap();
        assert_eq!(d.title, "MQTT In");
        assert_eq!(d.category, "Messaging");
        assert_eq!(d.outputs, vec!["sample".to_string()]);
    }
}
