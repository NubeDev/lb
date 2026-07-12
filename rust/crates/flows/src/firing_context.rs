//! The **firing context** (`fctx`) — the load-bearing seam of `any`-port funnels (flow-input-ports-
//! scope "How `any` fires more than once"). A node downstream of an `any` funnel inherits the
//! funnel's multiplicity: it must settle once **per funnel firing**, each reading its own firing's
//! upstream message. The primitive that makes that work is a **per-message identity carried down the
//! run** in an additive envelope field.
//!
//! - A firing context is **minted at each multi-wire `any` slot** when the port releases for a
//!   settled upstream, and **carried forward** down every wire the firing traverses (like `topic`/
//!   `parts`). A single-wire `any` port **propagates** the incoming `fctx` without extending it, so
//!   a linear chain's lineage never grows and its keys stay byte-identical (flow-plain-wiring-scope).
//! - Every step-output claim key, `${steps.*}` resolution, per-node job key, and outbox dedup key is
//!   **scoped by `fctx`** (the run-store claim-key seam). In the common case (frontier nodes +
//!   single-wire chains, which PROPAGATE) `fctx` is the **empty string**, so a plain linear flow's
//!   keys + resolution are **byte-for-byte the pre-ports engine's** — the
//!   `fctx` is an additive suffix that only appears when an `any` funnel is in play.
//! - Nested / diamond funnels **compose by extending** the context, so multiplicity multiplies
//!   correctly along path count (deterministically keyed, still one run).
//!
//! The context is **deterministic per `(node, upstream, parent fctx)`**, so a redelivered message
//! re-mints the SAME `fctx`, re-claims the same slot, and no-ops — exactly-once holds per
//! `(node, fctx)` (Decision 8's CAS claim, now keyed on the slot).

/// The additive envelope field that carries the firing context down a wire (rides like `topic` /
/// `parts` — flow-message-envelope-scope D4 / Decision 15 precedent). Empty in the common case (frontier +
/// single-wire chains) ⇒ no carry-over ambiguity.
pub const FCTX_FIELD: &str = "fctx";

/// The separator between an upstream node id and the triggering-upstream id within one firing
/// segment: `{node}#{upstream}`. Chosen so it never appears inside a node id (which is identifier-
/// like); a node id containing `#` or [`SEGMENT_SEP`] would corrupt the firing id.
const UPSTREAM_SEP: char = '#';
/// The separator between firing segments when a firing passes through a second multi-wire `any`
/// slot (nested / diamond funnels): `funnel#mqtt-a` → `funnel#mqtt-a·funnel2#x`. The middle dot
/// (U+00B7) is used so it cannot collide with an identifier-like node id.
const SEGMENT_SEP: char = '·';

/// Mint the firing context for a **multi-wire** `any` port's release for `triggering_upstream`.
/// (A single-wire `any` port PROPAGATES the incoming `fctx` unchanged instead — no mint — so a
/// linear chain never grows its lineage and keeps today's byte-identical keys;
/// flow-plain-wiring-scope.)
///
/// `parent_fctx` is the firing context the triggering upstream carried (the wave this firing rides
/// in). The new id appends a segment `{self_node}#{triggering_upstream}`, so a downstream node
/// reading `${steps.<self_node>}` under this `fctx` resolves to THIS firing's settle — and a second
/// multi-wire `any` slot downstream extends it again (`mint` composes):
/// `""` → `"funnel#mqtt-a"` → `"funnel#mqtt-a·funnel2#w"`.
///
/// `parent_fctx == ""` ⇒ the segment alone (the top-level funnel case). Deterministic per
/// `(self_node, triggering_upstream, parent_fctx)` so a redelivered upstream re-mints the same id.
pub fn mint(self_node: &str, triggering_upstream: &str, parent_fctx: &str) -> String {
    let segment = format!("{self_node}{UPSTREAM_SEP}{triggering_upstream}");
    if parent_fctx.is_empty() {
        segment
    } else {
        format!("{parent_fctx}{SEGMENT_SEP}{segment}")
    }
}

/// The step-output record id suffix for a `(node, fctx)` slot. `""` ⇒ no suffix (the common-case claim
/// key is `flow_step_output:{ws}:{run}:{node}` byte-for-byte); a non-empty `fctx` ⇒ `@{fctx}`. This
/// is the run-store claim-key seam: one record per `(node, fctx)`, the same key shape redelivery
/// re-claims.
pub fn slot_suffix(fctx: &str) -> String {
    if fctx.is_empty() {
        String::new()
    } else {
        format!("@{fctx}")
    }
}

