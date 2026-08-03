//! The registry error domain (registry scope). One enum for the artifact-identity + verification
//! crate; the host `registry` service maps these onto `ToolError` at the MCP edge.

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RegistryError {
    /// The artifact failed verification: the recomputed content digest did not match the claimed
    /// digest, or the Ed25519 signature did not verify against an allow-listed publisher key. One
    /// opaque variant for both — a caller learns "not the publisher's bytes", not which check failed
    /// (the same no-existence-signal discipline the MCP gate uses).
    #[error("artifact failed verification")]
    Unverified,
    /// A publisher key id or signature was structurally malformed (wrong length, not valid Ed25519).
    /// Distinct from `Unverified` only so a packaging bug is told apart from a tamper/foreign-key
    /// rejection during development; both are refused before caching.
    #[error("artifact signature material is malformed: {0}")]
    Malformed(String),
    /// The zip-transport envelope itself was malformed — a container problem, not a signed-artifact
    /// problem: not a readable zip, the wrong entry set, a non-stored payload entry, an oversized
    /// payload, or an EOCD comment that isn't the expected JSON shape. Named apart from `Malformed`
    /// (signature MATERIAL specifically) so a broken/adversarial container is distinguishable from a
    /// crypto problem — a tampered signed claim inside an otherwise well-formed container still
    /// surfaces as `Unverified` from `verify_artifact`, never from here.
    #[error("artifact zip transport error: {0}")]
    Transport(String),
}
