//! `parse` — the one relative time-range grammar (dashboard relative-time-range scope). An
//! expression is either an **endpoint** (resolves to an instant: `now`, `now-4h`, `now-1d/d`, an
//! ISO day, an ISO instant, a 13-digit epoch ms) or a **range token** (resolves to a whole window:
//! `today`, `this-month`, `last-3-months`, …). Range tokens are legal only in `from` with `to`
//! absent — [`super::resolve`] enforces that; this file only names what a string *is*.
//!
//! Every refusal names the offending token and the legal set — nothing defaults silently.

use chrono::{NaiveDate, NaiveDateTime};

/// An offset/snap unit, Grafana-compatible: `s m h d w M y` — lowercase `m` is minute, uppercase
/// `M` is month.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Second,
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Year,
}

impl Unit {
    fn from_letter(c: char) -> Option<Unit> {
        Some(match c {
            's' => Unit::Second,
            'm' => Unit::Minute,
            'h' => Unit::Hour,
            'd' => Unit::Day,
            'w' => Unit::Week,
            'M' => Unit::Month,
            'y' => Unit::Year,
            _ => return None,
        })
    }

    /// The long-form unit words the counted trailing windows use (`last-3-months`), singular or
    /// plural. `quarter` normalizes to three months at the call site.
    fn from_word(w: &str) -> Option<Unit> {
        Some(match w {
            "second" | "seconds" => Unit::Second,
            "minute" | "minutes" => Unit::Minute,
            "hour" | "hours" => Unit::Hour,
            "day" | "days" => Unit::Day,
            "week" | "weeks" => Unit::Week,
            "month" | "months" => Unit::Month,
            "year" | "years" => Unit::Year,
            _ => return None,
        })
    }
}

/// A calendar unit the whole-period tokens range over (`this-<unit>` / `last-<unit>` /
/// `next-<unit>`): `hour day week month quarter year`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalUnit {
    Hour,
    Day,
    Week,
    Month,
    Quarter,
    Year,
}

impl CalUnit {
    fn from_word(w: &str) -> Option<CalUnit> {
        Some(match w {
            "hour" => CalUnit::Hour,
            "day" => CalUnit::Day,
            "week" => CalUnit::Week,
            "month" => CalUnit::Month,
            "quarter" => CalUnit::Quarter,
            "year" => CalUnit::Year,
            _ => return None,
        })
    }
}

/// The base of an endpoint expression, before an optional snap.
#[derive(Debug, Clone, PartialEq)]
pub enum EndpointBase {
    /// `now`, optionally offset: `now-4h`, `now+1d`. Second/minute/hour offsets are absolute
    /// durations; day/week/month/year step the *local calendar* (calendar-aware, per the scope).
    Now { offset: Option<(i64, Unit)> },
    /// `yyyy-mm-dd` — midnight of that day **in the range timezone**.
    IsoDay(NaiveDate),
    /// An ISO instant WITH an offset (`2026-07-01T06:00:00Z`) — an absolute epoch ms.
    InstantFixed(i64),
    /// An ISO instant WITHOUT an offset (`2026-07-01T06:00:00`) — interpreted in the range tz.
    InstantLocal(NaiveDateTime),
    /// A 13-digit epoch-milliseconds literal.
    EpochMs(i64),
}

/// An endpoint: a base instant plus an optional Grafana snap suffix (`now-1d/d`). The snap floors
/// to the start of the unit in the range tz (weeks floor to Monday).
#[derive(Debug, Clone, PartialEq)]
pub struct Endpoint {
    pub base: EndpointBase,
    pub snap: Option<Unit>,
}

