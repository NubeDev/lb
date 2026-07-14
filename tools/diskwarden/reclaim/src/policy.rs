//! The TOML policy: what may be cleaned, what cleans itself, and how often we look.
//!
//! ## The safety posture
//!
//! Two different questions, deliberately answered by two different rules:
//!
//! * **May I clean this when the user clicks "Clean now"?** → [`Policy::gate`].
//!   Requires the category to be on and the candidate to be cold (`min_age_days`)
//!   and worth it (`min_bytes`). A click is explicit intent, so the bar is just
//!   "don't destroy something they're using".
//!
//! * **Should this clean itself with nobody watching?** → [`Policy::auto_verdict`].
//!   Requires ALL of: the `auto_clean` master toggle (**off by default**), passing
//!   the same `gate`, AND exceeding `auto_clean_over_bytes`.
//!
//! ## Why auto-clean is size AND age, never size alone
//!
//! "Auto-clean any target over 10 GB" sounds right until you price it on a real box.
//! Here, `lb` is 142 GB and `cc-app` is 91 GB — both clear a 10 GB bar by an order of
//! magnitude, and both were built *today*. Cleaning them costs an hour of rebuilds
//! for disk you'd immediately refill.
//!
//! So the two knobs mean different things and both must hold:
//!   * `auto_clean_over_bytes` — is it **worth** reclaiming? (size)
//!   * `min_age_days`          — is it **safe** to reclaim? (age)
//!
//! Size alone is the rule that makes you uninstall the tool. Manual clean is always
//! available for anything that passes `gate`, which is your override for the rest.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::candidate::Candidate;

/// 10 GB — the default "big enough to bother auto-cleaning" line.
pub const DEFAULT_AUTO_CLEAN_OVER_BYTES: u64 = 10 * 1024 * 1024 * 1024;
/// 5 minutes.
pub const DEFAULT_SCAN_INTERVAL_SECS: u64 = 300;

/// The whole policy file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Policy {
    pub general: General,
    /// Keyed by `Reclaimer::id()`. Absent = category defaults.
    pub reclaimer: BTreeMap<String, ReclaimerPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct General {
    /// Where to search.
    pub roots: Vec<PathBuf>,
    /// How often the tray rescans.
    pub scan_interval_secs: u64,
    /// Master switch for cleaning WITHOUT a click. Off by default: the tool never
    /// deletes unattended until you say so.
    pub auto_clean: bool,
    /// Auto-clean only things bigger than this. Adjustable from the tray.
    /// Meaningless unless `auto_clean` is on.
    pub auto_clean_over_bytes: u64,
    /// Amber icon below this much free disk.
    pub warn_free_pct: u8,
    /// Red icon below this much free disk.
    pub critical_free_pct: u8,
}

impl Default for General {
    fn default() -> Self {
        Self {
            roots: vec![],
            scan_interval_secs: DEFAULT_SCAN_INTERVAL_SECS,
            auto_clean: false,
            auto_clean_over_bytes: DEFAULT_AUTO_CLEAN_OVER_BYTES,
            warn_free_pct: 15,
            critical_free_pct: 7,
        }
    }
}

/// Per-category rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ReclaimerPolicy {
    /// Is this category in play at all? Unticking it in the tray hides and protects
    /// the whole category. On by default — the categories exist because they're the
    /// ones worth reclaiming; `min_age_days` is what keeps it safe.
    pub enabled: bool,
    /// Never clean something touched more recently than this. THE safety floor.
    pub min_age_days: u64,
    /// Noise filter: don't bother listing something smaller than this.
    /// Not a trigger — see `General::auto_clean_over_bytes` for that.
    pub min_bytes: u64,
}

impl Default for ReclaimerPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            min_age_days: 30,
            min_bytes: 100 * 1024 * 1024,
        }
    }
}

/// Why a candidate may or may not be cleaned by a click. The tray shows the reason,
/// so a protected 142 GB dir explains itself instead of silently not appearing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum Gate {
    /// Safe to clean.
    Allowed,
    /// The category is switched off.
    NotEnabled,
    /// In use. Carries its age and the floor, both in days.
    TooRecent { age_days: u64, min_age_days: u64 },
    /// Below `min_bytes` — not worth listing.
    TooSmall { bytes: u64, min_bytes: u64 },
}

impl Gate {
    pub fn allowed(&self) -> bool {
        matches!(self, Gate::Allowed)
    }
}

