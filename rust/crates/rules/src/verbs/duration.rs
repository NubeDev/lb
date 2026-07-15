//! Durations — the one `s/m/h/d/w` grammar, in three roles: `duration_to_surql` validates for
//! query composition (ported from rubix-cube's `duration_to_interval`, re-targeted to SurrealQL's
//! own suffixed literals); `parse_secs` is the shared numeric parser `time.add/sub/floor/ceil` and
//! the `dur_*` verbs call; and `register` wires the author-callable verbs (data-stdlib-scope) —
//! parse (`dur_secs`/`dur_ms`), humanize (`dur_human`), and the unit constructors.

use rhai::{Engine, EvalAltResult};

use crate::grid::rhai_err;

/// Validate a duration string and return its SurrealQL `duration` literal form. Accepts an integer
/// magnitude + one unit suffix in `s|m|h|d|w`. Errors on anything else.
pub fn duration_to_surql(s: &str) -> Result<String, String> {
    let (magnitude, unit) = split_duration(s)?;
    Ok(format!("{magnitude}{unit}"))
}

/// Parse the `s/m/h/d/w` form into whole seconds — the ONE duration grammar, shared by the `dur_*`
/// verbs and `time.add/sub/floor/ceil` (a duration means the same thing everywhere).
pub(crate) fn parse_secs(s: &str) -> Result<i64, String> {
    let (magnitude, unit) = split_duration(s)?;
    let mult: i64 = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3_600,
        "d" => 86_400,
        "w" => 604_800,
        _ => unreachable!("split_duration validated the unit"),
    };
    i64::try_from(magnitude)
        .ok()
        .and_then(|m| m.checked_mul(mult))
        .ok_or_else(|| format!("duration {s:?} overflows"))
}

/// Split `"24h"` into `(24, "h")`, validating both halves (a bad duration is a clear author error
/// at parse time, never raw text into a query).
fn split_duration(s: &str) -> Result<(u64, &str), String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty duration".into());
    }
    let (num, unit) = s.split_at(
        s.find(|c: char| !c.is_ascii_digit())
            .ok_or_else(|| format!("duration {s:?} missing a unit (e.g. 24h)"))?,
    );
    let magnitude: u64 = num
        .parse()
        .map_err(|_| format!("duration {s:?} has a non-numeric magnitude"))?;
    if !matches!(unit, "s" | "m" | "h" | "d" | "w") {
        return Err(format!(
            "duration {s:?} has unknown unit {unit:?} (use s/m/h/d/w)"
        ));
    }
    Ok((magnitude, unit))
}

/// The first `max` non-zero units of `|secs|`, largest first (`(12000, 2)` → `"3h 20m"`); `"0s"`
/// for zero; a leading `-` when negative. `time.ago` caps at 2 units; `dur_human` shows all.
pub(crate) fn human_units(secs: i64, max: usize) -> String {
    const UNITS: [(i64, &str); 5] = [
        (604_800, "w"),
        (86_400, "d"),
        (3_600, "h"),
        (60, "m"),
        (1, "s"),
    ];
    let mut left = secs.saturating_abs(); // i64::MIN saturates (not a realistic duration)
    let mut parts: Vec<String> = Vec::new();
    for (size, tag) in UNITS {
        if parts.len() == max {
            break;
        }
        let n = left / size;
        if n > 0 {
            parts.push(format!("{n}{tag}"));
            left -= n * size;
        }
    }
    if parts.is_empty() {
        return "0s".to_string();
    }
    let joined = parts.join(" ");
    if secs < 0 {
        format!("-{joined}")
    } else {
        joined
    }
}

