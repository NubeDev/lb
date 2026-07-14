//! The engine behind the tray: rescan on a timer, act on menu commands.
//!
//! Runs on its own thread so a 3-second walk of 273 GB never blocks the panel. The
//! tray posts [`Command`]s and returns instantly; this loop does the slow work and
//! swaps a fresh [`State`] in when it's done.
//!
//! Auto-clean happens HERE, right after each scan, and only for findings the policy
//! marked `AutoVerdict::Auto` — which already required the toggle, the size trigger,
//! and the age floor. This loop adds no rules of its own; it just obeys the verdict.

use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use diskwarden_reclaim::{reclaimers, Policy, ScanCtx, ScanReport};

use crate::disk;
use crate::human;
use crate::paths;
use crate::state::State;
use crate::tray::Command;

/// Owns the slow work. `redraw` pokes the tray to re-render after state changes.
pub struct ScanLoop {
    pub state: Arc<Mutex<State>>,
    pub rx: Receiver<Command>,
    pub redraw: Box<dyn Fn() + Send>,
}

impl ScanLoop {
    /// Block forever: scan on the interval, handle commands as they arrive.
    /// Returns when the user picks Quit (or the tray is gone).
    pub fn run(self) {
        self.scan_and_maybe_autoclean();

        loop {
            let interval = {
                let s = self.state.lock().unwrap();
                Duration::from_secs(s.policy.general.scan_interval_secs.max(60))
            };

            // One wait serves both jobs: it wakes early for a click, and times out to
            // rescan. So changing the interval takes effect on the next tick without
            // a separate timer to keep in sync.
            match self.rx.recv_timeout(interval) {
                Ok(Command::Quit) | Err(RecvTimeoutError::Disconnected) => return,
                Ok(cmd) => self.handle(cmd),
                Err(RecvTimeoutError::Timeout) => self.scan_and_maybe_autoclean(),
            }
        }
    }

    fn handle(&self, cmd: Command) {
        match cmd {
            Command::Quit => {}
            Command::Rescan => self.scan_and_maybe_autoclean(),
            Command::CleanNow => {
                self.clean_now();
                self.scan_and_maybe_autoclean();
            }
            Command::SetAutoClean(on) => self.edit_policy(|p| p.general.auto_clean = on),
            Command::SetAutoCleanOverBytes(b) => {
                self.edit_policy(|p| p.general.auto_clean_over_bytes = b)
            }
            Command::SetIntervalSecs(s) => self.edit_policy(|p| p.general.scan_interval_secs = s),
        }
    }

    /// Change a setting, persist it, and re-gate the CURRENT findings against it.
    ///
    /// The rescan matters: flipping "auto-clean" or dropping the threshold changes
    /// which rows are green, and the menu must reflect that on the next open — not in
    /// five minutes. Rescanning also means a threshold change can immediately
    /// auto-clean, which is the behaviour you'd expect from ticking the box.
    fn edit_policy(&self, edit: impl FnOnce(&mut Policy)) {
        {
            let mut s = self.state.lock().unwrap();
            edit(&mut s.policy);
            let policy = s.policy.clone();
            drop(s);
            if let Err(e) = paths::save_policy(&policy) {
                self.set_error(format!("couldn't save settings: {e}"));
            }
        }
        self.scan_and_maybe_autoclean();
    }

    /// Clean everything a click is allowed to clean.
    fn clean_now(&self) {
        let targets: Vec<_> = {
            let s = self.state.lock().unwrap();
            s.report
                .reclaimable()
                .map(|f| f.candidate.clone())
                .collect()
        };
        self.reclaim_all(&targets, "clean");
    }

    /// Clean only what the policy marked as self-cleaning.
    fn auto_clean(&self) {
        let targets: Vec<_> = {
            let s = self.state.lock().unwrap();
            s.report
                .auto_cleanable()
                .map(|f| f.candidate.clone())
                .collect()
        };
        if targets.is_empty() {
            return;
        }
        self.reclaim_all(&targets, "auto-clean");
    }

    /// Hand each candidate back to the reclaimer that produced it.
    ///
    /// One failure doesn't stop the rest: a target held open by a running build
    /// should not prevent the other 40 GB from being freed. The last error is
    /// surfaced in the menu rather than swallowed.
    fn reclaim_all(&self, targets: &[diskwarden_reclaim::Candidate], what: &str) {
        self.set_busy(true);

        let all = reclaimers::all();
        let mut freed = 0u64;
        let mut last_err = None;

        for c in targets {
            let Some(r) = all.iter().find(|r| r.id() == c.kind) else {
                last_err = Some(format!("no reclaimer for {:?}", c.kind));
                continue;
            };
            match r.reclaim(c) {
                Ok(n) => freed += n,
                Err(e) => last_err = Some(format!("{}: {e}", c.label)),
            }
        }

        eprintln!("{what}: freed {}", human::bytes(freed));
        self.set_busy(false);
        if let Some(e) = last_err {
            self.set_error(e);
        }
    }

    fn scan_and_maybe_autoclean(&self) {
        self.rescan();
        self.auto_clean();
        // Auto-clean deleted things, so the report we just built is stale. Re-scan so
        // the menu isn't offering to clean what's already gone.
        self.rescan();
    }

    fn rescan(&self) {
        self.set_busy(true);

        let policy = {
            let s = self.state.lock().unwrap();
            s.policy.clone()
        };
        let ctx = ScanCtx {
            roots: policy.general.roots.clone(),
            now_secs: diskwarden_reclaim::size::now_secs(),
        };

        let (report, errors) = ScanReport::scan(&ctx, &policy);
        let free = policy
            .general
            .roots
            .first()
            .and_then(|r| disk::free_for(r).ok());

        {
            let mut s = self.state.lock().unwrap();
            s.report = report;
            s.free = free;
            s.busy = false;
            if let Some(e) = errors.first() {
                s.last_error = Some(format!("{e:#}"));
            }
        }
        (self.redraw)();
    }

    fn set_busy(&self, busy: bool) {
        self.state.lock().unwrap().busy = busy;
        (self.redraw)();
    }

    fn set_error(&self, err: String) {
        eprintln!("error: {err}");
        self.state.lock().unwrap().last_error = Some(err);
        (self.redraw)();
    }
}

/// Convenience for `main`: the channel the tray posts commands on.
pub fn channel() -> (Sender<Command>, Receiver<Command>) {
    std::sync::mpsc::channel()
}
