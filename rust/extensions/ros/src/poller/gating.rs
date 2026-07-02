//! Enable-gating — the **driver-agnostic** rule that decides which targets actually poll: a leaf polls
//! only if **connection ∧ network ∧ device ∧ point** `enable` are ALL true (scope: "the effective
//! decision is the AND of the enable flags up the tree"). One write at any level silences everything
//! below it — `network.update {enable:false}` drops all its devices' points with no fan-out.
//!
//! It is a pure function over `PollTarget`'s four flags, so the exactness (each level silences
//! independently; the AND is precise) is unit-tested here once, without a box, a gateway, or the loop.
//! The `Source` only *reports* the flags it sees; this is where the decision lives, kept driver-agnostic
//! so the next driver reuses it unchanged.

use super::source::PollTarget;

/// True iff every level of the target's chain is enabled — the effective poll decision for one leaf.
pub fn is_pollable(t: &PollTarget) -> bool {
    t.connection_enable && t.network_enable && t.device_enable && t.point_enable
}

/// The enabled subset of a target list — what the engine actually reads this tick. Preserves order so
/// batch shaping stays deterministic (a test can assert exactly which series receive samples).
pub fn resolve(targets: &[PollTarget]) -> Vec<PollTarget> {
    targets.iter().filter(|t| is_pollable(t)).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A helper: a target whose four flags are set explicitly, so a test names exactly which level is off.
    fn tgt(id: &str, conn: bool, net: bool, dev: bool, point: bool) -> PollTarget {
        PollTarget {
            id: id.into(),
            series: format!("s.{id}"),
            connection_enable: conn,
            network_enable: net,
            device_enable: dev,
            point_enable: point,
        }
    }

    #[test]
    fn all_enabled_polls() {
        assert!(is_pollable(&tgt("p", true, true, true, true)));
    }

    #[test]
    fn each_level_off_silences_independently() {
        // Exactly one level off at a time — every one must silence the leaf (the AND is exact: no level
        // is ignored, none is sufficient alone).
        assert!(
            !is_pollable(&tgt("p", false, true, true, true)),
            "connection off"
        );
        assert!(
            !is_pollable(&tgt("p", true, false, true, true)),
            "network off"
        );
        assert!(
            !is_pollable(&tgt("p", true, true, false, true)),
            "device off"
        );
        assert!(
            !is_pollable(&tgt("p", true, true, true, false)),
            "point off"
        );
    }

    #[test]
    fn resolve_keeps_only_fully_enabled_in_order() {
        let targets = vec![
            tgt("a", true, true, true, true),  // polls
            tgt("b", true, false, true, true), // network off → silenced
            tgt("c", true, true, true, true),  // polls
            tgt("d", false, true, true, true), // connection off → silenced
        ];
        let live = resolve(&targets);
        let ids: Vec<&str> = live.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "c"], "only fully-enabled, original order");
    }

    #[test]
    fn network_off_silences_a_whole_branch() {
        // Two points under one network; the network flag is off on both → the whole branch drops with a
        // single (reported) flag, the "one write, fleet-wide effect" the scope calls out.
        let targets = vec![
            tgt("p1", true, false, true, true),
            tgt("p2", true, false, true, true),
        ];
        assert!(
            resolve(&targets).is_empty(),
            "network off drops every point under it"
        );
    }
}
