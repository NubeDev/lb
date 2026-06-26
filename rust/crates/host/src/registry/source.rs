//! The registry's **fetch seam** — the `Source` an artifact is pulled from (registry scope). Host-owned,
//! exactly like the outbox's `Target` and the agent's `ModelAccess`: the host defines the trait and the
//! `pull` verb calls only it; a real HTTP `registry-host` client (S7 follow-up) or the test supplies the
//! impl. This is what makes the pull path testable deterministically — the test source is a map of
//! `(ext_id, version) → Artifact` that can be told "offline" (every fetch errors) to prove the cached
//! path never touches it.
//!
//! Why a `Source` and NOT the outbox: a pull is a request-scoped READ the caller waits on, not a
//! fire-and-forget must-deliver write. Forcing it through the outbox would invert the dependency (the
//! install would poll for its own artifact). So the registry borrows the seam *shape* — fetch behind a
//! host trait, deterministic test impl — without the relay (registry scope, "why a Source and not the
//! outbox"). The fetched artifact is UNTRUSTED; `pull` verifies it before caching.

use std::future::Future;

use lb_registry::Artifact;

use super::error::RegistryServiceError;

/// A source of signed artifacts. `fetch` returns the artifact for `(ext_id, version)` if the origin
/// has it; `Err(NotAvailable)` if the origin lacks it OR is unreachable (offline) — the two are
/// indistinguishable to the caller, which is correct: an offline node with the artifact cached never
/// calls `fetch` at all, and one without it cannot tell "offline" from "no such version". The
/// returned bytes are untrusted until `verify_artifact` checks them.
pub trait Source {
    /// Fetch the artifact for `ext_id`@`version`. The implementation performs the network transfer
    /// (or, in tests, a map lookup); it does NO verification — that is `pull`'s job, host-side.
    fn fetch(
        &self,
        ext_id: &str,
        version: &str,
    ) -> impl Future<Output = Result<Artifact, RegistryServiceError>> + Send;
}