/// Whether a candidate cleans itself unattended, and if not, why not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "auto", rename_all = "snake_case")]
pub enum AutoVerdict {
    /// Will be cleaned by the next scan with no click.
    Auto,
    /// The `auto_clean` master toggle is off.
    Disabled,
    /// Not cleanable at all right now (in use, category off, too small).
    Blocked { gate: Gate },
    /// Cleanable, but under the size trigger — waiting for a click.
    UnderThreshold { bytes: u64, over_bytes: u64 },
}

impl AutoVerdict {
    pub fn is_auto(&self) -> bool {
        matches!(self, AutoVerdict::Auto)
    }
}

impl Policy {
    fn for_kind(&self, kind: &str) -> ReclaimerPolicy {
        self.reclaimer.get(kind).cloned().unwrap_or_default()
    }

    /// May a click clean this? The ONE place that question is answered.
    pub fn gate(&self, candidate: &Candidate, now_secs: u64) -> Gate {
        let rp = self.for_kind(&candidate.kind);
        if !rp.enabled {
            return Gate::NotEnabled;
        }
        let age_days = candidate.age_days_at(now_secs);
        if age_days < rp.min_age_days {
            return Gate::TooRecent {
                age_days,
                min_age_days: rp.min_age_days,
            };
        }
        if candidate.bytes < rp.min_bytes {
            return Gate::TooSmall {
                bytes: candidate.bytes,
                min_bytes: rp.min_bytes,
            };
        }
        Gate::Allowed
    }

    /// Should this clean itself, with nobody watching?
    ///
    /// Deliberately built ON TOP of `gate` rather than beside it — auto-clean can
    /// only ever be a *narrowing* of what a click may do. It can never reach
    /// something a click couldn't.
    pub fn auto_verdict(&self, candidate: &Candidate, now_secs: u64) -> AutoVerdict {
        if !self.general.auto_clean {
            return AutoVerdict::Disabled;
        }
        let gate = self.gate(candidate, now_secs);
        if !gate.allowed() {
            return AutoVerdict::Blocked { gate };
        }
        let over = self.general.auto_clean_over_bytes;
        if candidate.bytes < over {
            return AutoVerdict::UnderThreshold {
                bytes: candidate.bytes,
                over_bytes: over,
            };
        }
        AutoVerdict::Auto
    }

    pub fn parse(toml_src: &str) -> anyhow::Result<Self> {
        Ok(toml::from_str(toml_src)?)
    }

