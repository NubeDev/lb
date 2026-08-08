//! [`BuildInfo`] — what program embedded this node (embedder-build-info scope).
//!
//! Every version lb publishes is lb's own: `GET /health` reports `lb-role-gateway`'s
//! `CARGO_PKG_VERSION` and `GET /node` copies the same constant. For the stock binary that is
//! exactly right — lb *is* the product. For an **embedder**, a host that boots lb as a library
//! through `BootConfig`, it is a dead end: there was no field in which to state what *it* is, so
//! the only version an operator, installer, or fleet tool could read off the node was the core's.
//! The failure is quiet, which is what makes it worth a type: the number on screen is real, stable,
//! and about a different piece of software than the one that was asked about.
//!
//! This is the seam. An embedder fills [`BootConfig::build_info`](../../node/config) with one of
//! these and lb publishes it **beside** its own version — never instead of it — on `GET /node`,
//! `GET /health`, and the mDNS advertisement, so the three cannot disagree.
//!
//! # lb never derives this (rule 10)
//!
//! Both fields are opaque display strings. lb does not guess a product name, does not fall back to
//! its own, does not parse or validate the version, and has no default: an embedder states its
//! identity or it is absent. *How* the version string is computed — a `build.rs`, a `git describe`,
//! a date stamp — is entirely the embedder's business, and no core crate learns which embedder is
//! on top. Swapping the product that embeds lb changes no lb code.
//!
//! # It is published unauthenticated
//!
//! `product` rides the existing unauthenticated `/node` and `/health` and the cleartext mDNS TXT
//! record. It is identity-of-software — the same class as the `version` already published there —
//! and never workspace, persona, capability, member, or extension data. Two honest caveats:
//! whatever the embedder puts in `version` is disclosed verbatim to anything that can reach the
//! port (a commit id, if that is what it built), and nothing here is a trust signal — a TXT value
//! is trivially forged, and nothing may route, address, or authorize by either field.
//!
//! A deployment that considers its product identity sensitive leaves the field `None`; absence is a
//! supported posture, not a degraded one, and reproduces pre-`BuildInfo` behaviour byte for byte.
//!
//! # Absence is ambiguous at the far end
//!
//! A missing `product` means "not an embedder" *or* "an lb older than this seam". A consumer that
//! must tell those apart reads lb's `version`, which is always present.

/// The product identity of the host that embedded this node — the program on top of the core.
///
/// See the module docs: lb never derives either field, never parses them, and publishes them
/// unauthenticated beside (never instead of) its own version.
///
/// `#[non_exhaustive]` on purpose. A build timestamp, toolchain, or target triple all have obvious
/// fleet uses and may be worth adding later; this keeps that additive. An embedder that wants one
/// today puts it in [`version`](Self::version), which is free-form.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BuildInfo {
    /// The product's name, e.g. the embedding crate's package name. Display text, never an
    /// identifier — nothing routes or authorizes by it, and it is forgeable on the wire.
    pub name: String,
    /// The product's build version, free-form. Semver build metadata (`0.1.1+g1a2b3c4d5e6f`) is the
    /// expected shape but nothing here requires it: a date stamp or a bare SHA is equally fine.
    pub version: String,
}

impl BuildInfo {
    /// State a product identity. The only constructor, so `#[non_exhaustive]` stays additive for
    /// any field a later scope adds.
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
        }
    }
}