/// Whether `candidate` is an **ancestor** of (or equal to) `current` in the firing lineage — the
/// `${steps.X}` resolution rule (flow-plain-wiring-scope): X's settle matches when its `fctx` is a
/// prefix of the current firing's, **through whole `·`-separated segments**, falling back to `""`
/// (the root — an ancestor of every firing). This is what keeps a grandparent binding resolvable
/// down a linear chain under universal `any`, while a genuine cross-branch settle (a sibling
/// segment) never matches.
pub fn is_ancestor(candidate: &str, current: &str) -> bool {
    if candidate.is_empty() || candidate == current {
        return true;
    }
    // A strict segment-prefix: `a#x` is an ancestor of `a#x·b#y`, but NOT of `a#xy` (the boundary
    // must be a whole segment, so the check requires the separator right after the prefix).
    current
        .strip_prefix(candidate)
        .is_some_and(|rest| rest.starts_with(SEGMENT_SEP))
}

/// The triggering upstream node id encoded in a non-empty `fctx`'s **last** segment (the immediate
/// `any` slot this firing was minted by). `None` for an empty `fctx` (a barrier/frontier firing — no
/// triggering upstream). Used by an `any` firing to auto-wire its single arriving message: the node
/// reads the triggering upstream's settle (under the parent fctx) as its input.
pub fn triggering_upstream_of(fctx: &str) -> Option<&str> {
    let fctx = fctx.strip_prefix('@').unwrap_or(fctx);
    if fctx.is_empty() {
        return None;
    }
    let last_segment = fctx.rsplit(SEGMENT_SEP).next()?;
    last_segment.split_once(UPSTREAM_SEP).map(|(_, up)| up)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_fctx_is_the_all_all_common_case() {
        // No suffix ⇒ the claim key is byte-for-byte today's `{run}:{node}`.
        assert_eq!(slot_suffix(""), String::new());
        assert_eq!(triggering_upstream_of(""), None);
    }

    #[test]
    fn mint_appends_a_segment_to_an_empty_parent() {
        // Top-level multi-wire funnel: `funnel` fires for mqtt-a ⇒ fctx = "funnel#mqtt-a".
        let f = mint("funnel", "mqtt-a", "");
        assert_eq!(f, "funnel#mqtt-a");
        assert_eq!(slot_suffix(&f), "@funnel#mqtt-a");
        assert_eq!(triggering_upstream_of(&f), Some("mqtt-a"));
    }

    #[test]
    fn mint_extends_a_non_empty_parent_for_nested_funnels() {
        // A firing carrying "funnel#mqtt-a" passes through a second multi-wire any slot `funnel2`
        // fired by upstream `w` ⇒ the context extends (path-count multiplicity, deterministic).
        let parent = mint("funnel", "mqtt-a", "");
        let f = mint("funnel2", "w", &parent);
        assert_eq!(f, "funnel#mqtt-a·funnel2#w");
        // The LAST segment names the immediate triggering upstream ("w"); the prefix is the parent.
        assert_eq!(triggering_upstream_of(&f), Some("w"));
    }

    #[test]
    fn mint_is_deterministic_per_node_upstream_parent() {
        // Redelivery re-mints the SAME fctx ⇒ re-claims the same slot ⇒ exactly-once per firing.
        assert_eq!(mint("funnel", "mqtt-a", ""), mint("funnel", "mqtt-a", ""));
        // A different upstream ⇒ a different fctx (a different firing, not the same one swallowed).
        assert_ne!(mint("funnel", "mqtt-a", ""), mint("funnel", "mqtt-b", ""));
    }

    #[test]
    fn is_ancestor_matches_whole_segment_prefixes_only() {
        // "" is the root — an ancestor of everything (incl. itself).
        assert!(is_ancestor("", ""));
        assert!(is_ancestor("", "funnel#a"));
        assert!(is_ancestor("", "funnel#a·f2#w"));
        // Equal fctx ⇒ ancestor (the same firing).
        assert!(is_ancestor("funnel#a", "funnel#a"));
        // A whole-segment prefix ⇒ ancestor (the lineage walk).
        assert!(is_ancestor("funnel#a", "funnel#a·f2#w"));
        assert!(is_ancestor("funnel#a·f2#w", "funnel#a·f2#w·f3#x"));
        // A sibling segment is NOT an ancestor (cross-branch never matches).
        assert!(!is_ancestor("funnel#b", "funnel#a·f2#w"));
        // A character-prefix that is not a whole segment is NOT an ancestor.
        assert!(!is_ancestor("funnel#a", "funnel#ab"));
        // A descendant is not an ancestor (the relation is directional).
        assert!(!is_ancestor("funnel#a·f2#w", "funnel#a"));
        // Non-empty never ancestors the empty root.
        assert!(!is_ancestor("funnel#a", ""));
    }

    #[test]
    fn slot_suffix_round_trips_through_triggering_upstream_of() {
        for (node, up, parent) in [
            ("funnel", "a", ""),
            ("funnel", "b", ""),
            ("funnel2", "w", "funnel#a"),
            ("funnel3", "x", "funnel#a·funnel2#w"),
        ] {
            let fctx = mint(node, up, parent);
            assert_eq!(triggering_upstream_of(&fctx), Some(up));
            assert_eq!(slot_suffix(&fctx), format!("@{fctx}"));
        }
    }
}
