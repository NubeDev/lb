//! Validate a `view:"genui"` `rich_result` body at POST time (channel-widgets scope). The dashboard
//! already rejects a malformed genui cell loudly at `dashboard.save` (`dashboard/genui.rs`) — but a
//! widget PREVIEW posted into a conversation never touches `dashboard.save`, so before this check an
//! agent could post any IR dialect it hallucinated and the dock would render the invalid/draft state
//! silently. Same authority, same seam: the host is the boundary; a loud `BadInput` here feeds back
//! into the agent loop as the tool error, and the model self-corrects (exactly the behavior observed
//! with `check_genui_cells` on the save path).
//!
//! Runs on EVERY post path (WS/HTTP/MCP all funnel through `channel::post`), on the parsed payload
//! only: a non-JSON chat body, a non-`rich_result` kind, or a non-genui view all pass untouched.

use serde_json::Value;

use super::error::ChannelError;
use super::payload::KIND_RICH_RESULT;
use crate::dashboard::genui::{check_genui_block, normalize_genui_block};

/// Reject a `kind:"rich_result"`, `view:"genui"` item body whose `options.genui` block is missing or
/// structurally invalid. Anything that is not a genui rich_result is `Ok(None)` — this gate never
/// touches chat or other payloads. Unlike a dashboard draft, a posted preview with no IR is an error:
/// the whole point of the post is a rendered widget.
///
/// Lenient-args normalization first (the `typed_arg` precedent): a JSON-STRING `ir` that parses to an
/// object is rewritten in place — `Ok(Some(body))` returns the normalized body the caller must persist
/// (so the renderer sees the real IR, not a quoted blob).
pub(crate) fn check_rich_result_genui(body: &str) -> Result<Option<String>, ChannelError> {
    let mut value = match serde_json::from_str::<Value>(body) {
        Ok(v) => v,
        Err(e) => {
            // An ATTEMPTED payload that isn't valid JSON must NOT slip through as chat: a live model
            // emitted a rich_result with one missing closing brace — it landed as plain text and the
            // dock rendered raw JSON (2026-07-07). Chat is anything that doesn't look like a payload;
            // a `{`-leading body naming `"kind"` is an envelope with broken JSON — reject with the
            // parser's position so the model can fix it.
            let t = body.trim_start();
            if t.starts_with('{') && t.contains("\"kind\"") {
                return Err(ChannelError::BadInput(format!(
                    "body is not valid JSON ({e}) — the `body` arg must be ONE well-formed JSON \
                     string; check for an unbalanced brace/bracket"
                )));
            }
            return Ok(None); // chat, not a payload
        }
    };
    if value.get("kind").and_then(Value::as_str) != Some(KIND_RICH_RESULT)
        || value.get("view").and_then(Value::as_str) != Some("genui")
    {
        return Ok(None);
    }
    let genui = value
        .get_mut("options")
        .and_then(|o| o.get_mut("genui"))
        .ok_or_else(|| {
            ChannelError::BadInput(
                "genui rich_result: missing `options.genui = { v, ir }` block".to_string(),
            )
        })?;
    let normalized = normalize_genui_block(genui);
    check_genui_block(genui)
        .map_err(|msg| ChannelError::BadInput(format!("genui rich_result: {msg}")))?;
    Ok(normalized.then(|| value.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_body() -> String {
        // Uses real catalog names ("stack" root); mirrors the skill's canonical example.
        r#"{"kind":"rich_result","v":2,"view":"genui","options":{"genui":{"v":1,"ir":{
            "v":1,
            "surface":{"surfaceId":"s1","root":"root"},
            "components":{"root":{"id":"root","component":"stack","children":[]}}
        }}}}"#
            .to_string()
    }

    #[test]
    fn chat_and_non_genui_payloads_pass_untouched() {
        assert!(check_rich_result_genui("hello world").is_ok());
        assert!(check_rich_result_genui(r#"{"kind":"query","q":"x"}"#).is_ok());
        assert!(check_rich_result_genui(r#"{"kind":"rich_result","v":2,"view":"table"}"#).is_ok());
        // Broken JSON that does NOT look like a payload stays chat (a `{` smiley, a code snippet).
        assert!(check_rich_result_genui("{not json").is_ok());
    }

    // The 2026-07-07 live defect: a rich_result missing ONE closing brace parsed as "not JSON",
    // slipped through as chat, and rendered raw in the dock. An attempted payload with broken JSON
    // must be rejected with the parser's position, never silently landed as text.
    #[test]
    fn an_attempted_payload_with_broken_json_is_rejected_not_landed_as_chat() {
        let mut body = valid_body();
        body.pop(); // drop the final `}` — exactly the live defect
        let err = check_rich_result_genui(&body).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("not valid JSON"), "names the defect: {msg}");
        assert!(msg.contains("brace"), "names the likely fix: {msg}");
    }

    #[test]
    fn a_valid_genui_rich_result_passes_unrewritten() {
        assert_eq!(check_rich_result_genui(&valid_body()).unwrap(), None);
    }

    // Lenient-args: the live model kept sending `ir` as a JSON-ENCODED STRING and stalled on the
    // rejection. A string ir that parses to an object is normalized — the returned body carries the
    // real object and passes the same validation.
    #[test]
    fn a_json_string_ir_is_normalized_to_the_object_and_lands() {
        let ir = r#"{"v":1,"surface":{"surfaceId":"s1","root":"root"},"components":{"root":{"id":"root","component":"stack","children":[]}}}"#;
        let body = serde_json::json!({
            "kind": "rich_result", "v": 2, "view": "genui",
            "options": { "genui": { "v": 1, "ir": ir } },
        })
        .to_string();
        let rewritten = check_rich_result_genui(&body)
            .expect("normalizes and validates")
            .expect("body was rewritten");
        let v: Value = serde_json::from_str(&rewritten).unwrap();
        assert!(
            v["options"]["genui"]["ir"].is_object(),
            "ir must be the parsed object"
        );
        assert_eq!(v["options"]["genui"]["ir"]["surface"]["root"], "root");
    }

    // A string ir that parses to a BAD object still gets the precise structural error, not the
    // generic "must be an object".
    #[test]
    fn a_json_string_ir_with_a_bad_dialect_gets_the_precise_error() {
        let body = serde_json::json!({
            "kind": "rich_result", "v": 2, "view": "genui",
            "options": { "genui": { "v": 1, "ir": r#"{"v":1,"components":{"root":{"type":"stack"}}}"# } },
        })
        .to_string();
        let msg = check_rich_result_genui(&body).unwrap_err().to_string();
        assert!(
            msg.contains("`component`"),
            "precise dialect error expected: {msg}"
        );
    }

    // The live defect from the dock run: `"type"` instead of `"component"`, no per-component `id`,
    // no `surface`. Each must be named loudly so the tool error teaches the model the fix.
    #[test]
    fn the_wrong_ir_dialect_is_rejected_with_the_fix_named() {
        let body = r#"{"kind":"rich_result","v":2,"view":"genui","options":{"genui":{"v":1,"ir":{
            "v":1,"components":{"root":{"type":"stack"}}}}}}"#;
        let err = check_rich_result_genui(body).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("`component`"),
            "should name the field fix: {msg}"
        );
    }

    #[test]
    fn a_missing_genui_block_or_ir_is_rejected() {
        let no_block = r#"{"kind":"rich_result","v":2,"view":"genui"}"#;
        assert!(matches!(
            check_rich_result_genui(no_block),
            Err(ChannelError::BadInput(_))
        ));
        let no_ir = r#"{"kind":"rich_result","v":2,"view":"genui","options":{"genui":{"v":1}}}"#;
        assert!(matches!(
            check_rich_result_genui(no_ir),
            Err(ChannelError::BadInput(_))
        ));
    }

    #[test]
    fn a_missing_surface_is_rejected_naming_the_shape() {
        let body = r#"{"kind":"rich_result","v":2,"view":"genui","options":{"genui":{"v":1,"ir":{
            "v":1,"components":{"root":{"id":"root","component":"stack"}}}}}}"#;
        let msg = check_rich_result_genui(body).unwrap_err().to_string();
        assert!(
            msg.contains("surfaceId"),
            "should name the surface shape: {msg}"
        );
    }
}
