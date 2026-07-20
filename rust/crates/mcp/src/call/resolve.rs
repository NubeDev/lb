//! The resolve phase — map a qualified tool name `<ext>.<tool>` (plus an optional target node) to
//! ONE dispatch target. Only reached after `authorize` passed, so a `NotFound` or `Ambiguous` here
//! is never seen by an unauthorized caller (mcp scope; routed-node-dispatch #81).
//!
//! **This is where the ambiguity guard lives, and it can only live here** (scope, Risks): the
//! candidate set — which nodes host this extension — is known only on the CALLING side. A serving
//! node knows about itself and nothing else, so a serve-side guard would have to coordinate across
//! nodes to notice a duplicate, and would silently never fire. Getting this backwards produces a
//! guard that compiles, passes its tests, and protects nothing.

use lb_bus::NodeId;

use crate::registry::{Registry, Target};

use super::error::ToolError;

/// Split `<ext>.<tool>` and find the ONE target that should run it.
///
/// - `target_node = None` (the overwhelmingly common case): resolves to the single host if there
///   is exactly one, and refuses with [`ToolError::Ambiguous`] if there are several. It does NOT
///   auto-pick — picking one for the caller is the bug this scope removes (scope, open question 5).
/// - `target_node = Some(n)`: resolves to `n`'s target, or [`ToolError::NodeUnreachable`] if `n`
///   is not among the hosts this node can see. Never falls back to another host — the fallback IS
///   the misprovisioning bug.
///
/// **A local host wins an untargeted call outright**, even when remote hosts also exist: running
/// on this node is unambiguous (it is *here*), needs no bus hop, and is what an untargeted call
/// already did before #81. Refusing it as "ambiguous" would break every existing single-node
/// caller the moment a fleet peer appeared, which is a behaviour change this scope explicitly
/// does not want ("Unaddressed calls keep working, unchanged").
pub fn resolve(
    registry: &Registry,
    qualified_tool: &str,
    target_node: Option<&NodeId>,
) -> Result<Target, ToolError> {
    let (ext_id, tool) = qualified_tool.split_once('.').ok_or(ToolError::NotFound)?;

    // Only targets that actually declare `<tool>` are candidates. This is what makes descriptors a
    // per-node fact (scope, open question 4): a fleet mid-rolling-upgrade where gw-01 has a tool
    // and gw-02 does not is NORMAL, and resolves unambiguously to gw-01 rather than erroring.
    let candidates: Vec<Target> = registry
        .targets(ext_id)
        .into_iter()
        .filter(|t| t.tools().iter().any(|t_name| t_name == tool))
        .collect();

    if candidates.is_empty() {
        return Err(ToolError::NotFound);
    }

    match target_node {
        // An explicitly targeted call: find that exact node, or refuse. A target that names THIS
        // node resolves to the local instance with no bus hop (self-targeting).
        Some(node) => candidates
            .into_iter()
            .find(|t| t.node() == Some(node))
            .ok_or_else(|| ToolError::NodeUnreachable {
                node: node.to_string(),
            }),

        None => {
            // A local host is unambiguous and wins — see the doc comment above.
            if let Some(local) = candidates.iter().find(|t| t.node().is_none()) {
                return Ok(local.clone());
            }
            match candidates.len() {
                // The unchanged fast path: exactly one host, no target needed, resolves as it
                // always did.
                1 => Ok(candidates.into_iter().next().expect("len checked")),
                // THE GUARD. Several nodes host this tool and the caller did not say which — the
                // pre-#81 code silently coin-flipped here. Refuse, and name the candidates so the
                // caller can retry with a target instead of parsing prose.
                _ => Err(ToolError::Ambiguous {
                    ext: ext_id.to_string(),
                    // Sorted: a HashMap-ordered candidate list would make the error message vary
                    // between runs and any test asserting on it flaky.
                    candidates: {
                        let mut names: Vec<String> = candidates
                            .iter()
                            .filter_map(|t| t.node().map(|n| n.to_string()))
                            .collect();
                        names.sort();
                        names
                    },
                }),
            }
        }
    }
}
