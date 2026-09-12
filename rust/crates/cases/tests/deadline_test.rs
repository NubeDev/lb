//! The business-hours arithmetic, exercised through the crate's public surface
//! (`docs/scope/insights/case-plane-scope.md` — the SLA clock).
//!
//! **Pure**: no store, no node, no wall clock. `from_ms` is injected and every other input is a
//! literal — which is the whole point of keeping `add_business_hours` free of I/O. The
//! Friday-afternoon, public-holiday and daylight-saving cases are the ones that matter, and they
//! are all unreachable from a test that has to wait for a real clock.
//!
//! These sit beside the module rather than inside it only because `src/deadline.rs` would otherwise
//! be over the 400-line hard limit (FILE-LAYOUT §3). They are unit tests in everything but path.

use chrono::{TimeZone, Utc};
use chrono_tz::Tz;

use lb_cases::{add_business_hours, due_at, respond_by, Calendar, DayHours, ServicePolicy, NEVER};

const MS_PER_MINUTE: i64 = 60_000;
const MS_PER_HOUR: i64 = 3_600_000;
const BRISBANE: &str = "Australia/Brisbane"; // UTC+10 year-round — no DST to muddy a case.
const SYDNEY: &str = "Australia/Sydney"; // UTC+10/+11 — the DST case.

const NINE_AM: u32 = 9 * 60;
const FIVE_PM: u32 = 17 * 60;

/// Mon–Fri 09:00–17:00 in `tz`, closed weekends, with the given holidays.
fn office(tz: &str, holidays: &[&str]) -> Calendar {
    let mut hours = [DayHours::CLOSED; 7];
    for day in hours.iter_mut().take(5) {
        *day = DayHours::new(NINE_AM, FIVE_PM);
    }
    Calendar::Business {
        tz: tz.into(),
        hours,
        holidays: holidays.iter().map(|s| s.to_string()).collect(),
    }
}

/// A local wall time in `tz` → epoch ms. The tests are written in local time because that is
/// the language the contract is written in.
fn at(tz: &str, y: i32, m: u32, d: u32, h: u32, min: u32) -> u64 {
    let tz: Tz = tz.parse().unwrap();
    tz.with_ymd_and_hms(y, m, d, h, min, 0)
        .single()
        .expect("an unambiguous local time")
        .timestamp_millis() as u64
}

/// Render an epoch-ms instant back into `tz` local time, for readable assertions.
fn local(tz: &str, ms: u64) -> String {
    let tz: Tz = tz.parse().unwrap();
    Utc.timestamp_millis_opt(ms as i64)
        .single()
        .unwrap()
        .with_timezone(&tz)
        .format("%Y-%m-%d %H:%M")
        .to_string()
}

// --- the scope's named cases ---------------------------------------------------------------

/// **The case the scope names.** Mon–Fri 09:00–17:00, a case opened **Friday 16:00** with
/// `respond_h: 2`. One hour is left before close; the other hour is owed on the next open day.
/// The weekend is not business time, so the answer is **Monday 10:00** — not Friday 18:00, and
/// not Saturday.
#[test]
fn respond_by_crosses_a_weekend() {
    let cal = office(BRISBANE, &[]);
    // 2026-10-02 is a Friday; 2026-10-05 the Monday after.
    let opened = at(BRISBANE, 2026, 10, 2, 16, 0);
    let got = add_business_hours(&cal, opened, 2);
    assert_eq!(local(BRISBANE, got), "2026-10-05 10:00");
}

/// The same case with the Monday declared a public holiday: the owed hour moves again, to
/// **Tuesday 10:00**. A holiday is a local-calendar date, not a UTC one.
#[test]
fn respond_by_crosses_a_holiday() {
    let cal = office(BRISBANE, &["2026-10-05"]);
    let opened = at(BRISBANE, 2026, 10, 2, 16, 0);
    let got = add_business_hours(&cal, opened, 2);
    assert_eq!(local(BRISBANE, got), "2026-10-06 10:00");
}

