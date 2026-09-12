//! `policy_list` — every [`ServicePolicy`] in the workspace, **most specific first**
//! (case-plane scope, `policy.sla.list`).
//!
//! The order is not cosmetic: it is the resolution order [`crate::policy_match`] applies. A reader
//! of the settings list can therefore see *why* a case got the policy it got by reading down the
//! page — the first row whose `match` fits wins. A list sorted by name or by creation time would
//! hide that, and "which policy governs this case?" would become a question only the code can
//! answer.

use lb_store::{scan_all, Store};

use crate::error::CasesError;
use crate::policy::{ServicePolicy, POLICY_TABLE};

/// Read every policy row in `ws`, ordered most-specific-first (3 pinned axes → 2 → 1 → the
/// workspace default), ties broken by `id` ascending — the same deterministic tie-break resolution
/// uses, so the list is a faithful picture of the ladder.
///
/// A row that fails to decode is **skipped**, not fatal: one malformed row must not blank the whole
/// settings page and, with it, the admin's only way to fix it. That tolerance is why
/// `a_policy_round_trips_through_a_real_store` exists — a skip is silent by construction, so the
/// only thing standing between it and "the list is mysteriously always empty" is a test that writes
/// through the real store and reads back. (It caught exactly that: the envelope below.)
pub async fn policy_list(
    store: &Store,
    ws: &str,
    include_disabled: bool,
) -> Result<Vec<ServicePolicy>, CasesError> {
    let rows = scan_all(store, ws, POLICY_TABLE).await?;
    let mut policies: Vec<ServicePolicy> = rows
        .into_iter()
        .filter_map(|row| unwrap_policy(row.data))
        .filter(|p| include_disabled || p.active)
        .collect();
    sort_by_specificity(&mut policies);
    Ok(policies)
}

/// Unwrap the `{ data, rev }` write envelope `scan` returns, then decode.
///
/// `lb_store::scan` selects the WHOLE record, so a `write`-based row arrives wrapped; `read` and
/// `list` hand back the inner value already unwrapped. Decoding the wrapper directly yields no
/// fields and — because the miss is a `None` we deliberately skip — an empty list rather than an
/// error. Same unwrap the case lane does.
fn unwrap_policy(row: serde_json::Value) -> Option<ServicePolicy> {
    let inner = match row {
        serde_json::Value::Object(mut obj) => {
            obj.remove("data").unwrap_or(serde_json::Value::Object(obj))
        }
        other => other,
    };
    serde_json::from_value(inner).ok()
}

/// Order most-specific-first, ties by `id` ascending. Shared with the resolution ladder so the two
/// can never disagree about which of two equally specific policies comes first.
pub fn sort_by_specificity(policies: &mut [ServicePolicy]) {
    policies.sort_by(|a, b| {
        b.r#match
            .specificity()
            .cmp(&a.r#match.specificity())
            .then_with(|| a.id.cmp(&b.id))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Calendar;
    use crate::policy::PolicyMatch;

    fn policy(id: &str, m: PolicyMatch) -> ServicePolicy {
        ServicePolicy {
            id: id.into(),
            name: id.into(),
            r#match: m,
            active: true,
            respond_h: 4,
            resolve_h: 24,
            calendar: Calendar::Always,
            party_window_h: 48,
            hold_down_days: 14,
        }
    }

    /// The list reads as the ladder: three axes, then two, then one, then the default — and two
    /// equally specific rows are ordered by id, never by whatever order the store handed back.
    #[test]
    fn the_order_is_the_resolution_ladder() {
        let mut ps = vec![
            policy("default", PolicyMatch::default()),
            policy(
                "z-site",
                PolicyMatch {
                    site: Some("s1".into()),
                    ..Default::default()
                },
            ),
            policy(
                "a-site",
                PolicyMatch {
                    site: Some("s2".into()),
                    ..Default::default()
                },
            ),
            policy(
                "all-three",
                PolicyMatch {
                    site: Some("s1".into()),
                    category: Some("c".into()),
                    severity: Some("critical".into()),
                },
            ),
        ];
        sort_by_specificity(&mut ps);
        let ids: Vec<&str> = ps.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["all-three", "a-site", "z-site", "default"]);
    }
}
