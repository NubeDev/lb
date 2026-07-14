//! What the tray is currently showing: the last scan, the disk, and the icon colour.
//!
//! Kept pure and separate from `tray.rs` so the "what should the icon look like"
//! decision is testable without a D-Bus session or a desktop.

use diskwarden_reclaim::{Policy, ScanReport};

use crate::disk::Free;

/// The icon's colour. Maps to a freedesktop icon name in `tray.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    /// Nothing to do.
    Idle,
    /// There's something worth reclaiming.
    Reclaimable,
    /// Disk is getting low.
    Warn,
    /// Disk is nearly full.
    Critical,
}

/// Everything the tray renders from. One snapshot, swapped in wholesale after each
/// scan, so the menu can never show a half-updated mix of two scans.
#[derive(Debug, Clone)]
pub struct State {
    pub report: ScanReport,
    pub free: Option<Free>,
    pub policy: Policy,
    /// Set while a scan or clean is in flight, so the menu can say so.
    pub busy: bool,
    /// Last error worth surfacing in the menu (a failed clean, a bad policy).
    pub last_error: Option<String>,
}

impl State {
    /// The icon colour.
    ///
    /// Disk pressure outranks reclaimable space: if you're at 3% free, the icon must
    /// be red even when there's nothing we can clean, because that's precisely when
    /// you need to know. Amber-for-reclaimable is the "there's free money on the
    /// table" nudge, and it only matters while the disk is otherwise fine.
    pub fn health(&self) -> Health {
        if let Some(free) = self.free {
            let pct = free.free_pct();
            if pct <= self.policy.general.critical_free_pct {
                return Health::Critical;
            }
            if pct <= self.policy.general.warn_free_pct {
                return Health::Warn;
            }
        }
        if self.report.reclaimable_bytes() > 0 {
            return Health::Reclaimable;
        }
        Health::Idle
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diskwarden_reclaim::Candidate;
    use diskwarden_reclaim::{report::Finding, Gate};

    const GB: u64 = 1024 * 1024 * 1024;

    fn state(free_pct: u8, reclaimable_bytes: u64) -> State {
        let findings = if reclaimable_bytes > 0 {
            vec![Finding {
                candidate: Candidate {
                    kind: "cargo-target".into(),
                    path: "/p/target".into(),
                    label: "p".into(),
                    bytes: reclaimable_bytes,
                    last_used_secs: 0,
                },
                gate: Gate::Allowed,
                auto: diskwarden_reclaim::policy::AutoVerdict::Disabled,
            }]
        } else {
            vec![]
        };
        State {
            report: ScanReport {
                scanned_at_secs: 0,
                findings,
            },
            free: Some(Free {
                free_bytes: free_pct as u64,
                total_bytes: 100,
            }),
            policy: Policy::default(),
            busy: false,
            last_error: None,
        }
    }

    #[test]
    fn a_healthy_disk_with_nothing_to_clean_is_idle() {
        assert_eq!(state(50, 0).health(), Health::Idle);
    }

    #[test]
    fn a_healthy_disk_with_something_to_clean_nudges() {
        assert_eq!(state(50, 20 * GB).health(), Health::Reclaimable);
    }

    #[test]
    fn a_low_disk_warns() {
        assert_eq!(state(12, 0).health(), Health::Warn);
    }

    #[test]
    fn a_nearly_full_disk_is_critical() {
        assert_eq!(state(3, 0).health(), Health::Critical);
    }

    /// The case that matters most: nearly full AND nothing we can clean. The icon
    /// must still be red — that's exactly when you need the warning.
    #[test]
    fn disk_pressure_outranks_having_nothing_to_reclaim() {
        assert_eq!(state(3, 0).health(), Health::Critical);
        assert_eq!(state(3, 50 * GB).health(), Health::Critical);
    }

    #[test]
    fn the_thresholds_are_inclusive_and_adjustable() {
        let mut s = state(15, 0); // exactly at the default warn line
        assert_eq!(s.health(), Health::Warn);
        s.policy.general.warn_free_pct = 10;
        assert_eq!(
            s.health(),
            Health::Idle,
            "raising the bar clears the warning"
        );
    }

    /// If `df` failed we know nothing about pressure — don't invent a crisis.
    #[test]
    fn unknown_free_space_falls_back_to_reclaimable_only() {
        let mut s = state(3, 20 * GB);
        s.free = None;
        assert_eq!(s.health(), Health::Reclaimable);

        let mut s = state(3, 0);
        s.free = None;
        assert_eq!(s.health(), Health::Idle);
    }
}
