//! The merged `flows.nodes` registry — built-ins ∪ every installed extension's validated
//! `[[node]]` descriptors (node-descriptor-scope "The merged registry"). It is **derived, not
//! stored**: the host walks the workspace's install records (each carrying its parsed node blocks)
//! and unions them with the five built-ins. One read-only MCP verb, `flows.nodes`, returns this for
//! the calling workspace — the editor palette renders entirely from its response.
//!
//! The merge is symmetric (rule 1): built-in vs extension is data in the union, never an `if native`
//! branch. Two extensions may both ship a `publish`; the global `<ext_id>.<type>` namespace keeps
//! them distinct, mirroring `mcp:<id>.*`. The descriptor declares no capabilities — reading the
//! catalog reveals only *what could run*; the executing tool's own cap gates actual execution.

use crate::descriptor::NodeDescriptor;

/// Merge the built-in descriptors with a workspace's extension descriptors. Built-ins come first;
/// extension descriptors follow, sorted by `(ext_id, type)` for a stable palette order. Duplicate
/// global types (a broken install) are kept distinct — the host's install validation prevents them,
/// and a silent dedup here would hide a real collision.
pub fn merge_registry(
    builtins: Vec<NodeDescriptor>,
    mut extension: Vec<NodeDescriptor>,
) -> Vec<NodeDescriptor> {
    extension.sort_by(|a, b| a.r#type.cmp(&b.r#type));
    let mut out = builtins;
    out.append(&mut extension);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptor::NodeKind;

    #[test]
    fn builtins_first_then_extension_sorted() {
        let builtins = vec![
            NodeDescriptor::new("trigger", NodeKind::Trigger, ""),
            NodeDescriptor::new("tool", NodeKind::Transform, ""),
        ];
        let ext = vec![
            NodeDescriptor::new("zeta.z", NodeKind::Sink, "zeta.z"),
            NodeDescriptor::new("alpha.a", NodeKind::Sink, "alpha.a"),
        ];
        let r = merge_registry(builtins, ext);
        let types: Vec<&str> = r.iter().map(|d| d.r#type.as_str()).collect();
        assert_eq!(types, vec!["trigger", "tool", "alpha.a", "zeta.z"]);
    }

    #[test]
    fn empty_extension_is_just_builtins() {
        let builtins = vec![NodeDescriptor::new("trigger", NodeKind::Trigger, "")];
        let r = merge_registry(builtins.clone(), vec![]);
        assert_eq!(r.len(), 1);
    }
}
