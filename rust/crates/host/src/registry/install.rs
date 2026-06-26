//! `install_from_registry` — the registry's install verb: pull (verified) THEN install (registry
//! scope, README §6.4 "verifies its signature, caches it locally, and instantiates it through the
//! runtime"). **Rollback is the same verb with a prior version** — there is no bespoke rollback path
//! (registry scope: a rollback flag would be durable state the stateless-extension rule forbids).
//!
//! It composes, it does not re-invent: `pull` does fetch/verify/cache; the EXISTING
//! `lb_host::install_extension` does the S4 durable install (persist `requested ∩ admin_approved` as
//! the `Install` record, then load the component). So the registry install is *the S4 install with a
//! verified pull in front* — one trust model, one grant computation, no second copy. The signature
//! gate (in `pull`) and the capability/grant gate (in `install_extension`) stay two independent gates.

use lb_registry::{TrustedKeys, Visibility};

use super::error::RegistryServiceError;
use super::pull::pull;
use super::source::Source;
use crate::boot::Node;
use crate::install::install_extension;
use crate::load::Loaded;

/// Install (or roll back to) `ext_id`@`version` in workspace `ws` from `source`. Pulls + verifies the
/// artifact (offline-served if cached), then persists the `requested ∩ admin_approved` grant and loads
/// the component via the S4 install flow. `trusted` is the publisher-key allow-list; `visibility` tags
/// the catalog entry; `ts` is the injected logical timestamp.
///
/// Caller has passed the MCP gate (`authorize_registry`); the install flow re-computes the grant
/// intersection, so a tampered manifest cannot widen privilege even if it slipped past (it can't —
/// the digest binds the manifest). Returns the `Loaded` result (granted caps + registered tools).
#[allow(clippy::too_many_arguments)]
pub async fn install_from_registry<S: Source>(
    node: &Node,
    source: &S,
    ws: &str,
    ext_id: &str,
    version: &str,
    trusted: &TrustedKeys,
    admin_approved: &[String],
    visibility: Visibility,
    ts: u64,
) -> Result<Loaded, RegistryServiceError> {
    // 1. Pull + VERIFY (offline-served if already cached). A bad artifact is refused here.
    let artifact = pull(
        &node.store,
        source,
        ws,
        ext_id,
        version,
        trusted,
        visibility,
        ts,
    )
    .await?;

    // 2. Install the VERIFIED bytes through the existing S4 flow: persist the grant record, load the
    //    component. Rollback re-runs this with the prior version — the Install record upserts.
    let loaded = install_extension(
        node,
        ws,
        &artifact.manifest_toml,
        &artifact.wasm,
        admin_approved,
        ts,
    )
    .await?;
    Ok(loaded)
}
