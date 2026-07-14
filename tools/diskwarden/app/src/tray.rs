//! The GNOME tray icon — the whole UI.
//!
//! Talks StatusNotifierItem over D-Bus via `ksni` (pure-Rust zbus underneath, so no
//! libdbus C dependency). This is the same protocol the wifi/volume icons use; on
//! GNOME it needs an AppIndicator shell extension, which this box already runs
//! (`zorin-appindicator`, ACTIVE).
//!
//! Icons are stock freedesktop names, so they inherit the system theme and match the
//! rest of the panel instead of shipping bitmaps that look wrong on a theme switch.

use std::sync::{Arc, Mutex};

use ksni::menu::{CheckmarkItem, MenuItem, StandardItem, SubMenu};

use crate::human;
use crate::state::{Health, State};

/// Size options offered for the auto-clean trigger, in GB.
const SIZE_CHOICES_GB: [u64; 6] = [1, 5, 10, 20, 50, 100];
/// Scan interval options, in minutes.
const INTERVAL_CHOICES_MIN: [u64; 5] = [1, 5, 15, 60, 240];

const GB: u64 = 1024 * 1024 * 1024;

/// What the tray asks the scan loop to do. The tray thread never scans or deletes
/// itself — it posts intent and returns immediately, so a click can never block the
/// panel while we walk 273 GB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Rescan,
    CleanNow,
    SetAutoClean(bool),
    SetAutoCleanOverBytes(u64),
    SetIntervalSecs(u64),
    Quit,
}

pub struct Tray {
    pub state: Arc<Mutex<State>>,
    pub tx: std::sync::mpsc::Sender<Command>,
}

impl Tray {
    fn send(&self, cmd: Command) {
        // The scan loop owning the receiver is gone only during shutdown; dropping
        // the command then is correct, not an error worth surfacing.
        let _ = self.tx.send(cmd);
    }
}

/// Stock freedesktop icon names, themed by the system.
fn icon_for(health: Health) -> &'static str {
    match health {
        Health::Idle => "drive-harddisk-symbolic",
        Health::Reclaimable => "user-trash-full-symbolic",
        Health::Warn => "dialog-warning-symbolic",
        Health::Critical => "dialog-error-symbolic",
    }
}

impl ksni::Tray for Tray {
    fn id(&self) -> String {
        "diskwarden".into()
    }

    fn icon_name(&self) -> String {
        let state = self.state.lock().unwrap();
        icon_for(state.health()).into()
    }

