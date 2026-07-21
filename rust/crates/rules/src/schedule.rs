//! The `#[schedule(...)]` **authoring directive** — extract + compile (scheduled-rules-scope). This is
//! the `phrase → cron string` seam and NOTHING more: it parses a top-of-body annotation and turns a
//! natural-language phrase (or an explicit `cron="..."`) into a **5-field cron string**. It is
//! deliberately a *text compiler*, never a time engine — it computes no fire time, holds no clock, and
//! is **never executed** in the cage. The host runs this at `rules.save`, validates the emitted cron
//! with `croner` (the ONE sanctioned engine, via `lb-reminders`), and compiles the result to a managed
//! `cron → rule` flow. There is no rule-cron reactor — a scheduled rule fires through the existing flow
//! cron reactor (`react_to_flows_cron`).
//!
//! ## Why a vendored phrase-matcher (not `natural-cron`)
//!
//! The scope's candidate `natural-cron` is MIT (license OK) but a `0.0.2`, ~17%-documented crate whose
//! API is a cron *builder/validator*, not a `phrase → cron` NL parser — too immature to enter a core
//! crate under CLAUDE rule 1 ("core stays lean"). Per the scope's sanctioned fallback we vendor a thin
//! matcher for the **common phrases** here; the `#[schedule(cron = "...")]` explicit form is the
//! always-present escape hatch, and the always-visible next-runs preview (host-side, via `croner`) is
//! the trust primitive when a phrase guesses wrong. The seam is `phrase → cron string`, so swapping in
//! `ai.*` or a richer parser later touches only [`compile_phrase`].

use serde::{Deserialize, Serialize};

/// The compiled schedule stored on a saved rule (`schedule` field). `raw` is the author's directive
/// argument verbatim (a phrase or an explicit cron); `cron` is the compiled 5-field spec that `croner`
/// owns downstream (validation + next-runs). Reads return `cron` so the page never re-parses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleSchedule {
    /// The directive argument as written — the phrase (`"every 15 minutes"`) or the explicit cron.
    pub raw: String,
    /// The compiled 5-field cron string (what the managed flow's trigger carries).
    pub cron: String,
}

/// A directive-parse failure — surfaced as a **save error** (never a silent no-schedule). The message
/// is author-facing: it names what was wrong and points at the escape hatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleError {
    /// The `#[schedule(...)]` annotation was malformed (unbalanced parens, empty argument, bad quoting).
    Malformed(String),
    /// The phrase did not match any known form and is not an explicit `cron="..."`.
    Unparseable(String),
}

impl std::fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScheduleError::Malformed(s) => write!(f, "malformed #[schedule(...)] directive: {s}"),
            ScheduleError::Unparseable(p) => write!(
                f,
                "could not turn schedule phrase {p:?} into a cron expression — use one of the \
                 supported phrases (\"every N minutes/hours\", \"hourly\", \"daily at HH:MM\", \
                 \"weekdays at HH:MM\", \"every day\") or the explicit form \
                 #[schedule(cron = \"*/15 * * * *\")]"
            ),
        }
    }
}

/// Strip the `#[schedule(...)]` directive line(s) from a rule body so the **rhai cage never sees the
/// annotation** — `#` is a reserved symbol in rhai, so an un-stripped directive is a compile error at
/// run time (regression guard: `directive_line_is_stripped_before_the_cage`). The stored record keeps
/// the directive verbatim (it is the schedule's source of truth); only the executed body is stripped.
///
/// Cheap + allocation-free when there is no directive: returns `Cow::Borrowed` for the common
/// (unscheduled) rule, and only allocates when a directive line is actually removed. A directive line
/// is one whose first non-space chars are `#[schedule` (the same anchor [`extract_schedule`] uses), so
/// the token appearing inside rule logic is untouched.
pub fn strip_directive(body: &str) -> std::borrow::Cow<'_, str> {
    if !body
        .lines()
        .any(|l| l.trim_start().starts_with("#[schedule"))
    {
        return std::borrow::Cow::Borrowed(body);
    }
    let kept: Vec<&str> = body
        .lines()
        .filter(|l| !l.trim_start().starts_with("#[schedule"))
        .collect();
    std::borrow::Cow::Owned(kept.join("\n"))
}

