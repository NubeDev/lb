//! The gateway cache's **entity fold** (entity-scoped-data scope). The capability fingerprint
//! (`fingerprint.rs`) answers "which gated targets may this caller call"; two callers with the same
//! caps can still read DIFFERENT rows once a datasource's row policy is enforced. So the cache key
//! also folds each federation target's resolved entity set — otherwise a group member could be served
//! a frame an admin (or another group) computed. Unrestricted callers fold nothing, so their key, and
//! every entry already warm, is unchanged.

use lb_auth::Principal;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::boot::Node;
use crate::federation::row_scope_for;
use crate::viz::panel_federation_sources;

/// A key suffix for `principal`'s entity scope over `panel`'s federation sources; `None` when every
/// one of them reads unrestricted for this caller. A policy read failure folds a unique marker, so a
/// broken policy never shares an entry with anyone.
pub(super) async fn entity_suffix(
    node: &Node,
    principal: &Principal,
    ws: &str,
    panel: &Value,
) -> Option<String> {
    let mut sources = panel_federation_sources(panel);
    sources.sort();
    sources.dedup();
    let mut h = Sha256::new();
    h.update(b"viz-entity-fp\x1f");
    let mut restricted = false;
    for source in &sources {
        let part = match row_scope_for(node, principal, ws, source).await {
            Ok(None) => continue,
            Ok(Some(scope)) => scope.get("ids").map(Value::to_string).unwrap_or_default(),
            Err(_) => format!("error:{}", principal.owner_sub()),
        };
        restricted = true;
        h.update(source.as_bytes());
        h.update(b"\x1f");
        h.update(part.as_bytes());
        h.update(b"\x1f");
    }
    restricted.then(|| format!("{:x}", h.finalize()))
}
