//! `match_policy` — which [`ServicePolicy`] governs a case (case-plane scope).
//!
//! The sla-clock reactor calls this once per case open and once per severity escalation, then feeds
//! the answer to [`crate::deadline`].
//!
//! # The rule: most specific wins
//!
//! Each policy is scored against the query by how many of its three `match` axes are pinned **and**
//! equal the query. A policy with a pinned axis that does **not** equal the query is **disqualified
//! entirely** — it is a statement about other work, and a near-miss must never be treated as a
//! partial fit. The highest surviving score wins; the workspace default (empty match, score 0)
//! catches everything else. Ties are broken by `id` ascending, the same order
//! [`crate::policy_list`] renders, so the settings page and the reactor always agree.
//!
//! A `None` axis on the *query* means "the case does not have this fact". It matches only a policy
//! that leaves that axis open: a policy pinned to `site: "s1"` cannot govern a case with no site,
//! because we do not know that it belongs there.
//!
//! # Rule 10
//!
//! `site`, `category` and `severity` are opaque strings compared with `==`. This function has no
//! vocabulary, no ordering over severities, and no knowledge of any pack, rule or extension.

use lb_store::Store;

use crate::error::CasesError;
use crate::policy::ServicePolicy;
use crate::policy_list::policy_list;

/// Resolve the policy governing a case with these facts, or `None` if the workspace has no policy
/// that fits (not even a default).
pub async fn match_policy(
    store: &Store,
    ws: &str,
    site: Option<&str>,
    category: Option<&str>,
    severity: Option<&str>,
) -> Result<Option<ServicePolicy>, CasesError> {
    // `false` — a RETIRED policy governs no new case. That is the whole meaning of the flag, and it
    // has to be enforced here rather than only in the settings list: the ladder is what decides a
    // deadline, so a disabled row that still matched would be disabled in name only.
    let policies = policy_list(store, ws, false).await?;
    Ok(best_match(&policies, site, category, severity).cloned())
}

/// The pure half of the resolution — the ladder itself, with no store. Exposed so the rule can be
/// tested and reasoned about without booting anything.
pub fn best_match<'a>(
    policies: &'a [ServicePolicy],
    site: Option<&str>,
    category: Option<&str>,
    severity: Option<&str>,
) -> Option<&'a ServicePolicy> {
    let mut best: Option<(u8, &ServicePolicy)> = None;
    for policy in policies {
        let Some(score) = score(policy, site, category, severity) else {
            continue; // a pinned axis contradicts the query — disqualified outright
        };
        let better = match best {
            None => true,
            // Ties broken by id ascending: deterministic, and the same order `policy_list` shows.
            Some((best_score, best_policy)) => {
                score > best_score || (score == best_score && policy.id < best_policy.id)
            }
        };
        if better {
            best = Some((score, policy));
        }
    }
    best.map(|(_, p)| p)
}

/// How well `policy` fits the query: `Some(n)` where `n` is the number of axes it pins and matches,
/// or `None` if any pinned axis contradicts the query.
fn score(
    policy: &ServicePolicy,
    site: Option<&str>,
    category: Option<&str>,
    severity: Option<&str>,
) -> Option<u8> {
    let m = &policy.r#match;
    Some(
        axis(m.site.as_deref(), site)?
            + axis(m.category.as_deref(), category)?
            + axis(m.severity.as_deref(), severity)?,
    )
}

