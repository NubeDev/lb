//! The ONE place the host derives a dashboard's `managedBy` marker from the saving principal
//! (ext-managed-dashboards scope, D1 + "Risks: marker inference from the principal").
//!
//! A native sidecar's callback token is minted with `sub = "ext:<ext_id>"` (`native/spec.rs`), a
//! stable `Member` principal. A dashboard saved by such a principal is **machine-generated**, and
//! the record says so — so a roster can badge it and a client can explain a refusal instead of
//! showing a bare `Denied`.
//!
//! Two properties this file exists to keep:
//!
//! - **Derived, never accepted.** `managedBy` is computed HERE from the principal; no verb reads it
//!   from caller input. A human cannot mark a board managed, and one extension cannot claim
//!   another's (the owner check already refuses that; this just never gives it a second door).
//! - **One owner of the identity shape.** Matching the `ext:` prefix on a `sub` is a shape match on
//!   an *identity form*, not a branch on WHICH extension (rule 10 — the host never knows the id
//!   means "modbus"). A future identity scheme (a differently-prefixed `sub`, a typed principal
//!   kind) changes this one function, not five call sites.

use lb_auth::Principal;

/// The `sub` prefix an extension principal carries (`native/spec.rs::mint_child_token`).
const EXT_SUB_PREFIX: &str = "ext:";

/// The bare extension id behind `principal`, or `None` for a human/agent principal.
///
/// Reads [`Principal::owner_sub`] — the same identity `dashboard.save` stamps on `owner` — so
/// `managedBy` and `owner` can never disagree about who saved the record (`owner = "ext:modbus"`
/// ⇒ `managedBy = "modbus"`, D1: the bare id, because the full principal is already on `owner`).
///
/// An `ext:` sub with an empty id is treated as NOT an extension: an empty marker is the
/// "unmanaged" sentinel, so a malformed identity must not round-trip as a managed board.
pub fn managed_by_of(principal: &Principal) -> Option<String> {
    principal
        .owner_sub()
        .strip_prefix(EXT_SUB_PREFIX)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lb_auth::Principal;

    fn p(sub: &str) -> Principal {
        Principal::routed(sub, "nube", Vec::new())
    }

    /// An extension principal yields the BARE id — not the `ext:`-prefixed principal (D1).
    #[test]
    fn extension_principal_yields_the_bare_id() {
        assert_eq!(managed_by_of(&p("ext:modbus")).as_deref(), Some("modbus"));
    }

    /// A human (or agent) principal is not managed — the marker stays absent.
    #[test]
    fn human_principal_is_not_managed() {
        assert_eq!(managed_by_of(&p("user:test")), None);
        assert_eq!(managed_by_of(&p("agent:reporter")), None);
        // A sub that merely CONTAINS the token is not an extension identity — the prefix anchors.
        assert_eq!(managed_by_of(&p("user:ext:sneaky")), None);
    }

    /// A malformed `ext:` sub with no id is unmanaged — empty is the unmanaged sentinel, so it must
    /// never become a marker.
    #[test]
    fn empty_extension_id_is_not_managed() {
        assert_eq!(managed_by_of(&p("ext:")), None);
    }
}