/// Extract the `#[schedule(...)]` directive from a rule body, if present, and compile it to a
/// [`RuleSchedule`]. Returns:
///   - `Ok(None)` — no directive (the rule is run-on-demand; the syncer deletes any managed flow);
///   - `Ok(Some(sched))` — a directive that compiled to a valid-shaped cron string (croner-validated by
///     the host caller);
///   - `Err(_)` — a directive that is present but malformed or whose phrase is unparseable (a **save
///     error**, so an author is never silently told "scheduled" when nothing was compiled).
///
/// The scan is anchored to a **directive line** (a line whose first non-space chars are `#[schedule`),
/// so the token appearing inside rule logic or a string literal below the annotation block is never
/// mistaken for the directive. Only the first such line is honored; a second is a `Malformed` error
/// (an ambiguous double-schedule is a mistake, not a silent last-wins).
pub fn extract_schedule(body: &str) -> Result<Option<RuleSchedule>, ScheduleError> {
    let mut found: Option<&str> = None;
    for line in body.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("#[schedule") {
            continue;
        }
        if found.is_some() {
            return Err(ScheduleError::Malformed(
                "more than one #[schedule(...)] directive; a rule has at most one schedule".into(),
            ));
        }
        found = Some(trimmed);
    }
    let Some(line) = found else {
        return Ok(None);
    };
    let arg = directive_arg(line)?;
    compile_arg(&arg)
}

/// Pull the single argument out of `#[schedule( <arg> )]`. `<arg>` is either a quoted string
/// (`"every 15 minutes"`) or `cron = "..."`. Returns the inside-the-parens text with the outer
/// `#[schedule(` / `)]` stripped, trimmed — the caller ([`compile_arg`]) decides phrase vs explicit.
fn directive_arg(line: &str) -> Result<String, ScheduleError> {
    let open = line
        .find('(')
        .ok_or_else(|| ScheduleError::Malformed("expected `#[schedule(`".into()))?;
    let close = line
        .rfind(')')
        .ok_or_else(|| ScheduleError::Malformed("expected a closing `)`".into()))?;
    if close <= open + 1 {
        return Err(ScheduleError::Malformed(
            "empty #[schedule()] — supply a phrase or cron = \"...\"".into(),
        ));
    }
    Ok(line[open + 1..close].trim().to_string())
}

/// Compile a directive argument (`"phrase"` or `cron = "spec"`) to a [`RuleSchedule`]. The explicit
/// `cron = "..."` form passes its spec straight through as both `raw` and `cron` (host croner-validates
/// it); a bare quoted phrase goes through [`compile_phrase`].
fn compile_arg(arg: &str) -> Result<Option<RuleSchedule>, ScheduleError> {
    if let Some(rest) = arg.strip_prefix("cron") {
        let rest = rest.trim_start();
        let spec = rest
            .strip_prefix('=')
            .map(str::trim)
            .and_then(unquote)
            .ok_or_else(|| ScheduleError::Malformed("expected cron = \"<5-field spec>\"".into()))?;
        if spec.is_empty() {
            return Err(ScheduleError::Malformed("empty cron string".into()));
        }
        return Ok(Some(RuleSchedule {
            raw: spec.to_string(),
            cron: spec.to_string(),
        }));
    }
    let phrase = unquote(arg).ok_or_else(|| {
        ScheduleError::Malformed("expected a quoted phrase or cron = \"...\"".into())
    })?;
    let cron = compile_phrase(&phrase)?;
    Ok(Some(RuleSchedule { raw: phrase, cron }))
}

/// Strip a single pair of matching `"` or `'` quotes; `None` if the text is not quoted.
fn unquote(s: &str) -> Option<String> {
    let s = s.trim();
    let bytes = s.as_bytes();
    if bytes.len() >= 2 {
        let (a, b) = (bytes[0], bytes[bytes.len() - 1]);
        if (a == b'"' && b == b'"') || (a == b'\'' && b == b'\'') {
            return Some(s[1..s.len() - 1].to_string());
        }
    }
    None
}

