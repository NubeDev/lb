//! What a node says it *is* — the identity trio every embedder fills and both public surfaces read.
//!
//! Three fields, three different owners, because they answer three different questions and each
//! covers the others' failure mode:
//!
//! | field        | answers                       | survives            | lost to             |
//! |--------------|-------------------------------|---------------------|---------------------|
//! | `node`       | "same logical node?"          | restart, reimage*   | a state-dir wipe    |
//! | `machine_id` | "same physical box?"          | a state-dir wipe    | a board swap        |
//! | `name`       | "what do humans call it?"     | whatever an operator sets |               |
//!
//! \* if the embedder persists it — which is the whole point, see below.
//!
//! # Why `node` is the addressable one and `name` is not
//!
//! [`NodeId`] is what a bus key interpolates (`ws/{id}/nodes/{node}`) and what a routed call
//! addresses. It is therefore **immutable at runtime** here: this struct has no setter for it, and
//! nothing downstream may key off `name`. An operator renaming their node must never re-address it
//! mid-flight, and a human-facing label must never become an identifier by accident — that is the
//! single most common way a "display name" quietly becomes load-bearing.
//!
//! `name` is the one field an operator edits. It defaults to the node id so a node that was never
//! named still renders as *something* readable rather than an empty string.
//!
//! # Why `machine_id` exists at all, given `node`
//!
//! A persisted `node` id is unique by construction but dies with the state directory. A
//! machine-derived id survives that wipe but can be **duplicated by disk-image cloning** — the
//! classic silent fleet failure, where every flashed device reports the same value. Neither is
//! sufficient alone; together they let an operator tell "reinstalled" apart from "different box".
//!
//! **This crate does not derive it.** `machine_id` is an opaque `Option<String>` the embedder
//! fills from whatever source fits its platform (rule 10 — no core crate reaches for a
//! product- or OS-specific identity source, and lb must not grow a dependency on one). `None` is
//! entirely normal and simply means the embedder had no such source.
//!
//! # The broadcast wall (lib docs, rule 6)
//!
//! Every field here is published in an mDNS TXT record and served unauthenticated, so this type
//! obeys the same rule `DiscoveredPeer` does: **reachability and identity only** — no workspace, no
//! persona, no capability, no extension list. If a future field cannot be safely shouted onto an
//! untrusted LAN segment, it does not belong in this struct; it belongs behind the bus on the
//! fleet-presence roster.
//!
//! `machine_id` deserves a specific warning, and it is the caller's to heed: a raw OS machine-id is
//! documented by systemd as something that must not be exposed on an untrusted network (it is used
//! as a key-derivation input elsewhere). An embedder publishing one SHOULD pass a hashed or
//! otherwise non-reversible form, not the raw value. This crate cannot enforce that — it never sees
//! where the string came from — so it is stated here and at the [`NodeIdentity::machine_id`] field.

use lb_bus::NodeId;

/// TXT keys for the identity fields, kept short — a TXT record is size-constrained and these are
/// parsed by peers possibly running a different version. `node` itself is already carried by
/// [`crate::peer::TXT_NODE`]; these are the two that join it.
pub(crate) const TXT_MACHINE: &str = "mid";
pub(crate) const TXT_NAME: &str = "name";

/// Who this node is: a stable address, an optional machine-derived id, and a human label.
///
/// Construct with [`NodeIdentity::new`] and refine with the builder setters. The [`node`](Self::node)
/// id is deliberately settable only at construction — see the module docs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeIdentity {
    /// The addressable identity — the SAME `NodeId` fleet-presence announces on the bus and a
    /// routed call targets. Immutable after construction, by design.
    ///
    /// **Stability is the embedder's contract.** `boot*` mints a fresh random id per process, which
    /// is right for a test and wrong for a deployment (a restart would look like a brand-new node
    /// and the roster would grow ghosts). A real node persists one and installs it.
    node: NodeId,

    /// An opaque machine-derived identity, or `None` when the embedder has no source for one.
    ///
    /// Never interpreted here: not parsed, not validated beyond being a string, never used for
    /// addressing. It exists so an operator can answer "is this the same physical box?" across a
    /// state-directory wipe that changes [`node`](Self::node).
    ///
    /// **Publish a non-reversible form.** This value goes onto the LAN in cleartext and is served
    /// unauthenticated; a raw OS machine-id must not be exposed that way (module docs). Hash it
    /// before it reaches this field if that is where it came from.
    pub machine_id: Option<String>,

    /// The operator-editable human label. Defaults to the node id's string form.
    ///
    /// **Never an identifier.** Nothing addresses, routes to, or keys off this — it is display
    /// text, free-form, and two nodes may legitimately share one.
    pub name: String,
}

impl NodeIdentity {
    /// A node identified only by its id, with `name` defaulting to that id and no machine id.
    pub fn new(node: NodeId) -> Self {
        Self {
            name: node.to_string(),
            node,
            machine_id: None,
        }
    }

    /// The addressable node id. No setter — see the module docs for why renaming must not re-address.
    pub fn node(&self) -> &NodeId {
        &self.node
    }

    /// Attach the machine-derived id. Pass a non-reversible form (field docs).
    pub fn with_machine_id(mut self, id: impl Into<String>) -> Self {
        self.machine_id = Some(id.into());
        self
    }

    /// Set the human label. An empty or whitespace-only name is IGNORED rather than stored: a blank
    /// label is never what an operator meant, and letting it through would render as an unnamed
    /// node in every UI that shows it. The previous value (by default the node id) survives.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        let name = name.into();
        if !name.trim().is_empty() {
            self.name = name;
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node() -> NodeId {
        NodeId::new("node:gw-01").expect("valid id")
    }

    #[test]
    fn name_defaults_to_the_node_id() {
        // A node that was never named still renders as something readable.
        assert_eq!(NodeIdentity::new(node()).name, "node:gw-01");
    }

    #[test]
    fn a_blank_name_does_not_erase_the_default() {
        let id = NodeIdentity::new(node()).with_name("   ");
        assert_eq!(
            id.name, "node:gw-01",
            "whitespace must not become the label"
        );
    }

    #[test]
    fn machine_id_is_absent_unless_supplied() {
        assert_eq!(NodeIdentity::new(node()).machine_id, None);
        let id = NodeIdentity::new(node()).with_machine_id("abc123");
        assert_eq!(id.machine_id.as_deref(), Some("abc123"));
    }

    #[test]
    fn renaming_leaves_the_addressable_id_untouched() {
        // The invariant the whole type exists to protect: a label change must never re-address.
        let id = NodeIdentity::new(node()).with_name("front office");
        assert_eq!(id.node().as_str(), "node:gw-01");
        assert_eq!(id.name, "front office");
    }
}
