//! The **loop detector** (agent-loop-hardening slice B): a sliding window over
//! `(tool, hash(args), hash(result))` triples that catches an agent going in circles —
//!
//!   - **exact repeat**: 3+ consecutive identical calls with identical results;
//!   - **ping-pong**: an A→B→A→B alternation sustained for 4+ cycles;
//!   - **no-progress**: the same tool returning the identical result hash 5+ times *across the
//!     window* under different args (so interleaving other calls can't evade it — the classic
//!     "retry the query with slightly different SQL, same empty result" spiral).
//!
//! The escalation ladder is strike-based: the first firing **warns** (a corrective message the
//! model sees — legitimate polling can say "I am intentionally polling" and proceed), a second
//! consecutive firing **blocks** the matching calls (error-as-observation), a third **breaks** the
//! run with an honest terminal event. Any turn where no rule fires — a genuinely new result —
//! resets the ladder. Hash-based and free on the hot path (the rejected alternative was an
//! LLM-based "is it stuck" classifier — cost and latency for a heuristic signal).
//!
//! Hashing is FNV-1a, deterministic across processes (std's `DefaultHasher` seeds per-instance).
//! In-house runtime only; the wall-level variant for external runtimes waits for the capability
//! wall (scope: two runtimes, one wall).

use std::collections::VecDeque;

use super::model_access::{CallOutcome, ProposedCall};

/// The node-default sliding-window size (zeroclaw's default; revisit with real transcripts).
/// Workspace-tunable via `agent.config.loop_window`; `0` disables the detector.
pub const DEFAULT_LOOP_WINDOW: u32 = 20;

const EXACT_REPEAT: usize = 3;
const PING_PONG_CYCLES: usize = 4;
const NO_PROGRESS: usize = 5;

/// The corrective message the warn rung injects (live context only, never persisted).
pub const LOOP_WARNING: &str = "[loop detector] you appear to be repeating the same tool call \
    without progress. If this is intentional (e.g. polling), say so and continue deliberately; \
    otherwise change your approach — further identical calls will be blocked.";

/// The error a blocked call is fed as its observation.
pub const LOOP_BLOCKED: &str =
    "blocked by the loop detector: this call repeats a pattern that is making no progress — \
     change your approach";

/// What the loop should do after this turn's observation.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum LoopSignal {
    /// Inject [`LOOP_WARNING`] into the live context.
    Warn,
    /// Keep going; matching calls will be refused at dispatch ([`LoopDetector::should_block`]).
    Block,
    /// End the run with an honest terminal event.
    Break,
}

#[derive(Clone, PartialEq, Eq)]
struct Sig {
    tool: String,
    args: u64,
    result: u64,
}

/// What fired, so the block rung refuses only the *matching* calls: the tool, and — for an exact
/// repeat — the specific args hash (a no-progress spiral varies args, so it matches on tool alone).
struct Flag {
    tool: String,
    args: Option<u64>,
}

pub(super) struct LoopDetector {
    window: VecDeque<Sig>,
    cap: usize,
    strikes: u8,
    flagged: Option<Flag>,
}

impl LoopDetector {
    /// `window == 0` disables the detector entirely (`None`).
    pub(super) fn new(window: u32) -> Option<Self> {
        (window > 0).then(|| Self {
            window: VecDeque::new(),
            cap: window as usize,
            strikes: 0,
            flagged: None,
        })
    }

    /// Feed one turn's outcomes; returns the ladder rung when a repetition rule fires, `None` (and
    /// a ladder reset) when this turn produced a genuinely new pattern.
    pub(super) fn observe(&mut self, outcomes: &[CallOutcome]) -> Option<LoopSignal> {
        // Blocked calls are OUR OWN refusals, not the model's progress: they never enter the
        // window (their constant error would fake a pattern), and a turn that did NOTHING but
        // repeat blocked calls means the block rung was ignored — escalate directly (this is what
        // carries block → break instead of the blocked-error "novelty" resetting the ladder).
        let real: Vec<&CallOutcome> = outcomes
            .iter()
            .filter(|o| o.error.as_deref() != Some(LOOP_BLOCKED))
            .collect();
        if real.is_empty() && !outcomes.is_empty() {
            self.strikes = self.strikes.saturating_add(1);
            return Some(self.rung());
        }

        for o in &real {
            let result = o.ok.as_deref().or(o.error.as_deref()).unwrap_or("");
            self.window.push_back(Sig {
                tool: o.name.clone(),
                args: fnv1a(&o.input),
                result: fnv1a(result),
            });
            while self.window.len() > self.cap {
                self.window.pop_front();
            }
        }
        if real.is_empty() {
            return None;
        }

        let fired = self
            .exact_repeat()
            .or_else(|| self.ping_pong())
            .or_else(|| self.no_progress());

        match fired {
            Some(flag) => {
                self.strikes = self.strikes.saturating_add(1);
                self.flagged = Some(flag);
                Some(self.rung())
            }
            None => {
                // A genuinely new result — the ladder resets (scope: the escalation ladder resets
                // on a genuinely new result hash).
                self.strikes = 0;
                self.flagged = None;
                None
            }
        }
    }

