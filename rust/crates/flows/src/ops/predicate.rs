//! The one shared **predicate** the routing/gating/RBE nodes consume (data-nodes scope, Risk 5). A
//! predicate is `{ op, value? }` evaluated against a `payload` sub-value addressed by [`super::path`].
//! `switch` (route on match), `filter` (deadband RBE), and any future condition node all call
//! [`eval`] — one operator vocabulary, never four bespoke matchers.
//!
//! The operator set is deliberately small (no query language — a data-nodes non-goal): `eq`/`neq`,
//! the numeric orderings `lt`/`lte`/`gt`/`gte`, membership `contains`/`in`, `truthy`/`falsy`,
//! `exists`/`missing`, and the always-true `else` (the switch fallthrough port). Numeric compares
//! coerce both sides to `f64`; a non-numeric operand on an ordering op is `false` (never a panic).

use serde_json::Value;

/// Evaluate `op` comparing the addressed `lhs` value against the rule's `operand`. Unknown ops are
/// `false` (a mistyped rule never silently matches). `else` ignores both operands (the fallthrough).
pub fn eval(op: &str, lhs: &Value, operand: &Value) -> bool {
    match op {
        "else" | "true" | "always" => true,
        "eq" => lhs == operand,
        "neq" => lhs != operand,
        "lt" => num_cmp(lhs, operand).map(|o| o.is_lt()).unwrap_or(false),
        "lte" => num_cmp(lhs, operand).map(|o| o.is_le()).unwrap_or(false),
        "gt" => num_cmp(lhs, operand).map(|o| o.is_gt()).unwrap_or(false),
        "gte" => num_cmp(lhs, operand).map(|o| o.is_ge()).unwrap_or(false),
        "truthy" => is_truthy(lhs),
        "falsy" => !is_truthy(lhs),
        "exists" => !lhs.is_null(),
        "missing" => lhs.is_null(),
        "contains" => contains(lhs, operand),
        // `in`: the addressed value is one of the operand array's elements (the reverse of contains).
        "in" => match operand {
            Value::Array(a) => a.iter().any(|e| e == lhs),
            _ => false,
        },
        _ => false,
    }
}

/// Whether `haystack` contains `needle`: substring for strings, membership for arrays, key-presence
/// for objects. A mismatched pair is `false`.
fn contains(haystack: &Value, needle: &Value) -> bool {
    match (haystack, needle) {
        (Value::String(s), Value::String(n)) => s.contains(n.as_str()),
        (Value::Array(a), n) => a.iter().any(|e| e == n),
        (Value::Object(m), Value::String(k)) => m.contains_key(k.as_str()),
        _ => false,
    }
}

/// JS-ish truthiness for the `truthy`/`falsy` ops: `false`/`null`/`0`/`""`/`[]`/`{}` are falsy.
pub fn is_truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(m) => !m.is_empty(),
    }
}

/// Coerce both sides to `f64` and compare. `None` if either side is not numeric (an ordering op on a
/// non-number is a non-match, not an error).
pub fn num_cmp(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
    let x = as_f64(a)?;
    let y = as_f64(b)?;
    x.partial_cmp(&y)
}

/// Best-effort numeric coercion: a JSON number, or a numeric string (so a `"512"` from a CSV parse
/// compares numerically). Booleans and null are not numbers here.
pub fn as_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn equality_and_ordering() {
        assert!(eval("eq", &json!(5), &json!(5)));
        assert!(eval("neq", &json!(5), &json!(6)));
        assert!(eval("gt", &json!(10), &json!(3)));
        assert!(eval("lte", &json!(3), &json!(3)));
        // numeric string coerces
        assert!(eval("gt", &json!("512"), &json!(100)));
        // ordering on a non-number is a non-match, never a panic
        assert!(!eval("gt", &json!("abc"), &json!(1)));
    }

    #[test]
    fn membership_and_truthiness() {
        assert!(eval("contains", &json!("hello world"), &json!("world")));
        assert!(eval("contains", &json!([1, 2, 3]), &json!(2)));
        assert!(eval("in", &json!("b"), &json!(["a", "b"])));
        assert!(eval("truthy", &json!(1), &Value::Null));
        assert!(eval("falsy", &json!(0), &Value::Null));
        assert!(eval("falsy", &json!([]), &Value::Null));
    }

    #[test]
    fn existence_and_else() {
        assert!(eval("exists", &json!(0), &Value::Null));
        assert!(eval("missing", &Value::Null, &Value::Null));
        assert!(eval("else", &json!("anything"), &json!("ignored")));
        assert!(!eval("unknown_op", &json!(1), &json!(1)));
    }
}
