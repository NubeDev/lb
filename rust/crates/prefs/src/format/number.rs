//! `format.number(n, opts?)` — render a number with the resolved `number_format` axis's decimal +
//! grouping separators (prefs scope: `43,2` vs `43.2`). The separators come from the closed
//! [`NumberFormat`] axis, which encodes exactly the CLDR choices for the enabled locales — so the
//! rendering is locale-correct *and* deterministic, with no dependency on an external locale at
//! format time. (The richer icu `FixedDecimalFormatter` path — full per-locale digit shaping — is a
//! follow-up that swaps this impl behind the same signature; the axis is the contract.)
//!
//! Grouping is applied to the integer part in 3-digit groups; fractional digits are kept as given
//! (or rounded to `max_frac` when set). No locale round-trip, no I/O.

use crate::axis::NumberFormat;

/// Options for a number render. `max_frac` caps fractional digits (rounded half-away-from-zero);
/// `None` keeps the value's natural decimal expansion (trimmed of trailing zeros).
#[derive(Debug, Clone, Copy, Default)]
pub struct NumberOpts {
    pub max_frac: Option<u8>,
}

/// Render `n` per `fmt` and `opts`. Negative numbers keep a leading `-`.
pub fn format_number(n: f64, fmt: NumberFormat, opts: NumberOpts) -> String {
    let neg = n.is_sign_negative() && n != 0.0;
    let abs = n.abs();

    // Decide the fractional rendering first (so grouping sees the final integer part).
    let raw = match opts.max_frac {
        Some(p) => format!("{abs:.*}", p as usize),
        None => trim_trailing_zeros(format!("{abs}")),
    };
    let (int_part, frac_part) = match raw.split_once('.') {
        Some((i, f)) => (i.to_string(), Some(f.to_string())),
        None => (raw, None),
    };

    let grouped = group_thousands(&int_part, fmt.group_sep());
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    out.push_str(&grouped);
    if let Some(frac) = frac_part {
        if !frac.is_empty() {
            out.push_str(fmt.decimal_sep());
            out.push_str(&frac);
        }
    }
    out
}

/// Insert `sep` every three digits from the right of an integer-digit string.
fn group_thousands(int_digits: &str, sep: &str) -> String {
    let bytes = int_digits.as_bytes();
    let n = bytes.len();
    let mut out = String::with_capacity(n + n / 3 * sep.len());
    for (idx, ch) in int_digits.chars().enumerate() {
        if idx > 0 && (n - idx) % 3 == 0 {
            out.push_str(sep);
        }
        out.push(ch);
    }
    out
}

/// Drop trailing zeros (and a dangling `.`) from a default float render so `12.0` → `12`.
fn trim_trailing_zeros(s: String) -> String {
    if !s.contains('.') {
        return s;
    }
    let trimmed = s.trim_end_matches('0').trim_end_matches('.');
    trimmed.to_string()
}