    /// The rung the current strike count maps to.
    fn rung(&self) -> LoopSignal {
        match self.strikes {
            1 => LoopSignal::Warn,
            2 => LoopSignal::Block,
            _ => LoopSignal::Break,
        }
    }

    /// Whether `call` should be refused at dispatch — only meaningful on the Block rung, and only
    /// for calls matching the fired pattern.
    pub(super) fn should_block(&self, call: &ProposedCall) -> bool {
        if self.strikes < 2 {
            return false;
        }
        match &self.flagged {
            Some(flag) => {
                flag.tool == call.name
                    && flag.args.map(|a| a == fnv1a(&call.input)).unwrap_or(true)
            }
            None => false,
        }
    }

    /// 3+ consecutive identical triples at the tail.
    fn exact_repeat(&self) -> Option<Flag> {
        if self.window.len() < EXACT_REPEAT {
            return None;
        }
        let tail: Vec<&Sig> = self.window.iter().rev().take(EXACT_REPEAT).collect();
        let first = tail[0];
        tail.iter()
            .all(|s| *s == first)
            .then(|| Flag {
                tool: first.tool.clone(),
                args: Some(first.args),
            })
    }

    /// A,B,A,B… alternation (A ≠ B) sustained for `PING_PONG_CYCLES` cycles (2× that in calls).
    fn ping_pong(&self) -> Option<Flag> {
        let need = PING_PONG_CYCLES * 2;
        if self.window.len() < need {
            return None;
        }
        let tail: Vec<&Sig> = self.window.iter().rev().take(need).collect();
        let (a, b) = (tail[0], tail[1]);
        if a == b {
            return None;
        }
        tail.iter()
            .enumerate()
            .all(|(i, s)| if i % 2 == 0 { *s == a } else { *s == b })
            .then(|| Flag {
                tool: a.tool.clone(),
                args: Some(a.args),
            })
    }

    /// Same tool + identical result hash, ≥ `NO_PROGRESS` times anywhere in the window, under 2+
    /// distinct args — interleaved other calls can't evade it; args must vary (the identical-args
    /// case is the exact-repeat rule).
    fn no_progress(&self) -> Option<Flag> {
        for probe in self.window.iter().rev() {
            let matching: Vec<&Sig> = self
                .window
                .iter()
                .filter(|s| s.tool == probe.tool && s.result == probe.result)
                .collect();
            if matching.len() >= NO_PROGRESS {
                let mut args: Vec<u64> = matching.iter().map(|s| s.args).collect();
                args.sort_unstable();
                args.dedup();
                if args.len() >= 2 {
                    // Block on the tool alone — the spiral varies its args.
                    return Some(Flag {
                        tool: probe.tool.clone(),
                        args: None,
                    });
                }
            }
        }
        None
    }
}

