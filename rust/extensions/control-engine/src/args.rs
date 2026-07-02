//! The shared `control-engine.*` argument envelope + the CE identity forms the wire
//! uses (control-engine scope: `{ appliance, ...verb args }`; `NodeRef`/`NodeKey`
//! uid-keyed, never a bare integer).
//!
//! `rubix-ce`'s `NodeRef`/`NodeKey` deliberately do NOT derive serde (they are
//! in-process identity types, not wire types — `ce-client-rust` `identity.rs`). So
//! this file owns the wire (de)serialization form and maps to/from the client
//! types. The uid-keyed form is `{ "uid": <u32>, "kind": "component"|"property"|
//! "edge", "path"?: <string> }`; the root is `{ "root": true }` (or absent).

use rubix_ce::{EngineInstanceId, NodeKey, NodeRef, UidKind};
use serde::Deserialize;

/// The base envelope carried by every `control-engine.*` call. For S3 the
/// `appliance` field carries the CE base as `host:port` (or a bare port, or empty →
/// the canonical local `127.0.0.1:7979`); S4 resolves it against the appliance
/// registry instead. Kept small on purpose — verb-specific args are read off the
/// same `serde_json::Value` by each verb.
#[derive(Debug, Clone, Deserialize)]
pub struct Envelope {
    /// The appliance selector. In S3 this is the CE base `host:port` (local mode);
    /// S4 turns it into a registry id resolved to a node + base.
    #[serde(default)]
    pub appliance: String,
}

/// The canonical CE REST/WS port (control-engine scope open-question resolution:
/// ce-studio's `7979`, aligning the older `7878` mentions).
pub const CANONICAL_PORT: u16 = 7979;

/// Parse an `appliance` selector into `(host, port)`. Accepts `host:port`, a bare
/// `port`, or empty (→ `127.0.0.1:CANONICAL_PORT`). An optional `http://`/`ws://`
/// scheme prefix is tolerated and stripped.
#[must_use]
pub fn base_of(appliance: &str) -> (String, u16) {
    let s = appliance.trim();
    let s = s
        .strip_prefix("http://")
        .or_else(|| s.strip_prefix("ws://"))
        .unwrap_or(s);
    let s = s.trim_end_matches('/');
    if s.is_empty() {
        return ("127.0.0.1".to_string(), CANONICAL_PORT);
    }
    match s.rsplit_once(':') {
        Some((host, port)) => (host.to_string(), port.parse().unwrap_or(CANONICAL_PORT)),
        None => match s.parse::<u16>() {
            Ok(port) => ("127.0.0.1".to_string(), port),
            Err(_) => (s.to_string(), CANONICAL_PORT),
        },
    }
}

/// The uid-keyed wire form of a CE node reference (`ce-client-rust` `identity.rs`:
/// a UID is never a bare integer — it carries its pool `kind` and instance).
#[derive(Debug, Clone, Deserialize)]
pub struct NodeRefArg {
    /// True → the engine root (`NodeRef::Root`). When set, `uid`/`kind` are ignored.
    #[serde(default)]
    pub root: bool,
    /// The per-instance, per-pool UID (only when not `root`).
    #[serde(default)]
    pub uid: Option<u32>,
    /// Which pool the UID was drawn from: `component` (default) / `property` / `edge`.
    #[serde(default)]
    pub kind: Option<String>,
    /// A snapshotted path/name (optional; survives an engine restart re-numbering).
    #[serde(default)]
    pub path: Option<String>,
}

impl NodeRefArg {
    /// Map to `rubix-ce`'s `NodeRef`, keying the UID against `instance`. An absent
    /// `uid` (and non-`root`) falls back to `NodeRef::Root` (a safe, whole-tree read).
    #[must_use]
    pub fn to_node_ref(&self, instance: &EngineInstanceId) -> NodeRef {
        if self.root {
            return NodeRef::Root;
        }
        match self.uid {
            Some(uid) => {
                let mut key = NodeKey::new(instance.clone(), self.uid_kind(), uid);
                if let Some(p) = &self.path {
                    key = key.with_path(p.clone());
                }
                NodeRef::Parent(key)
            }
            None => NodeRef::Root,
        }
    }

    fn uid_kind(&self) -> UidKind {
        match self.kind.as_deref() {
            Some("property") => UidKind::Property,
            Some("edge") => UidKind::Edge,
            _ => UidKind::Component,
        }
    }
}

impl Default for NodeRefArg {
    fn default() -> Self {
        Self {
            root: true,
            uid: None,
            kind: None,
            path: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_defaults_to_canonical_local() {
        assert_eq!(base_of(""), ("127.0.0.1".to_string(), CANONICAL_PORT));
        assert_eq!(base_of("  "), ("127.0.0.1".to_string(), CANONICAL_PORT));
    }

    #[test]
    fn base_parses_host_port_bare_port_and_scheme() {
        assert_eq!(base_of("10.0.0.2:8080"), ("10.0.0.2".to_string(), 8080));
        assert_eq!(base_of("7979"), ("127.0.0.1".to_string(), 7979));
        assert_eq!(
            base_of("http://127.0.0.1:7979/"),
            ("127.0.0.1".to_string(), 7979)
        );
    }

    #[test]
    fn noderef_root_and_keyed() {
        let inst = EngineInstanceId::edge();
        let root: NodeRefArg = serde_json::from_str("{}").unwrap();
        assert!(matches!(root.to_node_ref(&inst), NodeRef::Root));
        let keyed: NodeRefArg =
            serde_json::from_str(r#"{"root":false,"uid":5,"kind":"component"}"#).unwrap();
        match keyed.to_node_ref(&inst) {
            NodeRef::Parent(k) => assert_eq!(k.uid.0, 5),
            NodeRef::Root => panic!("expected keyed parent"),
        }
    }
}
