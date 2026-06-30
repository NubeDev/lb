//! The workspace-relative bus subject the telemetry **live tail** rides (telemetry-console scope).
//! One place owns the key so the publisher (the `SurrealCappedLayer`, right after the capped insert)
//! and the subscriber (`telemetry.tail`) always agree, and so the workspace wall is structural:
//! `lb_bus::publish`/`subscribe` prepend `ws/{id}/`, so a tail in ws-B physically cannot observe a
//! ws-A event (§7) — the read-surface wall the operator sink legitimately doesn't have.
//!
//! The tail is **motion** (§3.3): a dropped subscriber misses a live row but the capped ring keeps the
//! recent history; a `tail` may seed a catch-up snapshot from the store, then fold live motion. The
//! store row is the recent-history record; this subject is never the record.

/// The workspace-relative subject for the telemetry live tail. `lb_bus` walls it under `ws/{id}/` →
/// `ws/{id}/telemetry/events`. A host-internal prefix (`telemetry/`), never a caller-nameable
/// `bus.*` subject, so it never collides with the `ext/`-namespaced user subjects.
pub const TAIL_SUBJECT: &str = "telemetry/events";
