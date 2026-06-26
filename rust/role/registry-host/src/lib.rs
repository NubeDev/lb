//! Role: extension **registry host** (README §6.4) — the cloud catalog + signed-artifact origin a
//! node pulls from, plus the matching HTTP **client** for the host's `Source` fetch seam.
//!
//! The S7 registry *client logic* (pull · verify · cache · install · rollback) ships in the host
//! `registry` service behind a `Source` trait (`lb_host::Source`), with `lb_registry` owning artifact
//! identity + signature verification. This crate is the **transport unit** that fills the seam's last
//! mock: a real HTTP `registry-host` **server** ([`router`]/[`serve`]) that serves signed
//! [`lb_registry::Artifact`]s by `(ext_id, version)`, and an [`HttpSource`] that implements
//! `lb_host::Source` over HTTP.
//!
//! The security boundary is unchanged: the server is a **dumb origin** (it neither signs nor
//! verifies — signing is the publisher's job, verification the client's), the wire is **untrusted**,
//! and `HttpSource::fetch` returns bytes that `lb_host::pull` runs through `verify_artifact` before
//! caching. A tamper in transit is caught by the same gate that catches a tamper at rest.

mod catalog;
mod client;
mod routes;
mod server;

pub use catalog::ArtifactStore;
pub use client::HttpSource;
pub use server::{router, serve};
