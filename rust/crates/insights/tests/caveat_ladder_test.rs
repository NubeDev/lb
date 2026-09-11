//! **A caveated insight is never a breakthrough** (case-plane-scope.md) — the one assertion the case
//! plane makes about the notify ladder, driven against the pure `ladder_step` with no I/O.
//!
//! Its own file rather than more of `ladder_test.rs`: that file is the ladder's OWN behaviour
//! (breakthroughs, levels, throttles, determinism) and this is a different question asked of the same
//! function — does a finding we already distrust reach anybody? Adding it there took that file past
//! the 400-line FILE-LAYOUT limit, which is the limit doing its job: two concerns had accumulated in
//! one place.
//!
//! The fixtures below are deliberately a local copy rather than a shared `common` module. A Rust
//! integration test is its own crate, and thirty lines of obvious fixture is a cheaper dependency
//! than a shared helper two suites can silently drift apart over.

use lb_insights::{
    ladder_step, policy_defaults, Delivery, DeliveryReason, Intent, IntentKind, LadderInput,
    NotifyState, Policy, Severity,
};

/// A minimal intent fixture (the dedup-key/sub identity is arbitrary for the pure fn).
fn intent(kind: IntentKind, severity: Severity, _ts: u64) -> Intent {
    Intent {
        sub_id: "sub-1".into(),
        insight_id: "ins-1".into(),
        dedup_key: "key-1".into(),
        severity,
        kind,
        caveated: false,
    }
}

/// The same fixture, but CAVEATED — an open data-quality finding on the same subjects undermines
/// it (`caveat.rs`).
fn caveated_intent(kind: IntentKind, severity: Severity) -> Intent {
    Intent {
        caveated: true,
        ..intent(kind, severity, 0)
    }
}

/// The default policy (compiled `defaults()`).
fn policy() -> Policy {
    policy_defaults()
}

/// Feed one Intent, delivery allowed (kill switch on, not muted, no throttle pin).
fn step_intent(
    state: Option<NotifyState>,
    kind: IntentKind,
    sev: Severity,
    acked: bool,
    now: u64,
) -> (NotifyState, Vec<Delivery>) {
    ladder_step(
        state,
        LadderInput::Intent {
            intent: &intent(kind, sev, now),
            acked,
            now,
        },
        &policy(),
        None,
        false,
        true,
    )
}

// --- "a caveated insight is never a breakthrough" (case-plane-scope.md) -------------------

/// **THE ASSERTION THE CASE PLANE TURNS ON.** A `critical`, first-ever raise is the loudest thing
/// this machine can produce: no prior state row ⇒ `FirstKey` breakthrough, and at L0 with no
/// `last_sent_ts` the immediate post fires too. Caveated, it must produce **no delivery at all** —
/// nobody is paged at 2am about a number produced by a sensor we already know is broken.
///
/// Written as a PAIR against the identical input so the test cannot pass by the machine being
/// silent for some unrelated reason: the uncaveated half must deliver.
#[test]
fn a_caveated_critical_raise_delivers_nothing_while_an_uncaveated_one_delivers() {
    // Uncaveated — the control.
    let (_, delivered) = step_intent(None, IntentKind::Raise, Severity::Critical, false, 1);
    assert_eq!(
        delivered.len(),
        1,
        "the control must actually deliver, or this test proves nothing"
    );
    assert_eq!(delivered[0].reason, DeliveryReason::FirstKey);

    // Caveated — the same input, the same clock, the same policy.
    let (state, deliveries) = ladder_step(
        None,
        LadderInput::Intent {
            intent: &caveated_intent(IntentKind::Raise, Severity::Critical),
            acked: false,
            now: 1,
        },
        &policy(),
        None,
        false,
        true,
    );
    assert!(
        deliveries.is_empty(),
        "a caveated finding must never break through and must not post immediately: {deliveries:?}"
    );
    // …but it is NOT dropped. The accounting ran and the pending accumulator holds it, so the
    // digest still carries it — suppressed, not silenced.
    assert_eq!(
        state.pending.count, 1,
        "the firing still accumulates for the digest"
    );
    assert_eq!(state.window_hits, 1);
    assert_eq!(state.last_sent_ts, None, "nothing was sent");
}

/// The two loud breakthrough paths — re-open and escalation — are suppressed too. Suppressing only
/// the ordinary path would leave exactly the deliveries that wake people untouched, which is the
/// whole failure this prevents.
#[test]
fn a_caveated_reopen_or_escalation_does_not_break_through_either() {
    for kind in [IntentKind::Reopen, IntentKind::Escalate] {
        // Seed a state row first, so `first_key` is not what is doing the work.
        let (state, _) = step_intent(None, IntentKind::Raise, Severity::Info, false, 1);
        let (_, deliveries) = ladder_step(
            Some(state),
            LadderInput::Intent {
                intent: &caveated_intent(kind, Severity::Critical),
                acked: false,
                now: 2,
            },
            &policy(),
            None,
            false,
            true,
        );
        assert!(
            deliveries.is_empty(),
            "a caveated {kind:?} must not break through: {deliveries:?}"
        );
    }
}

/// And the caveat is not sticky in the state machine: it rides the INTENT, so the firing after the
/// gating finding is resolved delivers normally. That is what makes the caveat self-healing rather
/// than a one-way mute.
#[test]
fn clearing_the_caveat_restores_delivery_on_the_next_firing() {
    let (state, deliveries) = ladder_step(
        None,
        LadderInput::Intent {
            intent: &caveated_intent(IntentKind::Raise, Severity::Critical),
            acked: false,
            now: 1,
        },
        &policy(),
        None,
        false,
        true,
    );
    assert!(deliveries.is_empty());

    // Same key, same sub — the data-quality finding was resolved, so this raise is uncaveated.
    let (_, deliveries) = ladder_step(
        Some(state),
        LadderInput::Intent {
            intent: &intent(IntentKind::Escalate, Severity::Critical, 2),
            acked: false,
            now: 2,
        },
        &policy(),
        None,
        false,
        true,
    );
    assert_eq!(
        deliveries.len(),
        1,
        "delivery resumes once the caveat clears"
    );
}
