//! `lb ext publish` — the `make publish-ext` retirement (operator-cli scope). Unlike every typed
//! verb (which routes through `/mcp/call`), publish is a dedicated `POST /extensions` upload: its body
//! is the signed [`Artifact`] JSON, not a `{tool, args}` call, because the artifact bytes + signature
//! are the payload the verify-before-store gate checks (the scope maps `lb ext publish` → `POST
//! /extensions` explicitly). One trait, two impls mirroring the transport split: remote POSTs the
//! artifact; local calls `lb_host::ext_publish` in-process.
//!
//! Local mode trusts the artifact's OWN publisher key — on the operator's own machine the operator
//! signed it, exactly the shortcut the gateway's dev publish path takes for its LB_DIR key. Remote
//! mode trusts nothing client-side: the gateway verifies against ITS `LB_TRUSTED_PUBKEYS` allow-list,
//! so an operator cannot self-trust an artifact into someone else's node (trust is environment, never
//! the upload).

use reqwest::StatusCode;

use lb_registry::{Artifact, PublisherKey, TrustedKeys, Visibility};

use crate::error::{CliError, CliResult};

use super::{Local, Remote};

/// The result of a publish attempt. A publish is a 204 (no body) on success — there is nothing to
/// render but the outcome, so this is a small marker the command turns into a one-line message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishOutcome {
    /// Verified, installed, loaded live (the `204` / the local `Ok`).
    Published,
}

/// The publish capability a transport exposes. Kept separate from [`Transport`](super::Transport)
/// because publish is a distinct route (not `/mcp/call`) with a distinct body (an `Artifact`, not
/// `{tool, args}`) and distinct status semantics (`422` = verification failure, a first-class outcome).
#[allow(async_fn_in_trait)]
pub trait ExtPublish {
    /// Publish a signed `artifact`. `403` / `Denied` → [`CliError::Denied`]; `422` / `Unverified` →
    /// [`CliError::BadInput`] (the artifact was well-formed but its signature/digest did not check
    /// out — distinct from "you may not"); a down gateway → [`CliError::Transport`].
    async fn publish(&self, artifact: Artifact) -> CliResult<PublishOutcome>;
}

impl ExtPublish for Remote {
    async fn publish(&self, artifact: Artifact) -> CliResult<PublishOutcome> {
        let url = format!("{}/extensions", self.base_url());
        let resp = self
            .client()
            .post(&url)
            .bearer_auth(self.token())
            .json(&artifact)
            .send()
            .await
            .map_err(|e| CliError::Transport(e.to_string()))?;
        let status = resp.status();
        if status == StatusCode::NO_CONTENT || status.is_success() {
            return Ok(PublishOutcome::Published);
        }
        let body = resp.text().await.unwrap_or_default();
        match status {
            StatusCode::FORBIDDEN => Err(CliError::Denied {
                tool: "ext.publish".to_string(),
            }),
            StatusCode::UNPROCESSABLE_ENTITY => Err(CliError::BadInput(format!(
                "artifact rejected (422): {} — is its publisher key in the gateway's LB_TRUSTED_PUBKEYS?",
                body.trim()
            ))),
            other => Err(CliError::Transport(format!(
                "gateway returned {other}: {}",
                body.trim()
            ))),
        }
    }
}

impl ExtPublish for Local {
    async fn publish(&self, artifact: Artifact) -> CliResult<PublishOutcome> {
        // Trust the artifact's own publisher key: local mode is the operator's own node, and the
        // operator signed the artifact — the same self-trust the gateway's dev publish shortcut takes.
        // Verification (digest + signature) still runs inside `ext_publish`; this only says "this
        // publisher is allowed on my own machine", it does not skip the check.
        let trusted = trust_artifact_publisher(&artifact)?;
        lb_host::ext_publish(
            self.node(),
            self.principal(),
            self.principal().ws(),
            artifact,
            &trusted,
            Visibility::Private,
            0,
        )
        .await
        .map_err(map_ext_error)?;
        Ok(PublishOutcome::Published)
    }
}

/// Build a `TrustedKeys` that trusts exactly the artifact's own publisher key. The signature is still
/// verified against it inside `ext_publish`, so a tampered artifact (digest mismatch) still fails —
/// this only supplies the allow-list entry for the key the artifact claims.
fn trust_artifact_publisher(artifact: &Artifact) -> CliResult<TrustedKeys> {
    // The publisher's public key is not carried in the artifact (only the signature is); the artifact
    // names a `publisher_key_id`. For a local self-publish we cannot reconstruct the verifying key from
    // the artifact alone, so local publish reads the same dev key `lb devkit sign` wrote and trusts it.
    // The key material lives beside the CLI's config (the devkit default root).
    let loaded = lb_devkit::load_or_create_key(&devkit_key_path())
        .map_err(|e| CliError::Other(format!("load publisher key: {e}")))?;
    let publisher = PublisherKey::from_bytes(&loaded.signing_key.verifying_key().to_bytes())
        .map_err(|e| CliError::Other(format!("publisher key: {e}")))?;
    let mut trusted = TrustedKeys::new();
    trusted.insert(artifact.publisher_key_id.clone(), publisher);
    Ok(trusted)
}

/// The dev publisher key path `lb devkit sign` uses by default — under the devkit root beside LB_DIR.
fn devkit_key_path() -> std::path::PathBuf {
    lb_devkit::default_devkit_root()
        .join("keys")
        .join("dev-publisher.key")
}

/// Map an `ext_publish` error to the CLI error: a deny is honest-denied; a verification failure is a
/// bad-input (well-formed but unverified); anything else is a generic failure.
fn map_ext_error(e: lb_host::ExtError) -> CliError {
    use lb_host::ExtError;
    match e {
        ExtError::Denied => CliError::Denied {
            tool: "ext.publish".to_string(),
        },
        ExtError::Unverified => CliError::BadInput(
            "artifact failed verification (digest/signature mismatch) — nothing stored".to_string(),
        ),
        other => CliError::Other(format!("publish failed: {other}")),
    }
}