/// The vendored **phrase → 5-field cron** matcher — the swappable seam (see module docs). Covers the
/// common phrases; anything unmatched is an [`ScheduleError::Unparseable`] save error (never a silent
/// no-schedule), which the `cron = "..."` escape hatch answers. Emits ONLY a cron string; it never
/// computes a time. All output is UTC (v1 — the reactor's logical clock is UTC; per-directive tz is a
/// documented follow-up).
pub fn compile_phrase(phrase: &str) -> Result<String, ScheduleError> {
    let p = phrase.trim().to_lowercase();
    let unparseable = || ScheduleError::Unparseable(phrase.to_string());

    // Fixed idioms first.
    match p.as_str() {
        "every minute" => return Ok("* * * * *".into()),
        "hourly" | "every hour" => return Ok("0 * * * *".into()),
        "daily" | "every day" | "nightly" => return Ok("0 0 * * *".into()),
        "weekly" | "every week" => return Ok("0 0 * * 0".into()),
        "monthly" | "every month" => return Ok("0 0 1 * *".into()),
        "weekdays" => return Ok("0 0 * * 1-5".into()),
        _ => {}
    }

    // "every N minutes" / "every N hours".
    if let Some(rest) = p.strip_prefix("every ") {
        if let Some(n) = rest
            .strip_suffix(" minutes")
            .or_else(|| rest.strip_suffix(" minute"))
        {
            let n: u32 = n.trim().parse().map_err(|_| unparseable())?;
            if n == 0 || n > 59 {
                return Err(unparseable());
            }
            return Ok(format!("*/{n} * * * *"));
        }
        if let Some(n) = rest
            .strip_suffix(" hours")
            .or_else(|| rest.strip_suffix(" hour"))
        {
            let n: u32 = n.trim().parse().map_err(|_| unparseable())?;
            if n == 0 || n > 23 {
                return Err(unparseable());
            }
            return Ok(format!("0 */{n} * * *"));
        }
    }

    // "... at HH:MM" family: "daily at 08:00", "every day at 2am", "weekdays at 08:00".
    if let Some((prefix, time)) = p.rsplit_once(" at ") {
        let (hh, mm) = parse_time(time.trim()).ok_or_else(unparseable)?;
        let prefix = prefix.trim();
        let dow = match prefix {
            "daily" | "every day" | "nightly" => "*",
            "weekdays" | "every weekday" => "1-5",
            "weekends" => "0,6",
            "monday" | "mondays" => "1",
            "tuesday" | "tuesdays" => "2",
            "wednesday" | "wednesdays" => "3",
            "thursday" | "thursdays" => "4",
            "friday" | "fridays" => "5",
            "saturday" | "saturdays" => "6",
            "sunday" | "sundays" => "0",
            _ => return Err(unparseable()),
        };
        return Ok(format!("{mm} {hh} * * {dow}"));
    }

    Err(unparseable())
}

