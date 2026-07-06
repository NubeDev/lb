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
use crate::dashboard::genui::check_genui_block;

/// Reject a `kind:"rich_result"`, `view:"genui"` item body whose `options.genui` block is missing or
/// structurally invalid. Anything that is not a genui rich_result is `Ok` — this gate never touches
/// chat or other payloads. Unlike a dashboard draft, a posted preview with no IR is an error: the
/// whole point of the post is a rendered widget.
pub(crate) fn check_rich_result_genui(body: &str) -> Result<(), ChannelError> {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return Ok(()); // chat, not a payload
    };
    if value.get("kind").and_then(Value::as_str) != Some(KIND_RICH_RESULT)
        || value.get("view").and_then(Value::as_str) != Some("genui")
    {
        return Ok(());
    }
    let genui = value
        .get("options")
        .and_then(|o| o.get("genui"))
        .ok_or_else(|| {
            ChannelError::BadInput(
                "genui rich_result: missing `options.genui = { v, ir }` block".to_string(),
            )
        })?;
    check_genui_block(genui)
        .map_err(|msg| ChannelError::BadInput(format!("genui rich_result: {msg}")))
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
    }

    #[test]
    fn a_valid_genui_rich_result_passes() {
        assert!(check_rich_result_genui(&valid_body()).is_ok());
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
