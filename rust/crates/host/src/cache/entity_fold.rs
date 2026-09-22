//! The gateway cache's **entity fold** (entity-scoped-data scope). The capability fingerprint
//! (`fingerprint.rs`) answers "which gated targets may this caller call"; two callers with the same
//! caps can still read DIFFERENT rows once a datasource's row policy is enforced. So the cache key
//! also folds the caller's resolved entity set — otherwise a group member could be served a frame an
//! admin (or another group) computed. Unrestricted callers fold nothing, so their key, and every
//! entry already warm, is unchanged.

use lb_auth::Principal;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::boot::Node;
use crate::federation::{enforced_policies, row_scope_for};

/// A key suffix for `principal`'s entity scope over EVERY enforced datasource in `ws`; `None` when
/// the caller reads all of them unrestricted (or none is enforced). Every enforced source is folded,
/// not just the ones the panel names directly: a target can reach a source indirectly (a saved
/// `query.run`), and folding one scope too many only splits a cache entry, never shares one. A read
/// failure folds a unique marker, so a broken policy never shares an entry with anyone.
pub(super) async fn entity_suffix(
    node: &Node,
    principal: &Principal,
    ws: &str,
    _panel: &Value,
) -> Option<String> {
    let policies = match enforced_policies(&node.store, ws).await {
        Ok(p) => p,
        Err(_) => return Some(format!("error:{}", principal.owner_sub())),
    };
    let mut h = Sha256::new();
    h.update(b"viz-entity-fp\x1f");
    let mut restricted = false;
    for policy in &policies {
        let part = match row_scope_for(node, principal, ws, &policy.source).await {
            Ok(None) => continue,
            Ok(Some(scope)) => scope.get("ids").map(Value::to_string).unwrap_or_default(),
            Err(_) => format!("error:{}", principal.owner_sub()),
        };
        restricted = true;
        h.update(policy.source.as_bytes());
        h.update(b"\x1f");
        h.update(part.as_bytes());
        h.update(b"\x1f");
    }
    restricted.then(|| format!("{:x}", h.finalize()))
}