/// A range token — a whole window, legal only in `from` with `to` absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Window {
    Today,
    Yesterday,
    Tomorrow,
    /// `this-<unit>` — from the START of the calendar period containing now to now (`this-week` =
    /// Monday → now, `this-year` = 1 Jan → now).
    This(CalUnit),
    /// `last-<unit>` — the previous WHOLE calendar period (`last-month` = the previous calendar
    /// month; deliberately ≠ `last-1-month`).
    LastCal(CalUnit),
    /// `next-<unit>` — the next whole calendar period.
    Next(CalUnit),
    /// `last-<n>-<unit>s` / `last-<n><unit>` — a trailing window of `n` units **ending now**
    /// (`last-1-month` on 31 Mar starts 28 Feb — clamped calendar stepping).
    Trailing {
        n: u32,
        unit: Unit,
    },
}

/// A parsed expression: an endpoint instant or a whole-window range token.
#[derive(Debug, Clone, PartialEq)]
pub enum RangeExpr {
    Endpoint(Endpoint),
    Window(Window),
}

/// The legal set, named verbatim in every refusal so a caller is steered, never left guessing.
pub const LEGAL_SET: &str = "now, now±<n><unit> (units s m h d w M y; m=minute, M=month) with an \
optional /<unit> snap, an ISO day (yyyy-mm-dd), an ISO instant, a 13-digit epoch ms, or a range \
token: today, yesterday, tomorrow, this-<unit>, last-<unit>, next-<unit> (units hour day week \
month quarter year), last-<n>-<unit>s, last-<n><unit>";

/// Everything the grammar or the resolver refuses. Every message names the bad token and the legal
/// set — `std::error::Error`, so a downstream `anyhow` caller `?`s it with no glue.
#[derive(Debug, Clone, PartialEq)]
pub enum TimeRangeError {
    /// A token the grammar does not know.
    Unknown { which: &'static str, token: String },
    /// An empty `from`/`to`.
    Empty { which: &'static str },
    /// A range token in `from` with a `to` present — a whole window IS both ends.
    WindowWithTo { token: String },
    /// A range token in `to` — only endpoints are legal there.
    WindowInTo { token: String },
    /// A resolved range that ends before it starts.
    Inverted { from: String, to: String },
    /// An unknown IANA timezone name.
    BadTimezone { tz: String },
}

impl std::fmt::Display for TimeRangeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TimeRangeError::Unknown { which, token } => {
                write!(
                    f,
                    "unknown {which} expression {token:?} (expected {LEGAL_SET})"
                )
            }
            TimeRangeError::Empty { which } => {
                write!(f, "empty {which} expression (expected {LEGAL_SET})")
            }
            TimeRangeError::WindowWithTo { token } => write!(
                f,
                "range token {token:?} already names a whole window — it is legal only in `from` \
                 with `to` absent"
            ),
            TimeRangeError::WindowInTo { token } => write!(
                f,
                "range token {token:?} is not legal in `to` — only an endpoint (now±…, an ISO \
                 day/instant, an epoch ms) is"
            ),
            TimeRangeError::Inverted { from, to } => {
                write!(f, "range ends before it starts (from {from}, to {to})")
            }
            TimeRangeError::BadTimezone { tz } => {
                write!(f, "unknown timezone {tz:?} (expected an IANA name like Australia/Sydney, or empty/UTC)")
            }
        }
    }
}

impl std::error::Error for TimeRangeError {}

/// Parse one expression. `which` names the slot (`"from"` / `"to"`) so the refusal reads right.
pub fn parse_expr(which: &'static str, s: &str) -> Result<RangeExpr, TimeRangeError> {
    let s = s.trim();
    if s.is_empty() {
        return Err(TimeRangeError::Empty { which });
    }
    if let Some(e) = parse_now(s) {
        return Ok(RangeExpr::Endpoint(e));
    }
    if let Some(w) = parse_window(s) {
        return Ok(RangeExpr::Window(w));
    }
    if let Some(e) = parse_literal(s) {
        return Ok(RangeExpr::Endpoint(e));
    }
    Err(TimeRangeError::Unknown {
        which,
        token: s.to_string(),
    })
}

