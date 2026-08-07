//! Join a native extension's **manifest tool list** with what its child **declared at `init`** into
//! the [`ToolDescriptor`]s the MCP registry holds (ext-tool-descriptors scope).
//!
//! Two sources describe the same tools and neither is redundant:
//!
//! - The **manifest** is authoritative for *which* tools exist. It is the capability source — every
//!   `mcp:<ext>.<tool>:call` an admin approves is grounded in a `[[tools]]` entry — so a child cannot
//!   widen its own surface by declaring extra tools at runtime. This has always been the allowlist
//!   and stays it.
//! - The **`init` declaration** is authoritative for what each tool *looks like*: title, group, input
//!   JSON Schema, external-effect flag. Only the running child knows that, because the schema is
//!   generated from the very struct the tool parses.
//!
//! So: iterate the manifest, enrich from the declaration, and drop anything declared that the
//! manifest does not list. A child that declares nothing (every extension built before the handshake
//! could carry it, and lb's own child serve loop) yields exactly the name-only descriptors this host
//! built before — bit-identical, which is the property the back-compat tests pin.
//!
//! Nothing here names an extension (§10): `ext_id` is opaque data used only to unqualify names.

use lb_supervisor::InitReply;

use lb_mcp::ToolDescriptor;

/// Strip a leading `<ext_id>.` from a tool name.
///
/// A native manifest MAY declare its tools already-qualified (the sidecar's own ABI shape) while the
/// registry matches on bare names — the host owns the prefix. Both forms must land on the same key,
/// or a qualified manifest silently fails to match its own declaration.
fn bare<'a>(ext_id: &str, name: &'a str) -> &'a str {
    name.strip_prefix(&format!("{ext_id}.")).unwrap_or(name)
}

/// Convert one wire descriptor into the registry's shape.
fn convert(d: &lb_supervisor::ToolDescriptor, bare_name: &str) -> ToolDescriptor {
    ToolDescriptor {
        // Always the manifest's bare name, never the declared one: the two agree by construction
        // here (we looked the descriptor up by it), and taking it from the manifest side keeps the
        // registry key provably inside the allowlist.
        name: bare_name.to_string(),
        title: d.title.clone(),
        group: d.group.clone(),
        input_schema: d.input_schema.clone(),
        emits_external: d.emits_external,
        result: d.result.clone(),
    }
}

/// Build the registry descriptors for `ext_id` from its manifest `tools` and the child's `declared`
/// `init` reply.
///
/// Falls back to [`ToolDescriptor::name_only`] per manifest entry when the child declared nothing —
/// see the module doc. Declared descriptors for tools absent from the manifest are dropped with a
/// warning: a boot must never fail over a cosmetic over-declaration, and honouring one would let a
/// child register a tool no capability was approved for.
pub fn join_descriptors(
    ext_id: &str,
    tools: &[String],
    declared: Option<&InitReply>,
) -> Vec<ToolDescriptor> {
    let declared = declared.filter(|d| !d.declares_nothing());

    let Some(declared) = declared else {
        return tools
            .iter()
            .map(|t| ToolDescriptor::name_only(bare(ext_id, t)))
            .collect();
    };

    for d in &declared.descriptors {
        let name = bare(ext_id, &d.name);
        if !tools.iter().any(|t| bare(ext_id, t) == name) {
            tracing::warn!(
                ext = %ext_id,
                tool = %d.name,
                "native child declared a descriptor for a tool its manifest does not list; dropped"
            );
        }
    }

    tools
        .iter()
        .map(|t| {
            let name = bare(ext_id, t);
            declared
                .descriptors
                .iter()
                .find(|d| bare(ext_id, &d.name) == name)
                .map(|d| convert(d, name))
                .unwrap_or_else(|| ToolDescriptor::name_only(name))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(tools: &[&str]) -> Vec<String> {
        tools.iter().map(|t| t.to_string()).collect()
    }

    fn declared(descriptors: Vec<lb_supervisor::ToolDescriptor>) -> InitReply {
        InitReply {
            protocol_major: 0,
            tools: descriptors.iter().map(|d| d.name.clone()).collect(),
            descriptors,
        }
    }

    fn schemad(name: &str) -> lb_supervisor::ToolDescriptor {
        lb_supervisor::ToolDescriptor {
            name: name.into(),
            title: "Write point".into(),
            group: "points".into(),
            input_schema: Some(serde_json::json!({"type": "object"})),
            emits_external: true,
            result: None,
        }
    }

    /// The old-child path, and the one that must stay bit-identical: no declaration at all.
    #[test]
    fn no_declaration_falls_back_to_name_only() {
        let out = join_descriptors("nube", &manifest(&["point.read", "point.write"]), None);
        assert_eq!(
            out,
            vec![
                ToolDescriptor::name_only("point.read"),
                ToolDescriptor::name_only("point.write"),
            ]
        );
    }

    /// A child that answered `init` but declared no descriptors is the same case as no reply at all.
    #[test]
    fn an_empty_declaration_falls_back_to_name_only() {
        let empty = declared(vec![]);
        let out = join_descriptors("nube", &manifest(&["point.read"]), Some(&empty));
        assert_eq!(out, vec![ToolDescriptor::name_only("point.read")]);
    }

    #[test]
    fn a_declared_tool_keeps_its_schema_and_flags() {
        let d = declared(vec![schemad("point.write")]);
        let out = join_descriptors("nube", &manifest(&["point.write"]), Some(&d));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "point.write");
        assert_eq!(out[0].title, "Write point");
        assert_eq!(out[0].group, "points");
        assert!(out[0].emits_external);
        assert!(out[0].input_schema.is_some());
    }

    /// A partially-declaring child: the undeclared tool must still register, name-only, rather than
    /// vanishing from the registry (which would break its dispatch).
    #[test]
    fn undeclared_manifest_tools_still_register() {
        let d = declared(vec![schemad("point.write")]);
        let out = join_descriptors("nube", &manifest(&["point.read", "point.write"]), Some(&d));
        assert_eq!(out.len(), 2);
        let read = out.iter().find(|d| d.name == "point.read").unwrap();
        assert!(read.input_schema.is_none());
        assert!(!read.emits_external);
    }

    /// The manifest is the allowlist: a tool the child declares but the manifest does not list has
    /// no approved capability, so it must not reach the registry.
    #[test]
    fn declared_tools_absent_from_the_manifest_are_dropped() {
        let d = declared(vec![schemad("point.write"), schemad("secret.exfil")]);
        let out = join_descriptors("nube", &manifest(&["point.write"]), Some(&d));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "point.write");
        assert!(!out.iter().any(|d| d.name == "secret.exfil"));
    }

    /// A manifest may declare qualified names while the child declares bare ones (or vice versa).
    /// Both must land on the same bare registry key, or the join silently misses every tool.
    #[test]
    fn qualified_and_bare_names_match_each_other() {
        let d = declared(vec![schemad("point.write")]);
        let out = join_descriptors("nube", &manifest(&["nube.point.write"]), Some(&d));
        assert_eq!(out[0].name, "point.write");
        assert!(out[0].input_schema.is_some(), "the join must still match");

        let d = declared(vec![schemad("nube.point.write")]);
        let out = join_descriptors("nube", &manifest(&["point.write"]), Some(&d));
        assert_eq!(out[0].name, "point.write");
        assert!(out[0].input_schema.is_some(), "and in the other direction");
    }
}
