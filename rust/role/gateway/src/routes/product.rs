//! The `product` object both `GET /node` and `GET /health` publish (embedder-build-info scope).
//!
//! One shape, one source, two routes — deliberately shared rather than declared twice, because the
//! whole point of the field is that the surfaces answering "what is this node" cannot disagree
//! about it. Both routes read the same [`Gateway::build_info`](crate::state::Gateway::build_info)
//! cell, which the boot seam filled once from `BootConfig::build_info`.
//!
//! ```text
//! GET /node   →  {"node":"node:…", "version":"0.1.0",        // ← lb core, UNCHANGED
//!                 "product":{"name":"rubix-ai","version":"0.1.1+g1a2b3c4d5e6f"},  // ← NEW
//!                 "gateway":{…}}
//! GET /health →  {"status":"ok","version":"0.1.0","detail":{…},
//!                 "product":{"name":"rubix-ai","version":"0.1.1+g1a2b3c4d5e6f"}}
//! ```
//!
//! **Additive, never a rename.** `version` keeps meaning *lb's gateway build* on both routes,
//! forever. The tidier-looking alternative — make `version` the product and move lb's to
//! `lb_version` — breaks every existing reader (including this crate's own route tests, which
//! assert `body["version"] == env!("CARGO_PKG_VERSION")`) and makes the *always-present* core the
//! special case. The optional value should be the optional field.
//!
//! **Omitted, never null.** With no embedder the key is absent and both bodies are byte-identical
//! to what they were before this existed; a `"product":null` would force every consumer into a
//! two-case read for no gain. A consumer that must distinguish "not an embedder" from "an older lb"
//! reads `version`, which is always present.
//!
//! Both fields are opaque strings lb never derives, parses, or validates (rule 10), published
//! outside the auth wall — see `lb_discovery::BuildInfo` for the full argument and the trade.

use serde::Serialize;

/// The `product` object: the identity of the program that embedded this node.
///
/// Owns its two strings rather than borrowing the gateway's cell: axum clones `Gateway` into each
/// request, so the `Arc` the cell lives behind is dropped when the handler returns and a borrow
/// could not outlive it. Two short `String`s per probe, only on a node that has an embedder at all.
#[derive(Debug, Serialize)]
pub struct ProductBody {
    /// The embedder's product name. Display text; nothing routes or authorizes by it.
    pub name: String,
    /// The embedder's build version, free-form (semver build metadata is the expected shape).
    pub version: String,
}

impl ProductBody {
    /// Render the gateway's build-info cell, or `None` when no embedder stated one (⇒ the field is
    /// skipped). The single conversion both routes call, so neither can render it differently.
    pub fn from_build_info(info: Option<&lb_discovery::BuildInfo>) -> Option<Self> {
        info.map(|i| Self {
            name: i.name.clone(),
            version: i.version.clone(),
        })
    }
}
