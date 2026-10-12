//! `next_after` — the cron "next after T" math on the injected clock (testing §3 — no wall-clock).
//! Deterministic anchors: 2024-01-01 is a Monday. `1704067200` = Mon 2024-01-01 00:00:00 UTC.

use lb_reminders::{is_valid, next_after};

const MON_JAN1_0000: u64 = 1_704_067_200; // Mon 2024-01-01 00:00:00 UTC
const MON_JAN1_0800: u64 = 1_704_096_000; // Mon 2024-01-01 08:00:00 UTC (+8h)
const MON_JAN8_0800: u64 = 1_704_700_800; // Mon 2024-01-08 08:00:00 UTC (+7d)
const SUN_JAN7_0800: u64 = 1_704_614_400; // Sun 2024-01-07 08:00:00 UTC

#[test]
fn recurring_weekly_monday_from_midnight() {
    // "0 8 * * 1" — every Monday 08:00. From Mon 00:00 the next slot is Mon 08:00 (same day).
    assert_eq!(
        next_after("0 8 * * 1", MON_JAN1_0000).unwrap(),
        MON_JAN1_0800
    );
    // From Mon 08:00 (inclusive=false) the next is the FOLLOWING Monday 08:00 (7 days).
    assert_eq!(
        next_after("0 8 * * 1", MON_JAN1_0800).unwrap(),
        MON_JAN8_0800
    );
}

#[test]
fn recurring_multi_day_field_sun_and_mon() {
    // "0 8 * * 0,1" — Sunday + Monday at 08:00. From Mon 00:00 the next is Mon 08:00; from Mon 08:00
    // the next is Sun 07 08:00 (Sunday comes before the following Monday).
    assert_eq!(
        next_after("0 8 * * 0,1", MON_JAN1_0000).unwrap(),
        MON_JAN1_0800
    );
    assert_eq!(
        next_after("0 8 * * 0,1", MON_JAN1_0800).unwrap(),
        SUN_JAN7_0800
    );
    // And after Sunday it rolls to Monday again (the multi-value field alternates correctly).
    assert_eq!(
        next_after("0 8 * * 0,1", SUN_JAN7_0800).unwrap(),
        MON_JAN8_0800
    );
}

#[test]
fn every_minute_advances_by_sixty_seconds() {
    // "* * * * *" — every minute. Strictly-after semantics: +60s each step.
    assert_eq!(
        next_after("* * * * *", MON_JAN1_0000).unwrap(),
        MON_JAN1_0000 + 60
    );
    assert_eq!(
        next_after("* * * * *", MON_JAN1_0000 + 60).unwrap(),
        MON_JAN1_0000 + 120
    );
}

#[test]
fn one_shot_anchor_picks_the_single_next_slot() {
    // A "one-shot" is just max_runs=1 over any cron; the math is the same. Verify the first slot is
    // strictly future (a fire at exactly T is not re-found at T — the reactor's idempotency anchor).
    let first = next_after("30 9 * * *", MON_JAN1_0000).unwrap();
    assert_eq!(first, MON_JAN1_0000 + (9 * 3600 + 30 * 60)); // 09:30 same day
    assert_ne!(first, MON_JAN1_0000);
}

#[test]
fn strictly_after_is_inclusive_false() {
    // At exactly the slot, the next is the FOLLOWING slot (inclusive=false). This is what makes the
    // reactor's advance idempotent: after firing at T and recomputing from now=T, it does not re-find T.
    let slot = next_after("0 8 * * 1", MON_JAN1_0000).unwrap();
    assert_eq!(slot, MON_JAN1_0800);
    let again = next_after("0 8 * * 1", slot).unwrap();
    assert_eq!(
        again, MON_JAN8_0800,
        "re-anchoring at the fired slot skips it"
    );
}

#[test]
fn bad_cron_is_rejected_and_valid_is_accepted() {
    assert!(is_valid("0 8 * * 1"));
    assert!(is_valid("*/5 * * * *"));
    assert!(!is_valid("not a cron"));
    assert!(!is_valid("99 8 * * *")); // minute out of range
    assert!(next_after("garbage", 0).is_err());
}

// ---- the schedule's own zone (`next_after_in`) -------------------------------------------------
// "0 9 * * *" in Australia/Sydney is 09:00 Sydney wall-clock, which in January (AEDT, UTC+11) is
// 22:00 UTC the previous day. The anchors below are all plain UTC-second instants.

use lb_reminders::{is_valid_tz, next_after_in};

const MON_JAN1_0900_UTC: u64 = 1_704_099_600; // Mon 2024-01-01 09:00:00 UTC
const MON_JAN1_2200_UTC: u64 = 1_704_146_400; // Mon 2024-01-01 22:00:00 UTC = Tue 09:00 AEDT

#[test]
fn a_zone_reads_the_cron_hour_in_that_zone() {
    // From Mon 00:00 UTC (= Mon 11:00 AEDT, already past 09:00 Sydney), the next 09:00 Sydney is
    // Tue 09:00 AEDT = Mon 22:00 UTC — not Mon 09:00 UTC, which is what the UTC reading gives.
    assert_eq!(
        next_after_in("0 9 * * *", MON_JAN1_0000, "Australia/Sydney").unwrap(),
        MON_JAN1_2200_UTC
    );
    assert_eq!(
        next_after_in("0 9 * * *", MON_JAN1_0000, "UTC").unwrap(),
        MON_JAN1_0900_UTC
    );
}

#[test]
fn an_empty_zone_is_utc_and_matches_next_after() {
    assert_eq!(
        next_after_in("0 8 * * 1", MON_JAN1_0000, "").unwrap(),
        next_after("0 8 * * 1", MON_JAN1_0000).unwrap()
    );
}

#[test]
fn the_weekday_field_follows_the_zone_too() {
    // "0 8 * * 1" (Mondays 08:00) in Sydney from Mon 00:00 UTC (Mon 11:00 AEDT, past 08:00): the
    // next is the FOLLOWING Monday 08:00 AEDT = Sun 2024-01-07 21:00 UTC.
    const SUN_JAN7_2100_UTC: u64 = 1_704_661_200;
    assert_eq!(
        next_after_in("0 8 * * 1", MON_JAN1_0000, "Australia/Sydney").unwrap(),
        SUN_JAN7_2100_UTC
    );
}

#[test]
fn an_unknown_zone_is_refused_by_name() {
    let err = next_after_in("0 9 * * *", MON_JAN1_0000, "Mars/Olympus").unwrap_err();
    assert!(err.to_string().contains("Mars/Olympus"), "got {err}");
    assert!(!is_valid_tz("Mars/Olympus"));
    assert!(is_valid_tz(""));
    assert!(is_valid_tz("Australia/Sydney"));
}
