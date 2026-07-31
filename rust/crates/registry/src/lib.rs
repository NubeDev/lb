//! The extension **registry** client primitives — artifact identity + signature verification
//! (README §6.4, registry scope). The S7 driver: a node pulls a signed artifact, *verifies* it, and
//! caches it; this crate owns the artifact shape and the verification gate.
//!
//! Two jobs, deliberately separate (the same split as `ext-loader`'s parse-vs-grant):
//! 1. [`digest`] computes the content digest that binds an artifact's manifest AND wasm.
//! 2. [`verify_artifact`] proves the digest + an Ed25519 signature against an allow-listed publisher
//!    key, returning a [`VerifiedArtifact`] — the **only** value the cache will accept. So unverified
//!    bytes can never be cached (verify-before-cache, enforced by the type system).
//!
//! The signature half of (2) has a caller-named development escape hatch,
//! [`verify_artifact_with`] + [`Authenticity`]; the digest half does not and never will. See the
//! `verify` module docs for the full contract and why the two checks are kept separate.
//!
//! This crate holds **no store, no authorization, no network** — exactly like `lb_outbox`/`lb_jobs`,
//! it is the record + the pure verb. The cache, the `Source` fetch, and the `mcp:registry.*` gate
//! all live in the host `registry` service (capability-first §3.5; the host is the chokepoint).
//! Verification is the only new crypto surface and reuses `ed25519-dalek` exactly as `lb_auth` does.

mod digest;
mod error;
mod model;
mod verify;

pub use digest::{digest, digest_hex};
pub use error::RegistryError;
pub use model::{Artifact, CatalogEntry, VerifiedArtifact, Visibility};
pub use verify::{verify_artifact, verify_artifact_with, Authenticity, PublisherKey, TrustedKeys};