/// Several consecutive holidays stack, and a holiday landing on an already-closed weekend day
/// costs nothing extra.
#[test]
fn consecutive_holidays_stack() {
    // Mon and Tue both closed; the Saturday holiday is a no-op.
    let cal = office(BRISBANE, &["2026-10-03", "2026-10-05", "2026-10-06"]);
    let opened = at(BRISBANE, 2026, 10, 2, 16, 0);
    assert_eq!(
        local(BRISBANE, add_business_hours(&cal, opened, 2)),
        "2026-10-07 10:00"
    );
}

// --- starting outside the window -----------------------------------------------------------

/// Opened at 07:00, before the doors open: the clock starts at **09:00**, not 07:00. Two hours
/// later is 11:00 — if the pre-opening time counted, this would read 09:00.
#[test]
fn a_case_opened_before_opening_starts_the_clock_at_opening() {
    let cal = office(BRISBANE, &[]);
    let opened = at(BRISBANE, 2026, 10, 5, 7, 0); // a Monday
    assert_eq!(
        local(BRISBANE, add_business_hours(&cal, opened, 2)),
        "2026-10-05 11:00"
    );
}

/// Opened at 19:00, after close: the clock starts the **next** morning at 09:00.
#[test]
fn a_case_opened_after_close_starts_the_next_morning() {
    let cal = office(BRISBANE, &[]);
    let opened = at(BRISBANE, 2026, 10, 5, 19, 0); // Monday evening
    assert_eq!(
        local(BRISBANE, add_business_hours(&cal, opened, 2)),
        "2026-10-06 11:00"
    );
}

/// The boundaries. Exactly at opening is **inside** the window, so the clock starts there and
/// nothing is skipped. Exactly at closing is **outside** it (the window is half-open), so the
/// clock rolls to the next open day — the one asymmetry, and it is deliberate: an instant can
/// belong to only one day's window.
#[test]
fn the_opening_instant_is_inside_and_the_closing_instant_is_not() {
    let cal = office(BRISBANE, &[]);
    let at_open = at(BRISBANE, 2026, 10, 5, 9, 0);
    assert_eq!(
        local(BRISBANE, add_business_hours(&cal, at_open, 2)),
        "2026-10-05 11:00"
    );
    let at_close = at(BRISBANE, 2026, 10, 5, 17, 0);
    assert_eq!(
        local(BRISBANE, add_business_hours(&cal, at_close, 2)),
        "2026-10-06 11:00"
    );
    // Consuming exactly the rest of the day lands ON closing, and stays on that day.
    assert_eq!(
        local(BRISBANE, add_business_hours(&cal, at_open, 8)),
        "2026-10-05 17:00"
    );
}

/// Zero hours means "the next instant business time is running" — now, if we are inside the
/// window; the next opening, if we are not.
#[test]
fn zero_hours_resolves_to_the_next_business_instant() {
    let cal = office(BRISBANE, &[]);
    let inside = at(BRISBANE, 2026, 10, 5, 10, 30);
    assert_eq!(
        local(BRISBANE, add_business_hours(&cal, inside, 0)),
        "2026-10-05 10:30"
    );
    let before = at(BRISBANE, 2026, 10, 5, 7, 0);
    assert_eq!(
        local(BRISBANE, add_business_hours(&cal, before, 0)),
        "2026-10-05 09:00"
    );
    let weekend = at(BRISBANE, 2026, 10, 3, 12, 0); // a Saturday
    assert_eq!(
        local(BRISBANE, add_business_hours(&cal, weekend, 0)),
        "2026-10-05 09:00"
    );
}

// --- spilling ------------------------------------------------------------------------------

/// A request longer than one day's window spills across days. `resolve_h: 20` over 8-hour days
/// is two and a half business days: Monday 09:00 → Wednesday 13:00, with the weekend nowhere
/// near it.
#[test]
fn a_multi_day_request_spills_across_days() {
    let cal = office(BRISBANE, &[]);
    let opened = at(BRISBANE, 2026, 10, 5, 9, 0); // Monday
    assert_eq!(
        local(BRISBANE, add_business_hours(&cal, opened, 20)),
        "2026-10-07 13:00"
    );
}

