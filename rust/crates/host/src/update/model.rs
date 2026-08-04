//! The **pinned, generic** wire types the `update.*` family answers with (node-update scope §Seam 1).
//!
//! "The types are pinned here, and they are generic — no field may name a backend." A provider whose
//! backend has a richer notion (bad-version lists, rollout phases) maps it into these shapes; a field
//! only one backend can fill is a leak (scope decision 11). Downstream repos compile against these,
//! so treat every field as public API.

use serde::{Deserialize, Serialize};

/// Where the resolved credential came from. Reported, never the value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CredentialSource {
    /// Sealed in the node's boot workspace, host-owned (the normal state after enrolment).
    Sealed,
    /// Resolved from the env var NAME the embedder configured (the fallback).
    Env,
    /// Nothing resolved — the node is not enrolled.
    None,
}

/// The credential's observable state: configured?, where from, and a fingerprint. **Never a value.**
/// The fingerprint (first/last 4 of the SHA-256 hex) lets an operator tell two credentials apart
/// without seeing either.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialStatus {
    pub configured: bool,
    pub source: CredentialSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
}

impl Default for CredentialStatus {
    fn default() -> Self {
        Self {
            configured: false,
            source: CredentialSource::None,
            fingerprint: None,
        }
    }
}

/// The verdict of the last update the backend executed. `outcome` is the backend's own token
/// (conventionally `committed` / `rolled-back` / `failed`) — lb does not parse or order it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateOutcome {
    pub tx: String,
    pub outcome: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// What `update.status` answers. Every field is answerable by any backend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateStatus {
    /// `false` ⇒ this node has no provider; every other verb is `Unsupported`.
    pub supported: bool,
    /// A provider-chosen label, e.g. `"supervisor"`. Free text, never matched on by core.
    pub backend: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_version: Option<String>,
    /// Is the node's token-signing key durable across a restart? A per-boot key means this update
    /// signs every session out mid-flight — a UI cannot warn about that unless the node says so.
    pub signing_key_durable: bool,
    /// The tx id of an update the backend reports as in flight, when it reports one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_flight: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last: Option<UpdateOutcome>,
    /// Versions the backend refuses to retry. The generic home for a backend's bad-version notion.
    #[serde(default)]
    pub quarantined: Vec<String>,
    pub credential: CredentialStatus,
    /// The provider's own identity check: does the package/instance it would act on describe THIS
    /// process? A `false` here is a misconfiguration an operator must see before clicking apply.
    pub target_matches_self: bool,
}

impl UpdateStatus {
    /// The honest answer for a node with **no provider** — the `UnconfiguredModel` posture. Not an
    /// error, not a 404: a plain, complete record saying "this node cannot replace itself".
    pub fn unsupported() -> Self {
        Self {
            supported: false,
            backend: String::new(),
            package: None,
            instance: None,
            current_version: None,
            signing_key_durable: false,
            in_flight: None,
            last: None,
            quarantined: Vec::new(),
            credential: CredentialStatus::default(),
            target_matches_self: false,
        }
    }
}

/// One reachable version. `source` is the provider's own label (`"remote"`, `"local"`, …); lb never
/// parses, compares, or orders versions — `check` returns the provider's order and the UI shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AvailableVersion {
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    pub source: String,
}

/// What `apply`/`rollback` answer: **accepted, never done**. The process serving the reply is the
/// process about to be replaced; any other contract is a lie the first time it is true.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Accepted {
    pub tx: String,
}

/// One row of the provider's journal. lb merges its own audited actor in by `tx` (decision 2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateEvent {
    pub tx: String,
    pub at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    pub outcome: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}