/// FNV-1a 64-bit — deterministic across processes and runs (unlike std's seeded `DefaultHasher`),
/// cheap, and collision-safe enough for a heuristic window.
fn fnv1a(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(tool: &str, args: &str, result: &str) -> CallOutcome {
        CallOutcome {
            id: "c".into(),
            name: tool.into(),
            input: args.into(),
            ok: Some(result.into()),
            error: None,
        }
    }

    fn call(tool: &str, args: &str) -> ProposedCall {
        ProposedCall {
            id: "p".into(),
            name: tool.into(),
            input: args.into(),
        }
    }

    #[test]
    fn exact_repeat_fires_on_the_third_identical_call_and_climbs_the_ladder() {
        let mut d = LoopDetector::new(DEFAULT_LOOP_WINDOW).unwrap();
        assert_eq!(d.observe(&[outcome("q.run", "{a}", "empty")]), None);
        assert_eq!(d.observe(&[outcome("q.run", "{a}", "empty")]), None);
        assert_eq!(
            d.observe(&[outcome("q.run", "{a}", "empty")]),
            Some(LoopSignal::Warn)
        );
        // The warn rung does not block yet.
        assert!(!d.should_block(&call("q.run", "{a}")));
        assert_eq!(
            d.observe(&[outcome("q.run", "{a}", "empty")]),
            Some(LoopSignal::Block)
        );
        // Block matches the exact fired pattern only — a different-args call still dispatches.
        assert!(d.should_block(&call("q.run", "{a}")));
        assert!(!d.should_block(&call("q.run", "{b}")));
        assert!(!d.should_block(&call("other.tool", "{a}")));
        assert_eq!(
            d.observe(&[outcome("q.run", "{a}", "empty")]),
            Some(LoopSignal::Break)
        );
    }

    #[test]
    fn a_genuinely_new_result_resets_the_ladder() {
        let mut d = LoopDetector::new(DEFAULT_LOOP_WINDOW).unwrap();
        for _ in 0..2 {
            d.observe(&[outcome("q.run", "{a}", "empty")]);
        }
        assert_eq!(
            d.observe(&[outcome("q.run", "{a}", "empty")]),
            Some(LoopSignal::Warn)
        );
        // Progress! A different result breaks the pattern and resets strikes.
        assert_eq!(d.observe(&[outcome("q.run", "{a}", "42 rows")]), None);
        // The next firing (a fresh spiral on a DIFFERENT tool, so the old window entries can't
        // combine into a no-progress hit) starts back at Warn, not Block.
        for _ in 0..2 {
            d.observe(&[outcome("r.run", "{b}", "empty")]);
        }
        assert_eq!(
            d.observe(&[outcome("r.run", "{b}", "empty")]),
            Some(LoopSignal::Warn)
        );
    }

    #[test]
    fn ping_pong_fires_after_four_cycles() {
        let mut d = LoopDetector::new(DEFAULT_LOOP_WINDOW).unwrap();
        let a = || outcome("read.x", "{1}", "v1");
        let b = || outcome("read.y", "{2}", "v2");
        let mut fired = None;
        for i in 0..8 {
            let sig = if i % 2 == 0 { a() } else { b() };
            fired = d.observe(&[sig]);
        }
        assert_eq!(fired, Some(LoopSignal::Warn), "A,B x4 must fire");
    }

    #[test]
    fn interleaved_no_progress_cannot_be_evaded() {
        // Same tool, DIFFERENT args, identical empty result — interleaved with unrelated calls.
        let mut d = LoopDetector::new(DEFAULT_LOOP_WINDOW).unwrap();
        let mut fired = None;
        for i in 0..5 {
            fired = d.observe(&[
                outcome("q.run", &format!("{{sql-{i}}}"), "empty"),
                outcome("other.tool", &format!("{{x-{i}}}"), &format!("v{i}")),
            ]);
        }
        assert_eq!(fired, Some(LoopSignal::Warn), "interleaving must not evade the window");
        // Its block flag matches the tool regardless of args (the spiral varies them).
        d.observe(&[outcome("q.run", "{sql-9}", "empty")]);
        assert!(d.should_block(&call("q.run", "{anything}")));
        assert!(!d.should_block(&call("other.tool", "{anything}")));
    }

    #[test]
    fn a_blocked_only_turn_escalates_instead_of_resetting() {
        let mut d = LoopDetector::new(DEFAULT_LOOP_WINDOW).unwrap();
        for _ in 0..3 {
            d.observe(&[outcome("q.run", "{a}", "empty")]);
        }
        assert_eq!(
            d.observe(&[outcome("q.run", "{a}", "empty")]),
            Some(LoopSignal::Block)
        );
        // The model repeats; the loop blocks it; the blocked-only turn must escalate to Break —
        // never read our own refusal as "a new result" and reset.
        let blocked = CallOutcome {
            id: "c".into(),
            name: "q.run".into(),
            input: "{a}".into(),
            ok: None,
            error: Some(LOOP_BLOCKED.to_string()),
        };
        assert_eq!(d.observe(&[blocked]), Some(LoopSignal::Break));
    }

    #[test]
    fn window_zero_disables_the_detector() {
        assert!(LoopDetector::new(0).is_none());
    }
}
