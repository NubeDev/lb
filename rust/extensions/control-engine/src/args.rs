//! The shared `control-engine.*` argument envelope + the CE identity forms the wire
//! uses (control-engine scope: `{ appliance, ...verb args }`; `NodeRef`/`NodeKey`
//! uid-keyed, never a bare integer).
//!
//! `rubix-ce`'s `NodeRef`/`NodeKey` deliberately do NOT derive serde (they are
//! in-process identity types, not wire types — `ce-client-rust` `identity.rs`). So
//! this file owns the wire (de)serialization form and maps to/from the client
//! types. The uid-keyed form is `{ "uid": <u32>, "kind": "component"|"property"|
//! "edge", "path"?: <string> }`; the root is `{ "root": true }` (or absent).

use rubix_ce::{EngineInstanceId, FlexValue, NodeKey, NodeRef, UidKind};
use serde::Deserialize;
use serde_json::Value;

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

/// The uid-keyed wire form of a CE node identity for a WRITE verb. Unlike
/// [`NodeRefArg`] (which defaults to the root for a whole-tree read), a write
/// verb MUST address a concrete node: an absent/empty `uid` is an error, never a
/// silent root fallback (a write to "the root" is not a thing S5 exposes).
///
/// Reuses the same wire shape (`{ "uid": <u32>, "kind": ..., "path"?: ... }`) as
/// [`NodeRefArg`] so a caller keys nodes identically on read and write.
#[derive(Debug, Clone, Deserialize)]
pub struct NodeKeyArg {
    /// The per-instance, per-pool UID (REQUIRED for a write).
    pub uid: u32,
    /// Which pool the UID was drawn from: `component` (default) / `property` / `edge`.
    #[serde(default)]
    pub kind: Option<String>,
    /// A snapshotted path/name (optional; survives an engine restart re-numbering).
    #[serde(default)]
    pub path: Option<String>,
}

impl NodeKeyArg {
    /// Map to `rubix-ce`'s [`NodeKey`], keying the UID against `instance`.
    #[must_use]
    pub fn to_node_key(&self, instance: &EngineInstanceId) -> NodeKey {
        let mut key = NodeKey::new(instance.clone(), self.uid_kind(), self.uid);
        if let Some(p) = &self.path {
            key = key.with_path(p.clone());
        }
        key
    }

    fn uid_kind(&self) -> UidKind {
        match self.kind.as_deref() {
            Some("property") => UidKind::Property,
            Some("edge") => UidKind::Edge,
            _ => UidKind::Component,
        }
    }
}

/// Parse a JSON value into a [`FlexValue`]. `FlexValue` is `#[serde(untagged)]`
/// (null/bool/int/float/string), so a JSON scalar deserializes straight into it;
/// a non-scalar (object/array) is a `bad value` error (CE values are scalar).
pub fn flex_value(v: &Value) -> Result<FlexValue, String> {
    if v.is_object() || v.is_array() {
        return Err(format!("bad value: expected a scalar, got {v}"));
    }
    serde_json::from_value(v.clone()).map_err(|e| format!("bad value: {e}"))
}

/// Parse a name-keyed JSON object of scalar values into a `Vec<(String, FlexValue)>`
/// (the shape both [`rubix_ce::PropPatch`] batches and action [`rubix_ce::Params`]
/// use). Order follows the object's insertion order (serde_json preserves it).
pub fn value_pairs(v: &Value) -> Result<Vec<(String, FlexValue)>, String> {
    let obj = v
        .as_object()
        .ok_or_else(|| format!("expected an object of name→value, got {v}"))?;
    obj.iter()
        .map(|(k, val)| Ok((k.clone(), flex_value(val)?)))
        .collect()
}

/// Parse the required uid-keyed `node` field of a write verb's input into a
/// [`NodeKey`]. A missing or malformed `node` (e.g. no `uid`) is a `bad node arg`
/// error — a write with no target is invalid, never a root fallback.
pub fn require_node_key(input: &Value, instance: &EngineInstanceId) -> Result<NodeKey, String> {
    let v = input
        .get("node")
        .ok_or_else(|| "bad node arg: missing `node`".to_string())?;
    let arg: NodeKeyArg =
        serde_json::from_value(v.clone()).map_err(|e| format!("bad node arg: {e}"))?;
    Ok(arg.to_node_key(instance))
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
