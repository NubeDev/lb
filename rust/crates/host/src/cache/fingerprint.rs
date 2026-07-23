//! The **capability fingerprint** for the `viz.query` gateway cache (dashboard-query-acceleration
//! scope, slice 2) — the single load-bearing piece of the subject-scoped key. It answers exactly one
//! question, stably: *which of the caps that gate this panel's targets does THIS caller hold?* Two
//! callers with the same answer share a warm entry (the win); a caller who would get a DIFFERENT
//! (denied) frame on any target produces a different fingerprint → a different key → their own resolve
//! (the wall). This is the one place a bug is a security bug — hence its own reviewed file and the
//! dedicated cross-grant deny test (mutation-checked).
//!
//! **Fold exactly the leak boundary, nothing more:**
//!   - It hashes the sorted set of **target caps the caller HOLDS** among the panel's dispatched
//!     tools — computed with the SAME `gate_tool_for` + `authorize_tool` decision the resolver makes
//!     per target, so the fingerprint's allow/deny vector is provably the resolver's.
//!   - It folds **no identity, no token, no ws** into the hash (the ws is already in the base key). A
//!     narrower fold (miss a target cap) would leak; a wider one (identity/token) would make every
//!     caller miss. The set of held target caps is the exact, minimal boundary.
//!
//! A panel with no dispatched targets (inline-frames, or all-hidden) yields the empty fingerprint —
//! correct, because such a panel resolves identically for every caller (it reaches no gated read).

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::tool_call::gate_tool_for;
use crate::viz::panel_target_tools;

/// The caller's capability fingerprint for `panel` in `ws`: a stable hash of the sorted set of the
/// panel's target caps this caller is granted. Deterministic and caller-identity-free.
pub fn capability_fingerprint(principal: &Principal, ws: &str, panel: &Value) -> String {
    // The caps the caller HOLDS among the panel's dispatched targets — the exact allow/deny vector the
    // resolver produces. `gate_tool_for` maps each target verb to the cap that actually gates it (so an
    // aliased verb — e.g. `federation.schema` riding `federation.query` — folds under the right cap),
    // then `authorize_tool` runs the SAME workspace-first + `mcp:<cap>:call` check the dispatcher runs.
    let mut held: Vec<String> = panel_target_tools(panel)
        .into_iter()
        .map(|tool| gate_tool_for(&tool).to_string())
        .filter(|cap| authorize_tool(principal, ws, cap).is_ok())
        .collect();
    held.sort();
    held.dedup();

    // Domain-tagged hash over the sorted held caps. The `\x1f` unit separators keep `["a", "bc"]`
    // distinct from `["ab", "c"]` (a boundary-collision a bare concat would allow).
    let mut h = Sha256::new();
    h.update(b"viz-cap-fp\x1f");
    for cap in &held {
        h.update(cap.as_bytes());
        h.update(b"\x1f");
    }
    format!("{:x}", h.finalize())
}
