//! The `init` handshake **payload** — what a child reports about itself at spawn.
//!
//! `rpc.rs` owns the envelope (`Request`/`Reply`/`Method`); this owns the body of the one reply that
//! carries more than "ok": the child's protocol major, the tools it serves, and — additively — a
//! self-declared [`ToolDescriptor`] per tool (title, group, input JSON Schema, external-effect flag).
//!
//! These types mirror `lb-ext-native`'s (the published SDK a native extension links against), the
//! same way [`crate::rpc::CallParams`]/[`crate::rpc::Caller`] are mirrored there. lb does not depend
//! on the SDK — the wire is the contract, and this crate is where lb writes its half of it.
//!
//! ## Everything here is optional, deliberately
//!
//! The handshake predates this payload: children built against an older SDK reply with whatever
//! shape they like (lb's own [`crate::serve`] loop answers `{"ready":true,"ext":"…"}`, and did so
//! for every native extension shipped to date). So **parsing is best-effort and fail-open**:
//! [`InitReply::parse`] returns `None` on anything it cannot read, every field carries
//! `#[serde(default)]`, and a caller that learns nothing falls back to the manifest exactly as it
//! did before. A child cannot break its own boot by mis-declaring — the worst case is the behaviour
//! that shipped.
//!
//! `tools` is **not** the dispatch allowlist here — lb takes that from the manifest, which is also
//! the capability source. What arrives on this line is enrichment, joined onto the manifest by name.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The child's `init` reply body. Every field is optional; see the module doc.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct InitReply {
    /// The wire protocol major the child was built against. Absent on a child that never declared
    /// one, which is why it defaults rather than failing the parse.
    #[serde(default)]
    pub protocol_major: u64,
    /// The tools the child says it serves. Advisory on the lb side — the manifest is authoritative.
    #[serde(default)]
    pub tools: Vec<String>,
    /// Per-tool self-declared contracts. Empty on every child built before the SDK could carry them.
    #[serde(default)]
    pub descriptors: Vec<ToolDescriptor>,
}

impl InitReply {
    /// Read an `init` reply body, or `None` if it is not one.
    ///
    /// Fail-open by construction: a child that answers `"ok"`, `{"ready":true}`, or anything else
    /// unparseable yields `None` (or an empty reply), never an error that would fail the spawn. The
    /// handshake's liveness check is the `Reply` envelope; this is only about what we can *learn*.
    pub fn parse(result: &str) -> Option<Self> {
        serde_json::from_str(result).ok()
    }

    /// True when the child declared nothing this host could not have synthesised itself — the signal
    /// to fall back to the manifest-derived name-only descriptors (bit-identical prior behaviour).
    pub fn declares_nothing(&self) -> bool {
        self.descriptors.is_empty()
    }
}

/// One tool's self-declared contract, as it arrives on the `init` line.
///
/// A deliberate mirror of `lb_mcp::ToolDescriptor` rather than that type itself: this crate is the
/// OS/wire layer and holds no registry, no store, and no authorization (see the crate doc). The host
/// converts one into the other at the single point where the two layers meet (`native/descriptors`).
///
/// **Self-declared, never enforced.** lb does not validate a call's input against `input_schema`
/// before dispatch — the child's own parse stays the authority, the same trust model as
/// `emits_external`. These schemas are a UI affordance for `tools.catalog`, not a security boundary.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ToolDescriptor {
    /// The tool name. Bare (`point.write`); the host owns the `<ext>.` qualification.
    pub name: String,
    /// Human label for a picker row; empty means "fall back to the name".
    #[serde(default)]
    pub title: String,
    /// Grouping key for a picker's section headers; empty means ungrouped.
    #[serde(default)]
    pub group: String,
    /// JSON Schema for the call's input, or `None` for "unknown shape, render free text".
    #[serde(default)]
    pub input_schema: Option<Value>,
    /// True when running this tool transmits an effect off the node. Drives the undo classifier's
    /// irreversible class.
    #[serde(default)]
    pub emits_external: bool,
    /// JSON Schema for the tool's output, or `None`. Advisory only.
    #[serde(default)]
    pub result: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shape a descriptor-declaring child sends.
    #[test]
    fn parses_a_declaring_child() {
        let body = r#"{
            "protocol_major": 0,
            "tools": ["point.read", "point.write"],
            "descriptors": [
                { "name": "point.write", "title": "Write point", "group": "points",
                  "input_schema": { "type": "object" }, "emits_external": true }
            ]
        }"#;
        let init = InitReply::parse(body).expect("parses");
        assert_eq!(init.tools.len(), 2);
        assert_eq!(init.descriptors.len(), 1);
        assert_eq!(init.descriptors[0].name, "point.write");
        assert_eq!(init.descriptors[0].group, "points");
        assert!(init.descriptors[0].emits_external);
        assert!(init.descriptors[0].input_schema.is_some());
        assert!(!init.declares_nothing());
    }

    /// A child on an older SDK sends names only. It must parse, and it must declare nothing — the
    /// signal that the host keeps its manifest-derived behaviour.
    #[test]
    fn an_old_frame_declares_nothing() {
        let init = InitReply::parse(r#"{"protocol_major":0,"tools":["echo"]}"#).expect("parses");
        assert_eq!(init.tools, vec!["echo".to_string()]);
        assert!(init.declares_nothing());
    }

    /// lb's own child serve loop answers `{"ready":true,"ext":"…"}` — the shape every native
    /// extension shipped to date replies with. It must not be an error, and it must declare nothing.
    #[test]
    fn lbs_own_ready_frame_is_tolerated() {
        let init =
            InitReply::parse(r#"{"ready":true,"ext":"echo"}"#).expect("unknown keys ignored");
        assert!(init.tools.is_empty());
        assert!(init.declares_nothing());
    }

    /// A child that replies with a bare string, or garbage, yields `None` rather than failing a
    /// spawn. Boot must never hinge on what a child *said*, only on whether it answered.
    #[test]
    fn an_unreadable_body_is_none_not_an_error() {
        assert!(InitReply::parse("\"ok\"").is_none());
        assert!(InitReply::parse("not json at all").is_none());
        assert!(InitReply::parse("").is_none());
    }

    /// A descriptor missing everything but its name is valid — absence is the encoding for
    /// "not declared", and a partial declaration must not poison the rest of the list.
    #[test]
    fn a_bare_descriptor_parses_with_absent_fields() {
        let init = InitReply::parse(r#"{"descriptors":[{"name":"echo"}]}"#).expect("parses");
        let d = &init.descriptors[0];
        assert_eq!(d.name, "echo");
        assert!(d.title.is_empty());
        assert!(d.input_schema.is_none());
        assert!(!d.emits_external);
    }
}
