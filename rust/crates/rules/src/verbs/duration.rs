//! `duration_to_surql` — parse a human duration (`"24h"`, `"7d"`, `"15m"`) into a validated form,
//! at AUTHORING time (a bad duration is a clear error, never raw text into a query). **Ported from
//! rubix-cube's `duration_to_interval`** (`rules/verbs/mod.rs`), re-targeted: platform data is
//! SurrealQL, whose `duration` literals are the suffixed form themselves (`24h`, `7d`), so we
//! validate + normalize rather than translate to `"24 hours"`.

/// Validate a duration string and return its SurrealQL `duration` literal form. Accepts an integer
/// magnitude + one unit suffix in `s|m|h|d|w`. Errors on anything else.
pub fn duration_to_surql(s: &str) -> Result<String, String> {
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
    let unit = match unit {
        "s" | "m" | "h" | "d" | "w" => unit,
        other => {
            return Err(format!(
                "duration {s:?} has unknown unit {other:?} (use s/m/h/d/w)"
            ))
        }
    };
    Ok(format!("{magnitude}{unit}"))
}

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
}