/// A spill that also crosses a weekend, and one that crosses two: the days that are not
/// business days simply do not count, however many of them there are.
#[test]
fn a_spill_skips_every_closed_day_it_meets() {
    let cal = office(BRISBANE, &[]);
    let opened = at(BRISBANE, 2026, 10, 1, 9, 0); // Thursday
                                                  // 20 h = Thu(8) + Fri(8) + Mon(4) → Monday 13:00.
    assert_eq!(
        local(BRISBANE, add_business_hours(&cal, opened, 20)),
        "2026-10-05 13:00"
    );
    // 60 h = 7.5 eight-hour days → Thu, Fri, Mon–Fri, then half of the following Monday.
    assert_eq!(
        local(BRISBANE, add_business_hours(&cal, opened, 60)),
        "2026-10-12 13:00"
    );
}

// --- Always ---------------------------------------------------------------------------------

/// `Calendar::Always` is plain wall arithmetic — no calendar walk, no timezone, weekends
/// included. This is the default, so it is also the behaviour of a policy that says nothing.
#[test]
fn always_is_plain_wall_arithmetic() {
    let opened = at(BRISBANE, 2026, 10, 2, 16, 0); // Friday
    assert_eq!(
        add_business_hours(&Calendar::Always, opened, 2),
        opened + 2 * MS_PER_HOUR as u64
    );
    assert_eq!(
        add_business_hours(&Calendar::Always, opened, 72),
        opened + 72 * MS_PER_HOUR as u64
    );
    assert_eq!(add_business_hours(&Calendar::Always, opened, 0), opened);
}

// --- degenerate calendars --------------------------------------------------------------------

/// A calendar with no open hours anywhere in the week **terminates** and returns the documented
/// `NEVER` sentinel. It does not spin. `Calendar::validate` refuses to store one, so this is
/// the backstop for a row that predates the check.
#[test]
fn a_calendar_that_never_opens_terminates() {
    let cal = Calendar::Business {
        tz: BRISBANE.into(),
        hours: [DayHours::CLOSED; 7],
        holidays: vec![],
    };
    assert_eq!(
        add_business_hours(&cal, at(BRISBANE, 2026, 10, 5, 9, 0), 2),
        NEVER
    );
    // Even asking for zero hours: there is no business instant to return.
    assert_eq!(
        add_business_hours(&cal, at(BRISBANE, 2026, 10, 5, 9, 0), 0),
        NEVER
    );
}

/// A day whose `open_min == close_min` is closed, and is stepped over exactly like a weekend.
#[test]
fn a_zero_length_day_is_closed() {
    let mut hours = [DayHours::CLOSED; 7];
    hours[0] = DayHours::new(NINE_AM, FIVE_PM); // Monday open
    hours[1] = DayHours::new(NINE_AM, NINE_AM); // Tuesday: open == close ⇒ closed
    hours[2] = DayHours::new(NINE_AM, FIVE_PM); // Wednesday open
    let cal = Calendar::Business {
        tz: BRISBANE.into(),
        hours,
        holidays: vec![],
    };
    // Monday 16:00 + 2 h: one hour Monday, Tuesday is closed, the rest Wednesday 09:00–10:00.
    let opened = at(BRISBANE, 2026, 10, 5, 16, 0);
    assert_eq!(
        local(BRISBANE, add_business_hours(&cal, opened, 2)),
        "2026-10-07 10:00"
    );
}

/// An unresolvable timezone on a stored row fails to `NEVER` rather than to an arbitrary
/// instant — the fail-safe direction (see the module note).
#[test]
fn an_unknown_timezone_fails_to_never() {
    let cal = Calendar::Business {
        tz: "Mars/Olympus".into(),
        hours: [DayHours::new(NINE_AM, FIVE_PM); 7],
        holidays: vec![],
    };
    assert_eq!(add_business_hours(&cal, 1_000_000, 2), NEVER);
}

// --- daylight saving --------------------------------------------------------------------------

