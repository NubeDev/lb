//! Role-only: extension registry host (README §6.4) — cloud, the catalog authority + signed-artifact
//! origin a node pulls from.
//!
//! The S7 registry *client* (pull · verify · cache · install · rollback) ships in the host `registry`
//! service behind a `Source` fetch seam (`lb_host::Source`), with `lb_registry` owning artifact
//! identity + signature verification. This role crate is the **server** end of that seam — the cloud
//! catalog + signed-artifact store a real HTTP `Source` impl would talk to. It stays a placeholder
//! until the network transport lands (registry scope non-goal: "no running registry-host HTTP server
//! this slice"); the in-memory test `Source` stands in for it for now. The crate exists so the §9
//! crate map and the dependency graph are real from day one.
#![allow(dead_code)]