/// Register the author-callable duration verbs (free functions — no handle, no authority).
pub fn register(engine: &mut Engine) {
    engine.register_fn("dur_secs", |s: &str| parse_secs(s).map_err(rhai_err));
    engine.register_fn("dur_ms", |s: &str| {
        parse_secs(s)
            .map_err(rhai_err)?
            .checked_mul(1000)
            .ok_or_else(|| rhai_err(format!("duration {s:?} overflows in milliseconds")))
    });
    engine.register_fn("dur_human", |secs: i64| human_units(secs, usize::MAX));
    engine.register_fn("seconds", |n: i64| n);
    engine.register_fn("minutes", |n: i64| construct(n, 60, "minutes"));
    engine.register_fn("hours", |n: i64| construct(n, 3_600, "hours"));
    engine.register_fn("days", |n: i64| construct(n, 86_400, "days"));
    engine.register_fn("weeks", |n: i64| construct(n, 604_800, "weeks"));
}

fn construct(n: i64, mult: i64, verb: &str) -> Result<i64, Box<EvalAltResult>> {
    n.checked_mul(mult)
        .ok_or_else(|| rhai_err(format!("{verb}({n}) overflows")))
}

/// Catalog rows for the author-callable duration verbs (`duration_to_surql`/`parse_secs` are
/// internal, not verbs). Family "time" — durations are the time family's units.
pub(crate) const CATALOG: &[crate::catalog::FnEntry] = &[
    crate::catalog::FnEntry {
        name: "dur_secs",
        family: "time",
        signature: "dur_secs(dur: String) -> int",
        description: "Parse a s/m/h/d/w duration (\"24h\") into seconds.",
    },
    crate::catalog::FnEntry {
        name: "dur_ms",
        family: "time",
        signature: "dur_ms(dur: String) -> int",
        description: "Parse a s/m/h/d/w duration (\"15m\") into milliseconds.",
    },
    crate::catalog::FnEntry {
        name: "dur_human",
        family: "time",
        signature: "dur_human(secs: int) -> String",
        description: "Humanize a second count into unit parts (\"1d 2h 5m\").",
    },
    crate::catalog::FnEntry {
        name: "seconds",
        family: "time",
        signature: "seconds(n: int) -> int",
        description: "n seconds, as seconds (the identity constructor, for symmetry).",
    },
    crate::catalog::FnEntry {
        name: "minutes",
        family: "time",
        signature: "minutes(n: int) -> int",
        description: "n minutes, as seconds.",
    },
    crate::catalog::FnEntry {
        name: "hours",
        family: "time",
        signature: "hours(n: int) -> int",
        description: "n hours, as seconds.",
    },
    crate::catalog::FnEntry {
        name: "days",
        family: "time",
        signature: "days(n: int) -> int",
        description: "n days, as seconds.",
    },
    crate::catalog::FnEntry {
        name: "weeks",
        family: "time",
        signature: "weeks(n: int) -> int",
        description: "n weeks, as seconds.",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_durations() {
        assert_eq!(duration_to_surql("24h").unwrap(), "24h");
        assert_eq!(duration_to_surql("7d").unwrap(), "7d");
        assert_eq!(duration_to_surql(" 15m ").unwrap(), "15m");
    }

    #[test]
    fn rejects_bad_durations() {
        assert!(duration_to_surql("").is_err());
        assert!(duration_to_surql("24").is_err());
        assert!(duration_to_surql("10y").is_err());
        assert!(duration_to_surql("abc").is_err());
    }

    #[test]
    fn parse_secs_converts_each_unit() {
        assert_eq!(parse_secs("90s").unwrap(), 90);
        assert_eq!(parse_secs("15m").unwrap(), 900);
        assert_eq!(parse_secs("24h").unwrap(), 86_400);
        assert_eq!(parse_secs("7d").unwrap(), 604_800);
        assert_eq!(parse_secs("2w").unwrap(), 1_209_600);
        assert!(parse_secs("99999999999999999999w").is_err()); // overflow, not a wrap
    }

    #[test]
    fn humanizes() {
        assert_eq!(human_units(0, usize::MAX), "0s");
        assert_eq!(human_units(93_900, usize::MAX), "1d 2h 5m");
        assert_eq!(human_units(12_000, 2), "3h 20m");
        assert_eq!(human_units(-90, usize::MAX), "-1m 30s");
        assert_eq!(human_units(604_800 + 61, usize::MAX), "1w 1m 1s"); // zero units skipped
    }
}
