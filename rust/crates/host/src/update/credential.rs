//! Credential custody — **host-owned, verified, unreadable** (node-update scope §Credential custody).
//!
//! The credential resolves per call: **sealed secret → env NAME → `None`**. The sealed record lives
//! at `secret:{boot_workspace}:{credential_secret}` and is stamped **owner = the node host
//! principal**, not the calling admin. That one choice settles three things:
//!
//! - **Nobody can read it back.** The secret plane denies `get` on a `Private` secret to any
//!   non-owner *even with the capability*; since the owner is the host, every human caller is a
//!   non-owner. (And since decision 9's raw-read wall, `store.query`/`scan`/`graph` refuse the
//!   `secret` table structurally, so the window is shut as well as the door.)
//! - **Anyone properly granted can rotate it** — through `update.credential.*`, gated by its own cap.
//! - **It is node-scoped**, like the thing it controls.
//!
//! The value never leaves the node process: it is returned to the provider and to nothing else.
//! Only a [`fingerprint`] ever crosses the wire.

use lb_auth::Principal;
use lb_secrets::{get as secret_get, reclaim as secret_reclaim, SecretsError, Visibility};
use sha2::{Digest, Sha256};

use super::error::UpdateError;
use super::installed::InstalledUpdate;
use super::model::{CredentialSource, CredentialStatus};
use crate::boot::Node;

/// The subject that OWNS the sealed credential. A `host:` subject no login can ever mint, so the
/// owner wall (secrets gate 3) denies `secret.get` to every human caller, admin included.
pub const HOST_SUBJECT: &str = "host:update";

/// The host mediator principal: `host:update` carrying exactly the two caps needed to manage the
/// node's OWN credential at `path`, in the boot workspace. Host-constructed — the CALLER already
/// passed the `update.*` capability gate at the verb boundary, and the value never crosses back.
fn mediator(ws: &str, path: &str) -> Principal {
    Principal::routed(
        HOST_SUBJECT.to_string(),
        ws.to_string(),
        vec![format!("secret:{path}:write"), format!("secret:{path}:get")],
    )
}

/// The credential's public shadow: first/last 4 of its SHA-256 hex. Enough to tell two credentials
/// apart, useless for recovering either. This is the ONLY derivation of a credential that is ever
/// returned, logged, or rendered.
pub fn fingerprint(value: &str) -> String {
    let hex = format!("{:x}", Sha256::digest(value.as_bytes()));
    format!("{}…{}", &hex[..4], &hex[hex.len() - 4..])
}

/// Read the sealed credential, if one exists. `None` covers both "no path configured" and "nothing
/// sealed yet"; a genuine store fault surfaces as [`UpdateError::Backend`].
async fn sealed(node: &Node, inst: &InstalledUpdate) -> Result<Option<String>, UpdateError> {
    let Some(path) = inst.cfg.credential_secret.as_deref() else {
        return Ok(None);
    };
    let ws = &inst.boot_workspace;
    match secret_get(&node.store, &mediator(ws, path), ws, path).await {
        Ok(v) => Ok(Some(v)),
        Err(SecretsError::NotFound) => Ok(None),
        Err(e) => Err(UpdateError::Backend(format!("secret read failed: {e}"))),
    }
}

/// The env fallback: the NAME the embedder configured, read here because that name rode
/// `BootConfig` down from the binary boundary — the same posture `AgentModelConfig::api_key_env`
/// takes. An empty value counts as absent.
fn from_env(inst: &InstalledUpdate) -> Option<String> {
    let name = inst.cfg.credential_env.as_deref()?;
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// Seal `value` host-owned at the configured path, TAKING ownership (`reclaim`) so a rotation by a
/// different admin can never be blocked by the owner wall — the owner is always `host:update`.
/// `Private` visibility, so even a caller holding `secret:*:get` is refused by gate 3.
pub async fn seal(node: &Node, inst: &InstalledUpdate, value: &str) -> Result<(), UpdateError> {
    let path = inst
        .cfg
        .credential_secret
        .as_deref()
        .ok_or_else(|| UpdateError::Backend("no credential_secret path configured".into()))?;
    let ws = &inst.boot_workspace;
    secret_reclaim(
        &node.store,
        &mediator(ws, path),
        ws,
        path,
        value,
        Visibility::Private,
    )
    .await
    .map_err(|e| UpdateError::Backend(format!("secret write failed: {e}")))
}

/// The resolved credential plus where it came from — the pair every verb needs (the value for the
/// provider, the source for `status`).
pub struct Resolved {
    pub value: Option<String>,
    pub source: CredentialSource,
    /// `true` when this resolution ran first-use auto-enrolment and sealed a fresh credential.
    pub auto_enrolled: bool,
}

impl Resolved {
    /// The caller-visible projection: configured?, source, fingerprint — **never the value**.
    pub fn status(&self) -> CredentialStatus {
        CredentialStatus {
            configured: self.value.is_some(),
            source: self.source,
            fingerprint: self.value.as_deref().map(fingerprint),
        }
    }
}

/// Resolve the credential for one verb call: **sealed → env NAME → first-use auto-enrolment → None**.
///
/// **First-use auto-enrolment** (scope decision 10). Zero-touch matters: an unattended box must end
/// up enrolled with nobody at the console, and the seam gives the host no way to seal a credential
/// outside a caller's `credential.claim`. So when resolution finds nothing sealed and no env value,
/// lb calls `provision_credential(None)` **once**, seals the result host-owned, and proceeds. It is
/// an optimisation of the happy path, never a second protocol: `Unsupported` and
/// `Unauthorized{code_required}` both degrade to the ordinary unconfigured/claim-needed answer.
///
/// A concurrent double-trigger is serialized on [`InstalledUpdate::seal_lock`]; the loser re-reads
/// under the lock and finds the winner's secret, so exactly one credential is ever minted.
pub async fn resolve(node: &Node, inst: &InstalledUpdate) -> Result<Resolved, UpdateError> {
    if let Some(v) = sealed(node, inst).await? {
        return Ok(Resolved {
            value: Some(v),
            source: CredentialSource::Sealed,
            auto_enrolled: false,
        });
    }
    if let Some(v) = from_env(inst) {
        return Ok(Resolved {
            value: Some(v),
            source: CredentialSource::Env,
            auto_enrolled: false,
        });
    }
    // Nowhere to seal ⇒ auto-enrolment cannot run; answer honestly rather than minting a credential
    // that would evaporate with the process.
    if inst.cfg.credential_secret.is_none() {
        return Ok(unenrolled());
    }

    let _guard = inst.seal_lock.lock().await;
    // Re-read UNDER the lock: the loser of a race finds the winner's secret and never provisions.
    if let Some(v) = sealed(node, inst).await? {
        return Ok(Resolved {
            value: Some(v),
            source: CredentialSource::Sealed,
            auto_enrolled: false,
        });
    }
    match inst.cfg.provider.provision_credential(None).await {
        Ok(v) => {
            seal(node, inst, &v).await?;
            Ok(Resolved {
                value: Some(v),
                source: CredentialSource::Sealed,
                auto_enrolled: true,
            })
        }
        // Both are normal answers, not failures: the backend has no handshake, or it wants a code.
        // Either way the node is simply not enrolled yet and the verb proceeds credential-less.
        Err(UpdateError::Unsupported) | Err(UpdateError::Unauthorized { .. }) => Ok(unenrolled()),
        Err(e) => Err(e),
    }
}

/// The "not enrolled" resolution — a value-less, source-`None` answer.
fn unenrolled() -> Resolved {
    Resolved {
        value: None,
        source: CredentialSource::None,
        auto_enrolled: false,
    }
}