/// **Why the calendar carries an IANA name and not a fixed offset.** The same Friday-16:00 case
/// in Sydney, over the weekend the clocks go forward. Local time says Monday 10:00 either way —
/// but the *instant* differs by an hour, and it is the instant a breach reminder fires at.
///
/// Friday 16:00 AEST is +10; Monday 10:00 AEDT is +11; so only **65** wall hours pass, not 66.
/// A fixed `tz_offset_minutes: 600` would compute 66 and fire every deadline an hour late for
/// half the year.
#[test]
fn a_daylight_saving_transition_shortens_the_wall_gap() {
    let cal = office(SYDNEY, &[]);
    let opened = at(SYDNEY, 2026, 10, 2, 16, 0); // Friday, AEST
    let got = add_business_hours(&cal, opened, 2);
    assert_eq!(
        local(SYDNEY, got),
        "2026-10-05 10:00",
        "local time is unmoved"
    );

    let wall_hours = (got - opened) as i64 / MS_PER_HOUR;
    assert_eq!(
        wall_hours, 65,
        "the clocks went forward over the weekend, so one fewer wall hour elapsed"
    );
    // The Brisbane twin has no DST, and is the 66 a fixed offset would have given.
    let bne = add_business_hours(&office(BRISBANE, &[]), at(BRISBANE, 2026, 10, 2, 16, 0), 2);
    assert_eq!(
        (bne - at(BRISBANE, 2026, 10, 2, 16, 0)) as i64 / MS_PER_HOUR,
        66
    );
}

/// The autumn transition, the other way: a local time that happens **twice** resolves to the
/// earlier of the two instants, deterministically.
#[test]
fn an_ambiguous_local_time_takes_the_earlier_instant() {
    // 2026-04-05 02:00–03:00 AEDT repeats in Sydney. A calendar open across it.
    let mut hours = [DayHours::CLOSED; 7];
    hours[6] = DayHours::new(0, 6 * 60); // Sunday 00:00–06:00
    let cal = Calendar::Business {
        tz: SYDNEY.into(),
        hours,
        holidays: vec![],
    };
    let opened = at(SYDNEY, 2026, 4, 5, 0, 0);
    let got = add_business_hours(&cal, opened, 2);
    // Two business hours from Sunday 00:00 is local 02:00 — the ambiguous one. We take the
    // first occurrence, which is 2 wall hours later, not 3.
    assert_eq!((got - opened) as i64 / MS_PER_HOUR, 2);
    assert_eq!(local(SYDNEY, got), "2026-04-05 02:00");
}

// --- the policy wrappers -----------------------------------------------------------------------

/// `respond_by` and `due_at` are the two hour fields fed through the same arithmetic — the
/// scope's Friday-16:00 case, read off a real policy row.
#[test]
fn respond_by_and_due_at_read_their_own_fields() {
    let policy = ServicePolicy {
        id: "contract-a".into(),
        name: "Contract A".into(),
        r#match: Default::default(),
        active: true,
        respond_h: 2,
        resolve_h: 20,
        calendar: office(BRISBANE, &[]),
        party_window_h: 48,
        hold_down_days: 14,
    };
    let opened = at(BRISBANE, 2026, 10, 2, 16, 0); // Friday
    assert_eq!(
        local(BRISBANE, respond_by(&policy, opened)),
        "2026-10-05 10:00"
    );
    // 20 h from the same start: 1 h Friday, then Mon(8) Tue(8) Wed(3) → Wednesday 12:00.
    assert_eq!(local(BRISBANE, due_at(&policy, opened)), "2026-10-07 12:00");
    assert!(respond_by(&policy, opened) < due_at(&policy, opened));
}

/// Seconds on the opening instant are carried, not rounded away — a case opened at 16:00:30 is
/// due at 10:00:30, so two cases opened a minute apart keep their order in the queue.
#[test]
fn sub_minute_precision_survives() {
    let cal = office(BRISBANE, &[]);
    let opened = at(BRISBANE, 2026, 10, 2, 16, 0) + 30_000;
    let got = add_business_hours(&cal, opened, 2);
    assert_eq!(
        got % MS_PER_MINUTE as u64,
        30_000,
        "the 30 seconds are still there"
    );
    assert_eq!(local(BRISBANE, got), "2026-10-05 10:00");
}
