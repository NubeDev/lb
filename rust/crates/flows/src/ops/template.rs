//! The `template` op — a **mustache-lite** text renderer (data-nodes Data category). Builds a string
//! (a body / topic / small JSON doc) from `payload` fields, with **no templating engine** (Risk 4: "a
//! heavy templating engine is a smell — a mustache-lite is enough"). The grammar is exactly one form:
//! `{{ dot.path }}` substitutions resolved against the `payload` via the shared [`super::path`] walker
//! (Q4: the existing walker, not a new one). No sections, no partials, no logic — a `{{path}}` that
//! resolves to a missing value renders the empty string (Node-RED parity); a value that is a
//! string renders verbatim, any other JSON value renders as compact JSON.
//!
//! `config = { template: "<string with {{path}} holes>" }`. The whole `payload` is the render scope;
//! `{{payload}}` (or any deeper path) addresses it. Pure; never fails (a template is just text).

use serde_json::Value;

use super::path;

/// Render `config.template` against `payload`, substituting each `{{ dot.path }}` hole. A missing
/// path → empty string; a string value → itself; any other value → compact JSON.
pub fn render(config: &Value, payload: &Value) -> Result<Value, String> {
    let tmpl = config
        .get("template")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    Ok(Value::String(substitute(tmpl, payload)))
}

/// Replace every `{{...}}` hole in `tmpl`. Unclosed `{{` past the end is emitted verbatim (a lone
/// brace pair is not a hole). The path inside a hole is trimmed, then walked from the `payload` root;
/// a bare `payload` addresses the whole message value.
fn substitute(tmpl: &str, payload: &Value) -> String {
    let mut out = String::with_capacity(tmpl.len());
    let mut rest = tmpl;
    while let Some(open) = rest.find("{{") {
        out.push_str(&rest[..open]);
        let after = &rest[open + 2..];
        let Some(close) = after.find("}}") else {
            // No closing braces — emit the remainder verbatim (not a hole).
            out.push_str(&rest[open..]);
            return out;
        };
        let raw = after[..close].trim();
        out.push_str(&resolve_hole(raw, payload));
        rest = &after[close + 2..];
    }
    out.push_str(rest);
    out
}

/// Resolve one hole's path against `payload`. `payload` (or `payload.<path>`) addresses into the
/// message; a leading `payload.` is stripped so `{{payload.temp}}` and `{{temp}}` mean the same field.
fn resolve_hole(raw: &str, payload: &Value) -> String {
    let p = raw.strip_prefix("payload.").unwrap_or(raw);
    let value = if p == "payload" || p.is_empty() {
        payload.clone()
    } else {
        path::get(payload, p)
    };
    match value {
        Value::Null => String::new(),
        Value::String(s) => s,
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn substitutes_fields() {
        let cfg = json!({"template": "temp={{temp}}C at {{site.name}}"});
        let out = render(&cfg, &json!({"temp": 21, "site": {"name": "roof"}})).unwrap();
        assert_eq!(out, json!("temp=21C at roof"));
    }

    #[test]
    fn missing_path_renders_empty() {
        let cfg = json!({"template": "x={{nope}}!"});
        assert_eq!(render(&cfg, &json!({})).unwrap(), json!("x=!"));
    }

    #[test]
    fn payload_addresses_whole_value_and_prefix_is_optional() {
        let cfg = json!({"template": "{{payload.a}}/{{a}}"});
        assert_eq!(render(&cfg, &json!({"a": "z"})).unwrap(), json!("z/z"));
    }

    #[test]
    fn non_string_value_renders_as_json() {
        let cfg = json!({"template": "list={{xs}}"});
        assert_eq!(
            render(&cfg, &json!({"xs": [1, 2]})).unwrap(),
            json!("list=[1,2]")
        );
    }

    #[test]
    fn unclosed_hole_is_verbatim() {
        let cfg = json!({"template": "a {{b"});
        assert_eq!(render(&cfg, &json!({})).unwrap(), json!("a {{b"));
    }
}