    /// The hover tooltip — the one-line answer without opening the menu.
    fn title(&self) -> String {
        let state = self.state.lock().unwrap();
        let r = state.report.reclaimable_bytes();
        if r == 0 {
            "diskwarden — nothing to clean".into()
        } else {
            format!("diskwarden — {} reclaimable", human::bytes(r))
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let state = self.state.lock().unwrap();
        let reclaimable = state.report.reclaimable_bytes();
        let interval_secs = state.policy.general.scan_interval_secs;
        let auto_clean = state.policy.general.auto_clean;
        let over_bytes = state.policy.general.auto_clean_over_bytes;

        let mut items: Vec<MenuItem<Self>> = Vec::new();

        // ---- status ---------------------------------------------------------
        if let Some(free) = state.free {
            items.push(
                StandardItem {
                    label: format!(
                        "Disk: {} free ({}%)",
                        human::bytes(free.free_bytes),
                        free.free_pct()
                    ),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
            );
        }
        items.push(
            StandardItem {
                label: if state.busy {
                    "Scanning…".into()
                } else {
                    format!("Reclaimable: {}", human::bytes(reclaimable))
                },
                enabled: false,
                ..Default::default()
            }
            .into(),
        );

        if let Some(err) = &state.last_error {
            items.push(
                StandardItem {
                    label: format!("⚠ {err}"),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
            );
        }

        // ---- the clean button ------------------------------------------------
        items.push(MenuItem::Separator);
        items.push(
            StandardItem {
                label: if reclaimable == 0 {
                    "Clean now".into()
                } else {
                    format!("Clean now ({})", human::bytes(reclaimable))
                },
                icon_name: "user-trash-symbolic".into(),
                // Nothing to clean, or already working: don't offer a no-op click.
                enabled: reclaimable > 0 && !state.busy,
                activate: Box::new(|t: &mut Self| t.send(Command::CleanNow)),
                ..Default::default()
            }
            .into(),
        );

        // ---- what would be cleaned -------------------------------------------
        let cleanable: Vec<_> = state.report.reclaimable().take(8).collect();
        if !cleanable.is_empty() {
            items.push(MenuItem::Separator);
            for f in cleanable {
                let auto_mark = if f.auto_cleanable() { " ⟳" } else { "" };
                items.push(
                    StandardItem {
                        label: format!(
                            "{}  {}  {}{}",
                            human::bytes(f.candidate.bytes),
                            human::age(f.candidate.age_days_at(state.report.scanned_at_secs)),
                            f.candidate.label,
                            auto_mark
                        ),
                        enabled: false,
                        ..Default::default()
                    }
                    .into(),
                );
            }
        }

        // ---- what is protected, and why --------------------------------------
        // The 142 GB you can see in `du` but we won't touch. Showing it (with the
        // reason) is what stops the tool looking broken or dishonest.
        let protected: Vec<_> = state.report.protected().take(5).collect();
        if !protected.is_empty() {
            items.push(MenuItem::Separator);
            items.push(
                StandardItem {
                    label: "Protected (in use)".into(),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
            );
            for f in protected {
                items.push(
                    StandardItem {
                        label: format!(
                            "{}  {}  {}",
                            human::bytes(f.candidate.bytes),
                            human::age(f.candidate.age_days_at(state.report.scanned_at_secs)),
                            f.candidate.label
                        ),
                        enabled: false,
                        ..Default::default()
                    }
                    .into(),
                );
            }
        }

        // ---- settings ---------------------------------------------------------
        items.push(MenuItem::Separator);
        items.push(
            CheckmarkItem {
                label: format!("Auto-clean over {}", human::bytes(over_bytes)),
                checked: auto_clean,
                activate: Box::new(move |t: &mut Self| t.send(Command::SetAutoClean(!auto_clean))),
                ..Default::default()
            }
            .into(),
        );
        items.push(
            SubMenu {
                label: "Auto-clean threshold".into(),
                submenu: SIZE_CHOICES_GB
                    .iter()
                    .map(|&gb| {
                        let bytes = gb * GB;
                        CheckmarkItem {
                            label: format!("{gb} GB"),
                            checked: over_bytes == bytes,
                            activate: Box::new(move |t: &mut Self| {
                                t.send(Command::SetAutoCleanOverBytes(bytes))
                            }),
                            ..Default::default()
                        }
                        .into()
                    })
                    .collect(),
                ..Default::default()
            }
            .into(),
        );
        items.push(
            SubMenu {
                label: format!("Scan every {}", human::interval(interval_secs)),
                submenu: INTERVAL_CHOICES_MIN
                    .iter()
                    .map(|&min| {
                        let secs = min * 60;
                        CheckmarkItem {
                            label: human::interval(secs),
                            checked: interval_secs == secs,
                            activate: Box::new(move |t: &mut Self| {
                                t.send(Command::SetIntervalSecs(secs))
                            }),
                            ..Default::default()
                        }
                        .into()
                    })
                    .collect(),
                ..Default::default()
            }
            .into(),
        );

        // ---- housekeeping -----------------------------------------------------
        items.push(MenuItem::Separator);
        items.push(
            StandardItem {
                label: "Scan now".into(),
                icon_name: "view-refresh-symbolic".into(),
                enabled: !state.busy,
                activate: Box::new(|t: &mut Self| t.send(Command::Rescan)),
                ..Default::default()
            }
            .into(),
        );
        items.push(
            StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit-symbolic".into(),
                activate: Box::new(|t: &mut Self| t.send(Command::Quit)),
                ..Default::default()
            }
            .into(),
        );

        items
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diskwarden_reclaim::policy::DEFAULT_AUTO_CLEAN_OVER_BYTES;

    #[test]
    fn each_health_maps_to_a_distinct_stock_icon() {
        let all = [
            Health::Idle,
            Health::Reclaimable,
            Health::Warn,
            Health::Critical,
        ];
        let names: Vec<_> = all.iter().map(|h| icon_for(*h)).collect();
        let unique: std::collections::BTreeSet<_> = names.iter().collect();
        assert_eq!(unique.len(), all.len(), "icons must be distinguishable");
        for n in names {
            assert!(n.ends_with("-symbolic"), "{n} should be a symbolic icon");
        }
    }

    /// The default threshold must be one of the values the menu offers, or the
    /// submenu shows nothing ticked on a fresh install.
    #[test]
    fn the_default_threshold_is_selectable_in_the_menu() {
        assert!(
            SIZE_CHOICES_GB
                .iter()
                .any(|&gb| gb * GB == DEFAULT_AUTO_CLEAN_OVER_BYTES),
            "10 GB default must appear in {SIZE_CHOICES_GB:?}"
        );
    }

    #[test]
    fn the_default_interval_is_selectable_in_the_menu() {
        let default = diskwarden_reclaim::policy::DEFAULT_SCAN_INTERVAL_SECS;
        assert!(
            INTERVAL_CHOICES_MIN.iter().any(|&m| m * 60 == default),
            "5 min default must appear in {INTERVAL_CHOICES_MIN:?}"
        );
    }
}