/// The public single-expression parse (`from`-flavoured refusals).
pub fn parse(s: &str) -> Result<RangeExpr, TimeRangeError> {
    parse_expr("from", s)
}

/// `now`, `now±<n><unit>`, with an optional `/<unit>` snap on any of them (`now/d`, `now-1d/d`).
fn parse_now(s: &str) -> Option<Endpoint> {
    let rest = s.strip_prefix("now")?;
    let (rest, snap) = match rest.rsplit_once('/') {
        Some((head, tail)) => {
            let mut chars = tail.chars();
            let unit = Unit::from_letter(chars.next()?)?;
            if chars.next().is_some() {
                return None;
            }
            (head, Some(unit))
        }
        None => (rest, None),
    };
    let offset = if rest.is_empty() {
        None
    } else {
        let sign: i64 = match rest.chars().next()? {
            '+' => 1,
            '-' => -1,
            _ => return None,
        };
        let body = &rest[1..];
        let unit = Unit::from_letter(body.chars().last()?)?;
        let digits = &body[..body.len() - 1];
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let n: i64 = digits.parse().ok()?;
        Some((sign * n, unit))
    };
    Some(Endpoint {
        base: EndpointBase::Now { offset },
        snap,
    })
}

/// The range tokens: `today`/`yesterday`/`tomorrow`, `this-`/`last-`/`next-<calendar unit>`, and
/// the counted trailing forms `last-<n>-<unit>s` / `last-<n><unit>`.
fn parse_window(s: &str) -> Option<Window> {
    match s {
        "today" => return Some(Window::Today),
        "yesterday" => return Some(Window::Yesterday),
        "tomorrow" => return Some(Window::Tomorrow),
        _ => {}
    }
    if let Some(u) = s.strip_prefix("this-").and_then(CalUnit::from_word) {
        return Some(Window::This(u));
    }
    if let Some(u) = s.strip_prefix("next-").and_then(CalUnit::from_word) {
        return Some(Window::Next(u));
    }
    let rest = s.strip_prefix("last-")?;
    if let Some(u) = CalUnit::from_word(rest) {
        return Some(Window::LastCal(u));
    }
    // Counted: `last-<n>-<unit>s` (long) or `last-<n><letter>` (short). n ≥ 1.
    let split = rest.find(|c: char| !c.is_ascii_digit())?;
    if split == 0 {
        return None;
    }
    let n: u32 = rest[..split].parse().ok()?;
    if n == 0 {
        return None;
    }
    let tail = &rest[split..];
    let (n, unit) = if let Some(word) = tail.strip_prefix('-') {
        match word {
            // A quarter is three calendar months — same clamped stepping.
            "quarter" | "quarters" => (n.checked_mul(3)?, Unit::Month),
            _ => (n, Unit::from_word(word)?),
        }
    } else {
        let mut chars = tail.chars();
        let unit = Unit::from_letter(chars.next()?)?;
        if chars.next().is_some() {
            return None;
        }
        (n, unit)
    };
    Some(Window::Trailing { n, unit })
}

/// The literal endpoints: an ISO day, an ISO instant (with or without an offset), a 13-digit epoch ms.
fn parse_literal(s: &str) -> Option<Endpoint> {
    if s.len() == 13 && s.bytes().all(|b| b.is_ascii_digit()) {
        return Some(Endpoint {
            base: EndpointBase::EpochMs(s.parse().ok()?),
            snap: None,
        });
    }
    if s.contains('T') {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
            return Some(Endpoint {
                base: EndpointBase::InstantFixed(dt.timestamp_millis()),
                snap: None,
            });
        }
        let naive = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
            .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M"))
            .ok()?;
        return Some(Endpoint {
            base: EndpointBase::InstantLocal(naive),
            snap: None,
        });
    }
    if s.len() == 10 {
        if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
            return Some(Endpoint {
                base: EndpointBase::IsoDay(d),
                snap: None,
            });
        }
    }
    None
}
