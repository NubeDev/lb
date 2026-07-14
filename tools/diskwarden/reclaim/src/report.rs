//! The scan result: every candidate, each with the policy's verdict attached.
//!
//! This is the artifact the whole tool passes around — the cron timer writes it, the
//! tray colours its icon from it, the UI renders it, and `clean` acts on it. It is
//! plain serde JSON so those pieces stay decoupled: nothing re-scans to render.
//!
//! Note it reports *both* halves: `reclaimable_bytes` (what policy would free right
//! now) and `total_bytes` (everything found). Showing only the first would hide the
//! 84 GB you're protecting by using it today; showing only the second would imply we
//! are about to delete it. The tray shows the first and explains the gap.

use serde::{Deserialize, Serialize};

use crate::candidate::Candidate;
use crate::policy::{AutoVerdict, Gate, Policy};
use crate::reclaimer::ScanCtx;
use crate::reclaimers;

/// One candidate + both verdicts: what a click may do, and what happens unattended.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    #[serde(flatten)]
    pub candidate: Candidate,
    /// May "Clean now" free this?
    pub gate: Gate,
    /// Will it free itself?
    pub auto: AutoVerdict,
}

impl Finding {
    /// Cleanable by a click.
    pub fn reclaimable(&self) -> bool {
        self.gate.allowed()
    }

    /// Cleans itself on the next scan, no click.
    pub fn auto_cleanable(&self) -> bool {
        self.auto.is_auto()
    }
}

/// Everything one scan found.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanReport {
    /// When the scan ran, seconds since the unix epoch.
    pub scanned_at_secs: u64,
    /// Biggest first.
    pub findings: Vec<Finding>,
}

impl ScanReport {
    /// Run every reclaimer and gate the results. Read-only: this deletes nothing.
    ///
    /// A reclaimer that fails is skipped with its error returned alongside, rather
    /// than sinking the whole scan — one broken cleaner must not blind the others.
    pub fn scan(ctx: &ScanCtx, policy: &Policy) -> (Self, Vec<anyhow::Error>) {
        let mut findings = Vec::new();
        let mut errors = Vec::new();

        for r in reclaimers::all() {
            match r.scan(ctx) {
                Ok(candidates) => findings.extend(candidates.into_iter().map(|c| Finding {
                    gate: policy.gate(&c, ctx.now_secs),
                    auto: policy.auto_verdict(&c, ctx.now_secs),
                    candidate: c,
                })),
                Err(e) => errors.push(e.context(format!("reclaimer {} failed to scan", r.id()))),
            }
        }

        findings.sort_by(|a, b| b.candidate.bytes.cmp(&a.candidate.bytes));

        (
            Self {
                scanned_at_secs: ctx.now_secs,
                findings,
            },
            errors,
        )
    }

    /// Bytes the policy would free right now.
    pub fn reclaimable_bytes(&self) -> u64 {
        self.findings
            .iter()
            .filter(|f| f.reclaimable())
            .map(|f| f.candidate.bytes)
            .sum()
    }

    /// Bytes found, regardless of policy.
    pub fn total_bytes(&self) -> u64 {
        self.findings.iter().map(|f| f.candidate.bytes).sum()
    }

    /// Bytes that will free themselves on this scan, with no click.
    pub fn auto_clean_bytes(&self) -> u64 {
        self.auto_cleanable().map(|f| f.candidate.bytes).sum()
    }

    /// Just the findings that may be freed, biggest first.
    pub fn reclaimable(&self) -> impl Iterator<Item = &Finding> {
        self.findings.iter().filter(|f| f.reclaimable())
    }

    /// Just the findings that free themselves, biggest first.
    pub fn auto_cleanable(&self) -> impl Iterator<Item = &Finding> {
        self.findings.iter().filter(|f| f.auto_cleanable())
    }

    /// Found, but protected — what the tray lists under "Protected". Biggest first.
    pub fn protected(&self) -> impl Iterator<Item = &Finding> {
        self.findings.iter().filter(|f| !f.reclaimable())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn project(root: &Path, name: &str, bytes: usize) {
        let target = root.join(name).join("target");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(root.join(name).join("Cargo.toml"), "[package]\nname='x'\n").unwrap();
        std::fs::write(target.join("artifact.rlib"), vec![0u8; bytes]).unwrap();
    }

    /// End-to-end on a real tree: scan finds it, and the DEFAULT policy frees nothing.
    #[test]
    fn a_default_policy_scan_finds_everything_and_reclaims_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        project(tmp.path(), "proj", 4096);

        let ctx = ScanCtx {
            roots: vec![tmp.path().to_path_buf()],
            now_secs: crate::size::now_secs(),
        };
        let (report, errors) = ScanReport::scan(&ctx, &Policy::default());

        assert!(errors.is_empty());
        assert_eq!(report.total_bytes(), 4096, "it is found");
        assert_eq!(report.reclaimable_bytes(), 0, "and it is not deletable");
        assert_eq!(report.reclaimable().count(), 0);
    }

    /// The gap between "found" and "reclaimable" is the whole UX: a freshly-used
    /// target is reported but protected.
    #[test]
    fn a_hot_target_counts_toward_total_but_not_reclaimable() {
        let tmp = tempfile::tempdir().unwrap();
        project(tmp.path(), "hot", 4096);

        let ctx = ScanCtx {
            roots: vec![tmp.path().to_path_buf()],
            now_secs: crate::size::now_secs(),
        };
        let policy = Policy::parse(
            "[reclaimer.cargo-target]\nenabled = true\nmin_age_days = 30\nmin_bytes = 1\n",
        )
        .unwrap();
        let (report, _) = ScanReport::scan(&ctx, &policy);

        assert_eq!(report.total_bytes(), 4096);
        assert_eq!(
            report.reclaimable_bytes(),
            0,
            "just-built target is protected"
        );
        assert!(matches!(
            report.findings[0].gate,
            Gate::TooRecent {
                min_age_days: 30,
                ..
            }
        ));
    }

    /// Same tree, but the clock has moved a year on — now it is reclaimable.
    #[test]
    fn an_aged_out_target_becomes_reclaimable() {
        let tmp = tempfile::tempdir().unwrap();
        project(tmp.path(), "cold", 4096);

        let ctx = ScanCtx {
            roots: vec![tmp.path().to_path_buf()],
            now_secs: crate::size::now_secs() + 365 * 86_400,
        };
        let policy = Policy::parse(
            "[reclaimer.cargo-target]\nenabled = true\nmin_age_days = 30\nmin_bytes = 1\n",
        )
        .unwrap();
        let (report, _) = ScanReport::scan(&ctx, &policy);

        assert_eq!(report.reclaimable_bytes(), 4096);
    }

    #[test]
    fn report_round_trips_as_json_for_the_state_file() {
        let tmp = tempfile::tempdir().unwrap();
        project(tmp.path(), "proj", 2048);
        let ctx = ScanCtx {
            roots: vec![tmp.path().to_path_buf()],
            now_secs: crate::size::now_secs(),
        };
        let (report, _) = ScanReport::scan(&ctx, &Policy::default());

        let json = serde_json::to_string(&report).unwrap();
        let back: ScanReport = serde_json::from_str(&json).unwrap();

        assert_eq!(back.total_bytes(), report.total_bytes());
        assert_eq!(back.findings[0].candidate.label, "proj");
    }
}
