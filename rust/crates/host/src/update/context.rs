//! The per-call preamble every `update.*` verb runs: find the installed seam, resolve the
//! credential, and build the [`UpdateCx`] the provider is called with.
//!
//! One place, so no verb can accidentally skip credential resolution (and therefore first-use
//! auto-enrolment), and no verb can hand the provider a context lb did not build.

use std::sync::Arc;

use lb_auth::Principal;

use super::audit;
use super::credential::{resolve, Resolved};
use super::error::UpdateError;
use super::installed::InstalledUpdate;
use super::provider::UpdateCx;
use crate::boot::Node;

/// The installed seam, or [`UpdateError::Unsupported`] — the honest answer for a node whose
/// embedder filled no `BootConfig.update`. Never a `NotFound`: the verb exists on every node; this
/// node simply cannot replace itself.
pub fn installed(node: &Node) -> Result<Arc<InstalledUpdate>, UpdateError> {
    node.update().ok_or(UpdateError::Unsupported)
}

/// Everything a verb needs after the preamble.
pub struct Prepared {
    pub inst: Arc<InstalledUpdate>,
    pub cx: UpdateCx,
    pub resolved: Resolved,
}

/// Resolve the credential (running first-use auto-enrolment when nothing is sealed) and build the
/// provider context stamped with the calling actor. Audits the auto-enrolment when it fires, with
/// the triggering caller as the actor — "every path into the sealed record is one of `set`, `claim`,
/// or auto-enrolment, all audited" (scope decision 10).
pub async fn prepare(
    node: &Node,
    principal: &Principal,
    inst: Arc<InstalledUpdate>,
) -> Result<Prepared, UpdateError> {
    let resolved = resolve(node, &inst).await?;
    if resolved.auto_enrolled {
        audit::record(
            &node.store,
            &inst.boot_workspace,
            principal.sub(),
            "update.credential.claim",
            "credential",
            "auto_enrolled",
            None,
        )
        .await;
    }
    let cx = UpdateCx {
        credential: resolved.value.clone(),
        actor: principal.sub().to_string(),
    };
    Ok(Prepared { inst, cx, resolved })
}
