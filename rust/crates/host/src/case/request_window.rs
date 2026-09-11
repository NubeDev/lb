//! **How long the party gets** — the ask window, resolved in one place (case-plane scope, wave 2).
//!
//! The ladder, in order:
//!   1. the **party's** `default_ask_window_h`, when they have an opinion (`> 0`);
//!   2. else the matching `ServicePolicy.party_window_h` — the same policy the case's deadlines came
//!      from, resolved against the same three facets (site / category / severity);
//!   3. else [`lb_cases::DEFAULT_PARTY_WINDOW_H`], the crate's stated default.
//!
//! Party-first, and not the other way round, because the party window is a statement about *this
//! company's* responsiveness — the people who answer in an hour and the people who answer in a week
//! should not share a nudge ladder — while the policy window is the workspace's blanket assumption
//! for everyone it has said nothing specific about.
//!
//! One file because this number decides four things at once (`expires_ts`, `respond_by`, and the
//! two nudge instants), and a second resolution of it anywhere would be a second answer to "when is
//! this due".

use lb_cases::{Case, Party, DEFAULT_PARTY_WINDOW_H};
use lb_store::Store;

use super::error::CaseSvcError;

/// One hour in epoch-milliseconds — the unit the case plane stores every timestamp in.
pub(super) const HOUR_MS: u64 = 60 * 60 * 1000;

/// The resolved window for asking `party` about `case`, in hours (never zero).
pub(super) async fn window_hours_for(
    store: &Store,
    ws: &str,
    case: &Case,
    party: &Party,
) -> Result<u32, CaseSvcError> {
    if party.default_ask_window_h > 0 {
        return Ok(party.default_ask_window_h);
    }
    let policy = lb_cases::match_policy(
        store,
        ws,
        case.site.as_deref(),
        case.category.as_deref(),
        Some(&case.severity),
    )
    .await?;
    // A policy whose `party_window_h` is 0 is treated as "no opinion" too, not as "answer
    // instantly": a zero window would mint a link that is dead before it is sent, and
    // `request_open` refuses that outright. Falling through to the stated default is the honest
    // reading of a field nobody filled in.
    Ok(policy
        .map(|p| p.party_window_h)
        .filter(|h| *h > 0)
        .unwrap_or(DEFAULT_PARTY_WINDOW_H))
}

/// The three nudge instants for a window of `window_h` hours starting at `sent_ts` (epoch-ms):
/// **50 %**, **80 %**, and the **breach** at 100 %.
///
/// 50/80 rather than a fixed "one day before": a proportional ladder means a 4-hour ask and a
/// 5-day ask both get a reminder while there is still time to act, which a fixed offset cannot do
/// for both. The third is not a nudge to the party at all — it is the escalation that tells us they
/// went quiet (see `request_nudge.rs`).
pub(super) fn nudge_instants(sent_ts: u64, window_h: u32) -> [u64; 3] {
    let window_ms = window_h as u64 * HOUR_MS;
    [
        sent_ts + window_ms / 2,
        sent_ts + window_ms * 4 / 5,
        sent_ts + window_ms,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ladder_is_half_four_fifths_and_the_deadline() {
        // A 10-hour window from an arbitrary instant.
        let [half, four_fifths, breach] = nudge_instants(1_000, 10);
        assert_eq!(half, 1_000 + 5 * HOUR_MS);
        assert_eq!(four_fifths, 1_000 + 8 * HOUR_MS);
        assert_eq!(breach, 1_000 + 10 * HOUR_MS);
    }

    #[test]
    fn every_instant_is_strictly_after_the_send_and_ordered() {
        let [a, b, c] = nudge_instants(0, 1);
        assert!(0 < a && a < b && b < c, "{a} {b} {c}");
    }
}
