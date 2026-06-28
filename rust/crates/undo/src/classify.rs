//! Reversibility classification from **runtime taint**, not trusted metadata
//! (`docs/scope/undo/undo-scope.md` "Intent": classification is runtime transaction taint).
//!
//! The host tracks, for the in-flight transaction, whether it reached the outbox. This crate does
//! not own that tracking (the host does, at the dispatch/outbox seam); it owns the *rule* that turns
//! the taint + any manifest-declared compensation into the authoritative [`Class`]. Keeping the rule
//! here — pure, unit-tested — is what makes the composition (`max`) and the no-downgrade invariant
//! auditable in one place.

use crate::model::Class;

/// Derive the authoritative class of an action from runtime facts.
///
/// - `reached_outbox`: did the transaction (including nested tool calls) enqueue an outbox effect?
/// - `declared_compensation`: an optional compensating tool the manifest declared.
///
/// Rules (the load-bearing ones):
/// 1. If the transaction reached the outbox it is **irreversible** — derived, never trusted from a
///    manifest. A declared compensation only *upgrades* irreversible → compensable (adds a handle);
///    it can never downgrade to reversible.
/// 2. If it did not reach the outbox it is **reversible** — even if a (spurious) compensation was
///    declared, there is nothing irreversible to compensate, so the before-image undo is correct.
pub fn classify(reached_outbox: bool, declared_compensation: Option<&str>) -> Class {
    if reached_outbox {
        match declared_compensation {
            Some(tool) if !tool.is_empty() => Class::Compensable {
                compensation_tool: tool.to_string(),
            },
            _ => Class::Irreversible,
        }
    } else {
        Class::Reversible
    }
}