    pub fn to_toml(&self) -> anyhow::Result<String> {
        Ok(toml::to_string_pretty(self)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_000 * 86_400;
    const GB: u64 = 1024 * 1024 * 1024;

    fn candidate(kind: &str, age_days: u64, bytes: u64) -> Candidate {
        Candidate {
            kind: kind.into(),
            path: "/tmp/p/target".into(),
            label: "p".into(),
            bytes,
            last_used_secs: NOW.saturating_sub(age_days * 86_400),
        }
    }

    fn auto_policy() -> Policy {
        Policy::parse(
            r#"
            [general]
            auto_clean = true
            auto_clean_over_bytes = 10737418240   # 10 GB
        "#,
        )
        .unwrap()
    }

    // ---- gate: what a click may do -------------------------------------------

    /// The rule that protects the 142 GB target of the repo you're building in today.
    #[test]
    fn a_recently_used_target_is_never_cleaned_however_huge() {
        let hot = candidate("cargo-target", 0, 142 * GB);
        assert_eq!(
            Policy::default().gate(&hot, NOW),
            Gate::TooRecent {
                age_days: 0,
                min_age_days: 30
            }
        );
    }

    #[test]
    fn a_cold_target_is_cleanable_by_default() {
        assert_eq!(
            Policy::default().gate(&candidate("cargo-target", 90, 20 * GB), NOW),
            Gate::Allowed
        );
    }

    #[test]
    fn unticking_a_category_protects_it() {
        let p = Policy::parse("[reclaimer.cargo-target]\nenabled = false\n").unwrap();
        assert_eq!(
            p.gate(&candidate("cargo-target", 999, 50 * GB), NOW),
            Gate::NotEnabled
        );
    }

    #[test]
    fn the_age_floor_is_inclusive() {
        let p = Policy::default();
        assert!(!p.gate(&candidate("cargo-target", 29, GB), NOW).allowed());
        assert!(p.gate(&candidate("cargo-target", 30, GB), NOW).allowed());
    }

    #[test]
    fn small_fry_is_not_listed() {
        assert!(matches!(
            Policy::default().gate(&candidate("cargo-target", 90, 10), NOW),
            Gate::TooSmall { .. }
        ));
    }

    // ---- auto_verdict: what happens with nobody watching ----------------------

    /// The headline: unattended deletion is off until you turn it on.
    #[test]
    fn auto_clean_is_off_by_default() {
        let ancient_and_huge = candidate("cargo-target", 999, 100 * GB);
        assert_eq!(
            Policy::default().auto_verdict(&ancient_and_huge, NOW),
            AutoVerdict::Disabled
        );
    }

    /// THE interlock. Both of this box's giants clear a 10 GB size bar by 10x and
    /// were built today. Size alone would auto-nuke 233 GB of hot build cache.
    #[test]
    fn auto_clean_never_touches_a_hot_target_however_far_over_the_size_trigger() {
        let p = auto_policy();
        for (label, bytes) in [("lb", 142 * GB), ("cc-app", 91 * GB)] {
            let hot = candidate("cargo-target", 0, bytes);
            assert!(
                matches!(
                    p.auto_verdict(&hot, NOW),
                    AutoVerdict::Blocked {
                        gate: Gate::TooRecent { .. }
                    }
                ),
                "{label} is over the trigger but in use — must not auto-clean"
            );
        }
    }

    #[test]
    fn auto_clean_fires_when_big_and_cold_and_enabled() {
        assert_eq!(
            auto_policy().auto_verdict(&candidate("cargo-target", 90, 20 * GB), NOW),
            AutoVerdict::Auto
        );
    }

    /// Cold but small: cleanable by a click, not worth doing unattended.
    #[test]
    fn a_cold_candidate_under_the_size_trigger_waits_for_a_click() {
        let small_and_cold = candidate("cargo-target", 90, 2 * GB);
        assert_eq!(
            auto_policy().auto_verdict(&small_and_cold, NOW),
            AutoVerdict::UnderThreshold {
                bytes: 2 * GB,
                over_bytes: 10 * GB
            }
        );
        assert_eq!(
            auto_policy().gate(&small_and_cold, NOW),
            Gate::Allowed,
            "a click can still clean it"
        );
    }

    #[test]
    fn the_size_trigger_is_adjustable() {
        let p = Policy::parse(
            "[general]\nauto_clean = true\nauto_clean_over_bytes = 1073741824\n", // 1 GB
        )
        .unwrap();
        assert_eq!(
            p.auto_verdict(&candidate("cargo-target", 90, 2 * GB), NOW),
            AutoVerdict::Auto,
            "2 GB now clears a 1 GB trigger"
        );
    }

    /// Structural: auto-clean must only ever narrow what a click may do.
    #[test]
    fn auto_clean_can_never_reach_what_a_click_cannot() {
        let p = auto_policy();
        for age in [0, 15, 29, 30, 90] {
            for bytes in [1, GB, 10 * GB, 50 * GB] {
                let c = candidate("cargo-target", age, bytes);
                if p.auto_verdict(&c, NOW).is_auto() {
                    assert!(
                        p.gate(&c, NOW).allowed(),
                        "auto-cleaned something a click couldn't: age={age} bytes={bytes}"
                    );
                }
            }
        }
    }

    // ---- the file itself ------------------------------------------------------

    #[test]
    fn defaults_are_five_minutes_and_ten_gb() {
        let g = Policy::default().general;
        assert_eq!(g.scan_interval_secs, 300);
        assert_eq!(g.auto_clean_over_bytes, 10 * GB);
        assert!(!g.auto_clean);
    }

    #[test]
    fn a_typo_in_the_policy_file_is_a_loud_error_not_a_silent_default() {
        // deny_unknown_fields: `auto_cleen = true` must NOT quietly parse as off.
        assert!(Policy::parse("[general]\nauto_cleen = true\n").is_err());
    }

    /// The tray writes this file back when you flip a toggle — it must round-trip.
    #[test]
    fn policy_round_trips_through_toml() {
        let mut p = Policy::default();
        p.general.auto_clean = true;
        p.general.auto_clean_over_bytes = 5 * GB;
        p.general.scan_interval_secs = 600;

        let back = Policy::parse(&p.to_toml().unwrap()).unwrap();
        assert!(back.general.auto_clean);
        assert_eq!(back.general.auto_clean_over_bytes, 5 * GB);
        assert_eq!(back.general.scan_interval_secs, 600);
    }
}
