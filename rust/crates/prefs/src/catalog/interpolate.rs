//! Placeholder interpolation — routes a `{arg, fmt…}` to the **already-shipped `format::*`**
//! (i18n-catalogs scope: "Reuse, do not re-derive … this layer only routes placeholders to it").
//! A bare `{arg}` stringifies the JSON value; `{ts, date}` → `format::datetime`, `{n, number}` →
//! `format::number`, `{v, quantity, <dim>}` → `format::quantity` (converting from the dimension's
//! canonical unit into the recipient's display unit).
//!
//! **Failure contract (pinned):** if a typed placeholder's `format::*` call errors (null/out-of-range
//! `ts`, wrong value type, unknown dimension already rejected at parse), the renderer substitutes the
//! literal `[<arg>]` and continues — never a panic, never a blank, always a returned message.

use serde_json::Value;

use crate::axis::Dimension;
use crate::format::{format_datetime, format_number, format_quantity, NumberOpts};
use crate::prefs::ResolvedPrefs;

use super::message::Placeholder;

/// Render one placeholder against `args` and the `resolved` prefs. Never fails: any error path
/// yields the honest literal `[<arg>]` sentinel (the pinned failure contract).
pub fn render_placeholder(ph: &Placeholder, args: &Value, resolved: &ResolvedPrefs) -> String {
    match ph {
        Placeholder::Arg(name) => stringify(lookup(args, name)),
        Placeholder::Date(name) => match lookup(args, name).and_then(Value::as_i64) {
            Some(ms) => format_datetime(
                ms,
                &resolved.timezone,
                resolved.date_style,
                resolved.time_style,
            )
            .unwrap_or_else(|_| sentinel(name)),
            None => sentinel(name),
        },
        Placeholder::Number(name) => match lookup(args, name).and_then(Value::as_f64) {
            Some(n) => format_number(n, resolved.number_format, NumberOpts::default()),
            None => sentinel(name),
        },
        Placeholder::Quantity(name, dim) => render_quantity(name, *dim, args, resolved),
    }
}

/// The count token `#` inside a plural arm renders the plural number through `format::number` (scope
/// grammar note: "`#` renders the plural number via format::number").
pub fn render_count(n: i64, resolved: &ResolvedPrefs) -> String {
    format_number(n as f64, resolved.number_format, NumberOpts::default())
}

fn render_quantity(name: &str, dim: Dimension, args: &Value, resolved: &ResolvedPrefs) -> String {
    let Some(value) = lookup(args, name).and_then(Value::as_f64) else {
        return sentinel(name);
    };
    // The catalog carries the CANONICAL value (base unit of the dimension); convert to the display
    // unit the recipient's prefs pick. Reuses the shipped chart bridge verbatim.
    match format_quantity(
        value,
        dim.canonical_unit(),
        dim,
        resolved,
        NumberOpts::default(),
    ) {
        Ok(q) => q.text,
        Err(_) => sentinel(name),
    }
}

/// Look up `name` in the args object (a top-level flat map). `None` if args isn't an object or the
/// key is absent.
fn lookup<'a>(args: &'a Value, name: &str) -> Option<&'a Value> {
    args.as_object().and_then(|m| m.get(name))
}

/// The honest failure literal for `arg` — `[<arg>]` (pinned contract). Also the missing-arg render.
fn sentinel(arg: &str) -> String {
    format!("[{arg}]")
}

/// Stringify a bare-arg JSON value for text substitution: a string is inserted as-is (not quoted),
/// numbers/bools by their JSON form, a missing/null value as the `[<arg>]` sentinel would be too
/// aggressive for a plain arg — an explicit `null` renders empty, an *absent* key is the sentinel.
fn stringify(v: Option<&Value>) -> String {
    match v {
        None => String::new(), // a bare arg the caller simply didn't pass renders empty, not `[x]`.
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) => String::new(),
        Some(other) => other.to_string(),
    }
}
