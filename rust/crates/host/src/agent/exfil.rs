//! The node's **exfiltration-taint inventory** (agent-loop-hardening slice E): every tool whose
//! descriptor self-declares `emits_external: true` — host-native descriptors and extension
//! manifests alike, each id treated as opaque data (rule 10: no tool-name list lives in core; a
//! tool is tainted because *it says so*, exactly like every other descriptor field).
//!
//! Taint is a property of the TOOL, not the caller, so no authorization runs here — the set is
//! only ever used to *narrow* a guarded run (menu exclusion + dispatch deny), never to widen.
//! Trust model (stated in the scope): a tool that lies is not caught; the guard is
//! defense-in-depth over the capability wall, not a replacement.

use std::collections::HashSet;

use crate::boot::Node;
use crate::tools::host_descriptors;

/// The deny fed to the model when a guarded run proposes a tainted tool anyway (it can hallucinate
/// a tool it was never shown — gate at definition time AND call time).
pub const EXFIL_DENIED: &str = "denied: this workspace's exfiltration guard is on and this tool \
    can transmit data off the node";

/// Every reachable tool (qualified name) declaring `emits_external: true` on this node.
pub(crate) fn tainted_tools(node: &Node) -> HashSet<String> {
    let mut out = HashSet::new();
    // Host-native descriptors carry qualified names already.
    for d in host_descriptors() {
        if d.emits_external {
            out.insert(d.name);
        }
    }
    // Extension descriptors are bare names; qualify with the (opaque) extension id.
    for (ext_id, descriptors) in node.registry.descriptor_entries() {
        for d in descriptors {
            if d.emits_external {
                out.insert(format!("{ext_id}.{}", d.name));
            }
        }
    }
    out
}