/// Parse a clock time into `(hour, minute)`: `"08:00"`, `"8:00"`, `"2am"`, `"2pm"`, `"14:30"`.
fn parse_time(t: &str) -> Option<(u32, u32)> {
    let t = t.trim();
    // am/pm forms without a colon: "2am", "12pm".
    for (suffix, pm) in [("am", false), ("pm", true)] {
        if let Some(h) = t.strip_suffix(suffix) {
            let h: u32 = h.trim().parse().ok()?;
            if h > 12 {
                return None;
            }
            let hour = match (h, pm) {
                (12, false) => 0, // 12am = 00:00
                (12, true) => 12, // 12pm = 12:00
                (h, false) => h,
                (h, true) => h + 12,
            };
            return Some((hour, 0));
        }
    }
    // HH:MM (24-hour), optionally with an am/pm suffix on the minute.
    let (h_str, m_rest) = t.split_once(':')?;
    let hour: u32 = h_str.trim().parse().ok()?;
    let (m_str, pm) = if let Some(m) = m_rest.strip_suffix("am") {
        (m, Some(false))
    } else if let Some(m) = m_rest.strip_suffix("pm") {
        (m, Some(true))
    } else {
        (m_rest, None)
    };
    let minute: u32 = m_str.trim().parse().ok()?;
    if minute > 59 {
        return None;
    }
    let hour = match pm {
        Some(false) if hour == 12 => 0,
        Some(true) if hour != 12 => hour + 12,
        _ => hour,
    };
    if hour > 23 {
        return None;
    }
    Some((hour, minute))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_phrases_compile() {
        assert_eq!(compile_phrase("every 15 minutes").unwrap(), "*/15 * * * *");
        assert_eq!(compile_phrase("every 5 minutes").unwrap(), "*/5 * * * *");
        // Singular "minute" + n=1 (the "every 1 minute" demo example) must compile, not reject.
        assert_eq!(compile_phrase("every 1 minute").unwrap(), "*/1 * * * *");
        assert_eq!(compile_phrase("every 2 hours").unwrap(), "0 */2 * * *");
        assert_eq!(compile_phrase("hourly").unwrap(), "0 * * * *");
        assert_eq!(compile_phrase("every hour").unwrap(), "0 * * * *");
        assert_eq!(compile_phrase("daily").unwrap(), "0 0 * * *");
        assert_eq!(compile_phrase("weekdays at 08:00").unwrap(), "0 8 * * 1-5");
        assert_eq!(compile_phrase("daily at 02:00").unwrap(), "0 2 * * *");
        assert_eq!(compile_phrase("every day at 2am").unwrap(), "0 2 * * *");
        assert_eq!(compile_phrase("every day at 2pm").unwrap(), "0 14 * * *");
        assert_eq!(compile_phrase("mondays at 9:30").unwrap(), "30 9 * * 1");
    }

    #[test]
    fn twelve_am_pm_edges() {
        assert_eq!(compile_phrase("daily at 12am").unwrap(), "0 0 * * *");
        assert_eq!(compile_phrase("daily at 12pm").unwrap(), "0 12 * * *");
    }

    #[test]
    fn unparseable_phrase_is_an_error() {
        assert!(matches!(
            compile_phrase("whenever the mood strikes"),
            Err(ScheduleError::Unparseable(_))
        ));
        assert!(matches!(compile_phrase("every 0 minutes"), Err(_)));
        assert!(matches!(compile_phrase("every 90 minutes"), Err(_)));
    }

    #[test]
    fn extract_phrase_directive() {
        let body = "#[schedule(\"every 15 minutes\")]\n\nlet x = source(\"a\");";
        let sched = extract_schedule(body).unwrap().unwrap();
        assert_eq!(sched.raw, "every 15 minutes");
        assert_eq!(sched.cron, "*/15 * * * *");
    }

    #[test]
    fn extract_explicit_cron_passthrough() {
        let body = "#[schedule(cron = \"0 2 * * *\")]\nlet x = 1;";
        let sched = extract_schedule(body).unwrap().unwrap();
        assert_eq!(sched.raw, "0 2 * * *");
        assert_eq!(sched.cron, "0 2 * * *");
    }

    #[test]
    fn no_directive_is_none() {
        assert_eq!(extract_schedule("let x = 1;").unwrap(), None);
        // The token appearing INSIDE rule logic (not at a line's start as a directive) is ignored.
        assert_eq!(
            extract_schedule("let s = \"#[schedule(x)] is not a directive\";")
                .unwrap()
                .map(|_| ()),
            None
        );
    }

    #[test]
    fn unparseable_directive_is_a_save_error() {
        let body = "#[schedule(\"whenever\")]\nlet x = 1;";
        assert!(matches!(
            extract_schedule(body),
            Err(ScheduleError::Unparseable(_))
        ));
    }

    #[test]
    fn double_directive_is_malformed() {
        let body = "#[schedule(\"hourly\")]\n#[schedule(\"daily\")]\nlet x=1;";
        assert!(matches!(
            extract_schedule(body),
            Err(ScheduleError::Malformed(_))
        ));
    }

    #[test]
    fn directive_line_is_stripped_before_the_cage() {
        // Regression (debugging/rules/scheduled-rule-directive-breaks-cage): the stored body keeps the
        // directive (source of truth), but the executed body must NOT — `#` is reserved in rhai and an
        // un-stripped directive is a run-time compile error ("'#' is a reserved symbol").
        let body = "#[schedule(\"every 15 minutes\")]\n\ninsight.raise(#{ title: \"t\" });";
        let stripped = strip_directive(body);
        assert!(
            !stripped.contains("#[schedule"),
            "the directive line must be gone"
        );
        assert!(
            stripped.contains("insight.raise"),
            "the rule logic is preserved"
        );
        // No directive ⇒ zero-copy borrow (no needless allocation on the common path).
        assert!(matches!(
            strip_directive("let x = 1;"),
            std::borrow::Cow::Borrowed(_)
        ));
    }

    #[test]
    fn empty_or_malformed_directive_errors() {
        assert!(matches!(
            extract_schedule("#[schedule()]"),
            Err(ScheduleError::Malformed(_))
        ));
        assert!(matches!(
            extract_schedule("#[schedule(cron = )]"),
            Err(ScheduleError::Malformed(_))
        ));
    }
}