/// One axis: an unpinned policy axis scores 0 and always fits; a pinned one scores 1 if it equals
/// the query and disqualifies the policy otherwise (including when the case has no such fact).
fn axis(pinned: Option<&str>, query: Option<&str>) -> Option<u8> {
    match pinned {
        None => Some(0),
        Some(want) if query == Some(want) => Some(1),
        Some(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Calendar;
    use crate::policy::PolicyMatch;

    fn policy(
        id: &str,
        site: Option<&str>,
        category: Option<&str>,
        severity: Option<&str>,
    ) -> ServicePolicy {
        ServicePolicy {
            id: id.into(),
            name: id.into(),
            r#match: PolicyMatch {
                site: site.map(str::to_string),
                category: category.map(str::to_string),
                severity: severity.map(str::to_string),
            },
            active: true,
            respond_h: 4,
            resolve_h: 24,
            calendar: Calendar::Always,
            party_window_h: 48,
            hold_down_days: 14,
        }
    }

    fn pick<'a>(
        ps: &'a [ServicePolicy],
        site: Option<&str>,
        cat: Option<&str>,
        sev: Option<&str>,
    ) -> Option<&'a str> {
        best_match(ps, site, cat, sev).map(|p| p.id.as_str())
    }

    /// Rung 1 — the workspace default alone catches everything, whatever the case's facts.
    #[test]
    fn the_default_alone_catches_everything() {
        let ps = [policy("default", None, None, None)];
        assert_eq!(
            pick(&ps, Some("s1"), Some("device_health"), Some("critical")),
            Some("default")
        );
        assert_eq!(pick(&ps, None, None, None), Some("default"));
    }

    /// Rung 2 — a category policy beats the default for its category, and the default still catches
    /// every other category.
    #[test]
    fn a_category_policy_beats_the_default_for_its_category() {
        let ps = [
            policy("default", None, None, None),
            policy("dq", None, Some("data_quality"), None),
        ];
        assert_eq!(
            pick(&ps, Some("s1"), Some("data_quality"), None),
            Some("dq")
        );
        assert_eq!(
            pick(&ps, Some("s1"), Some("device_health"), None),
            Some("default")
        );
        // The case has no category at all: the pinned policy cannot claim it.
        assert_eq!(pick(&ps, Some("s1"), None, None), Some("default"));
    }

    /// Rung 3 — a site override outranks the category policy for that site, and only that site.
    #[test]
    fn a_site_override_outranks_the_category_policy() {
        let ps = [
            policy("default", None, None, None),
            policy("dq", None, Some("data_quality"), None),
            policy("s1-dq", Some("s1"), Some("data_quality"), None),
        ];
        assert_eq!(
            pick(&ps, Some("s1"), Some("data_quality"), None),
            Some("s1-dq")
        );
        assert_eq!(
            pick(&ps, Some("s2"), Some("data_quality"), None),
            Some("dq")
        );
        assert_eq!(
            pick(&ps, Some("s1"), Some("device_health"), None),
            Some("default")
        );
    }

    /// **The disqualification rule.** A policy whose site does not match is never chosen, even
    /// though its category matches and it is more specific than the default. A near miss is a
    /// statement about *other* work, not a partial fit — if this scored 1 for the category, the
    /// wrong site's contract would silently govern the case.
    #[test]
    fn a_wrong_site_is_never_chosen_even_when_its_category_fits() {
        let ps = [
            policy("default", None, None, None),
            policy("s9-dq", Some("s9"), Some("data_quality"), None),
        ];
        assert_eq!(
            pick(&ps, Some("s1"), Some("data_quality"), None),
            Some("default")
        );
        assert_eq!(
            pick(&ps, Some("s9"), Some("data_quality"), None),
            Some("s9-dq")
        );
    }

    /// All three axes is the top rung, and it too is disqualified by a single contradicting axis.
    #[test]
    fn three_axes_is_the_top_rung() {
        let ps = [
            policy("default", None, None, None),
            policy("s1", Some("s1"), None, None),
            policy("s1-dq", Some("s1"), Some("data_quality"), None),
            policy(
                "s1-dq-crit",
                Some("s1"),
                Some("data_quality"),
                Some("critical"),
            ),
        ];
        assert_eq!(
            pick(&ps, Some("s1"), Some("data_quality"), Some("critical")),
            Some("s1-dq-crit")
        );
        assert_eq!(
            pick(&ps, Some("s1"), Some("data_quality"), Some("warning")),
            Some("s1-dq")
        );
        assert_eq!(
            pick(&ps, Some("s1"), Some("optimisation"), Some("critical")),
            Some("s1")
        );
        assert_eq!(
            pick(&ps, Some("s2"), Some("data_quality"), Some("critical")),
            Some("default")
        );
    }

    /// With no default row, a case that fits nothing gets **nothing** — the reactor must be able to
    /// tell "no SLA applies" from "the 24x7 default applies", so this is `None`, not a fallback.
    #[test]
    fn no_fitting_policy_resolves_to_none() {
        let ps = [policy("s9", Some("s9"), None, None)];
        assert_eq!(pick(&ps, Some("s1"), None, None), None);
        assert!(best_match(&[], Some("s1"), None, None).is_none());
    }

    /// Two equally specific policies tie-break by `id` ascending — deterministically, and in the
    /// same order `policy_list` renders them, whatever order the store returned.
    #[test]
    fn ties_break_by_id_ascending() {
        let ps = [
            policy("zeta", None, Some("dq"), None),
            policy("alpha", None, Some("dq"), None),
        ];
        assert_eq!(pick(&ps, None, Some("dq"), None), Some("alpha"));
        let reversed = [ps[1].clone(), ps[0].clone()];
        assert_eq!(pick(&reversed, None, Some("dq"), None), Some("alpha"));
    }
}
