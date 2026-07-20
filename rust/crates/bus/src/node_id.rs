//! `NodeId` — a node's stable, key-expression-safe identity on the bus.
//!
//! **Owned by fleet-presence** (`docs/scope/node-roles/fleet-presence-scope.md`), minted here by
//! routed-node-dispatch (#81) because addressing a call to a node requires an identity to address
//! it *by*, and nothing in the platform minted one. Placing it in `lb-bus` — below both `lb-mcp`
//! and `lb-host` — means fleet-presence widens THIS type (adding persona/role/version around it)
//! rather than introducing a second one. The two scopes must not fork node identity; see the
//! 2026-07-20 update in fleet-presence-scope.md.
//!
//! **Why the charset is enforced at construction.** A node id rides a Zenoh key expression as a
//! segment: `mcp/{ext}/{node}/call` (#81) and `ws/{id}/nodes/{node}` (fleet-presence). Zenoh
//! treats `/` as the segment separator and `* $ ? #` as pattern/verbatim syntax, so an id
//! containing any of them would silently change the key's SHAPE — a `/` would split one segment
//! into two, and a `*` would turn a specific address into a WILDCARD that matches other nodes.
//! That last one is a security-relevant failure, not a cosmetic one: an id like `gw-*` would
//! address the whole fleet while reading as a single box.
//!
//! The alternative — percent-encoding ids at the key boundary — was **rejected**: an encode/decode
//! pair is exactly the drift hazard `route.rs` exists to prevent (the caller's key and the serving
//! node's declaration must agree character-for-character, and two call sites encoding differently
//! is a bug that only appears cross-node). Making the *type* unable to hold an unsafe character
//! means raw interpolation is always correct and there is nothing to keep in sync.
//!
//! `:` is deliberately ALLOWED — it is not structural in a Zenoh key expression, and it keeps the
//! platform's readable `node:gw-01` convention (matching `user:ada`, `job:{id}` elsewhere).

use std::fmt;

use serde::{Deserialize, Serialize};

/// The characters that would change a key expression's meaning if they appeared in an id.
/// `/` splits segments; `*` and `$` are Zenoh wildcard/verbatim syntax; `?` and `#` are reserved.
const KEY_UNSAFE: &[char] = &['/', '*', '$', '?', '#'];

/// A node's stable identity — validated key-expression-safe at construction, so it can be
/// interpolated into a bus key without encoding or escaping.
///
/// Stability is the CALLER's contract (it must come from durable config, not a per-boot random —
/// fleet-presence's "NodeId stability" risk); this type guarantees only that whatever id is
/// chosen cannot corrupt a key expression.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct NodeId(String);

/// Why a candidate node id was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NodeIdError {
    #[error("node id must not be empty")]
    Empty,
    #[error("node id {0:?} contains {1:?}, which is not safe in a bus key expression")]
    KeyUnsafe(String, char),
}

impl NodeId {
    /// Build a `NodeId`, rejecting anything that would change the shape of a key expression.
    pub fn new(id: impl Into<String>) -> Result<Self, NodeIdError> {
        let id = id.into();
        if id.is_empty() {
            return Err(NodeIdError::Empty);
        }
        if let Some(bad) = id.chars().find(|c| KEY_UNSAFE.contains(c)) {
            return Err(NodeIdError::KeyUnsafe(id, bad));
        }
        Ok(Self(id))
    }

    /// The id as a `&str`, ready to interpolate into a key segment.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for NodeId {
    type Error = NodeIdError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

impl From<NodeId> for String {
    fn from(n: NodeId) -> String {
        n.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_platform_id_convention() {
        // `:` is not structural in a Zenoh key expression, so the readable `node:gw-01` shape
        // (matching `user:ada`, `job:{id}`) survives without encoding.
        let id = NodeId::new("node:gw-01").expect("colon ids are safe");
        assert_eq!(id.as_str(), "node:gw-01");
    }

    #[test]
    fn rejects_every_key_structural_character() {
        for bad in ['/', '*', '$', '?', '#'] {
            let candidate = format!("gw{bad}01");
            assert!(
                NodeId::new(candidate.clone()).is_err(),
                "{candidate:?} must be refused — it would change the key's shape"
            );
        }
    }

    #[test]
    fn rejects_empty() {
        assert_eq!(NodeId::new(""), Err(NodeIdError::Empty));
    }

    /// The security-relevant case, called out explicitly: a wildcard id would read as ONE box but
    /// address the whole fleet. This is why validation lives in the constructor, not at the key
    /// boundary where a caller could skip it.
    #[test]
    fn a_wildcard_id_cannot_be_constructed() {
        assert!(
            NodeId::new("gw-*").is_err(),
            "a wildcard id would silently address every matching node"
        );
    }

    #[test]
    fn round_trips_through_serde_and_rejects_unsafe_on_the_way_in() {
        let id = NodeId::new("node:gw-01").unwrap();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, r#""node:gw-01""#);
        assert_eq!(serde_json::from_str::<NodeId>(&json).unwrap(), id);
        // Deserialization goes through the same validation — an id arriving over the wire
        // cannot smuggle in a wildcard.
        assert!(serde_json::from_str::<NodeId>(r#""gw/evil""#).is_err());
    }
}
