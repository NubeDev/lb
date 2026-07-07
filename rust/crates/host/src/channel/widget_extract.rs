//! Extract a widget envelope the agent emitted INSIDE its final answer text (channel-widgets slice —
//! the no-`channel.post` path for the AGENT DOCK). When the agent's answer carries a fenced
//! ```lb-widget code block whose body is a `rich_result` render-envelope JSON, the worker splits it
//! off: the envelope is posted as a separate `rich_result` channel item (rendered by the shipped
//! ResponseView), and the block is stripped from the answer text the durable `agent_result` persists.
//!
//! Why this exists: the dock conversation is internally a channel (`dock-{user}-{id}`), so a widget
//! must land there to render. But forcing the AGENT to call `channel.post` as a tool burns turns on
//! arg schemas and cid discovery (the live 13-turn `missing arg: cid` run, the wrong-cid run). The
//! worker already KNOWS the cid (it owns the run). Letting the model author the widget as a fenced
//! block in its answer — and the worker split + post it — removes the agent-facing tool entirely.
//! The dock's live-refresh path (`useChannel` SSE merge) handles delivery; no UI change.
//!
//! The gate is the SAME one `channel::post` runs (`check_rich_result_genui`): a malformed IR is
//! left in the answer text (so the user sees the agent's broken attempt) and no widget item lands.
//! A valid block is normalized (a JSON-string `ir` rewritten) before it is posted, exactly as a
//! `channel.post` body is.

use super::error::ChannelError;
use super::genui_check::check_rich_result_genui;

/// The fenced-code info string the worker scans for. Picked to read naturally in markdown previews
/// (GitHub renders the JSON inside) and to name the contract (`lb-widget`). The model is taught this
/// exact fence in the channel-widgets skill.
pub(crate) const WIDGET_FENCE: &str = "lb-widget";

/// Split a widget envelope out of an agent's final answer text. Returns `(stripped_answer, body)`:
///   - `stripped_answer` — the answer with the fenced block removed (surrounding whitespace trimmed).
///   - `body`            — the validated envelope JSON to post as a `rich_result` channel item.
///
/// Returns `None` when the answer has no fenced ```lb-widget block, OR when the block fails the
/// `rich_result`/genui gate (the block is left in the answer in that case so the user sees what the
/// agent tried, instead of the widget silently vanishing). The first valid block wins; later ones
/// are left in place (a one-widget-per-answer contract — enforced by extraction, not by the model).
///
/// The returned body is the post-gate normalized form when the gate rewrote it (a JSON-string `ir`
/// → object), so the renderer always sees the real IR. A non-genui `rich_result` (e.g. `view:"table"`)
/// passes the gate unchanged and is returned verbatim.
pub(crate) fn extract_widget_block(answer: &str) -> Option<(String, String)> {
    let (before, body_text, after) = find_fenced_block(answer, WIDGET_FENCE)?;
    let validated = match check_rich_result_genui(body_text.trim()) {
        Ok(rewritten) => rewritten,
        Err(ChannelError::BadInput(_)) => {
            // A present-but-invalid block: leave the answer untouched so the user sees the agent's
            // broken attempt. The widget does NOT land — the model's broken IR is not silently dropped
            // on the floor, but neither does it pollute the dock as a raw JSON dump that renders broken.
            return None;
        }
        // A store/bus fault is impossible on a pure-in-memory parse — treat defensively as "no block".
        Err(_) => return None,
    };
    let body = validated.unwrap_or_else(|| body_text.trim().to_string());
    // Belt-and-braces: the gate permits only a `rich_result` envelope to be posted as a widget; a
    // stray code block that parsed as chat (no `kind`) must not land as a "widget".
    let value: serde_json::Value = serde_json::from_str(&body).ok()?;
    if value.get("kind").and_then(serde_json::Value::as_str) != Some(super::payload::KIND_RICH_RESULT)
    {
        return None;
    }
    let stripped = format!("{before}{after}");
    let stripped = stripped.trim_end().to_string();
    Some((stripped, body))
}

/// Find the FIRST fenced code block whose info string names `info`. Returns `(text_before_block,
/// block_body, text_after_block)` as byte slices of `text`. `None` when no such block exists. The
/// parser is the CommonMark core: an opener line whose first non-whitespace is `` ```{info} `` (any
/// trailing info-string text allowed) and a closer line whose first non-whitespace is `` ``` ``.
fn find_fenced_block<'a>(text: &'a str, info: &str) -> Option<(&'a str, &'a str, &'a str)> {
    let opener = format!("```{info}");
    let bytes = text.as_bytes();
    let mut line_start = 0usize;
    while line_start < bytes.len() {
        let line_end = text[line_start..]
            .find('\n')
            .map(|p| line_start + p)
            .unwrap_or(bytes.len());
        let line = text[line_start..line_end].trim_start();
        if line.starts_with(opener.as_str()) {
            // Body starts at the next line.
            let body_start = if line_end < bytes.len() {
                line_end + 1
            } else {
                bytes.len()
            };
            // Scan for the closing fence on its own line.
            let mut j = body_start;
            while j < bytes.len() {
                let le = text[j..].find('\n').map(|p| j + p).unwrap_or(bytes.len());
                let l = text[j..le].trim_start();
                if l.starts_with("```") {
                    let body_end = j; // exclude the trailing newline so the body has none.
                    let after_close = if le < bytes.len() { le + 1 } else { bytes.len() };
                    let before = &text[..line_start];
                    let body = &text[body_start..body_end];
                    let after = &text[after_close.min(bytes.len())..];
                    return Some((before, body, after));
                }
                j = if le < bytes.len() { le + 1 } else { bytes.len() };
            }
            // Opener with no closer — not a block, leave the text untouched.
            return None;
        }
        line_start = if line_end < bytes.len() {
            line_end + 1
        } else {
            bytes.len()
        };
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn genui_body() -> &'static str {
        // Real catalog names; the skill's canonical minimal example.
        r#"{"kind":"rich_result","v":2,"view":"genui","options":{"genui":{"v":1,"ir":{
            "v":1,
            "surface":{"surfaceId":"s1","root":"root"},
            "components":{"root":{"id":"root","component":"stack","children":[]}}
        }}}}"#
    }

    fn table_body() -> &'static str {
        r#"{"kind":"rich_result","v":2,"view":"table",
            "source":{"tool":"store.query","args":{"sql":"SELECT 1"}},
            "tools":["store.query"]}"#
    }

    #[test]
    fn no_block_returns_none() {
        assert!(extract_widget_block("just prose, no widget").is_none());
        assert!(extract_widget_block("```json\n{}\n```").is_none()); // wrong info string
    }

    #[test]
    fn a_valid_genui_block_is_extracted_and_stripped() {
        let answer = format!(
            "Here is your widget:\n\n```lb-widget\n{}\n```\nLet me know if you want changes.",
            genui_body()
        );
        let (stripped, body) = extract_widget_block(&answer).expect("extracted");
        assert!(
            !stripped.contains("```lb-widget") && !stripped.contains(genui_body()),
            "the block must be stripped from the answer: {stripped}"
        );
        assert!(stripped.contains("Here is your widget:"));
        assert!(stripped.contains("Let me know"));
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["kind"], "rich_result");
        assert_eq!(v["view"], "genui");
    }

    #[test]
    fn a_table_envelope_is_also_extracted() {
        let answer = format!("```lb-widget\n{}\n```", table_body());
        let (stripped, body) = extract_widget_block(&answer).expect("extracted");
        assert!(stripped.is_empty(), "no surrounding text → empty stripped answer");
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["view"], "table");
    }

    #[test]
    fn a_string_ir_is_normalized_to_the_object() {
        // A JSON-encoded string `ir` is the most common LLM slip — the gate rewrites it to the object.
        let ir = r#"{"v":1,"surface":{"surfaceId":"s1","root":"root"},"components":{"root":{"id":"root","component":"stack","children":[]}}}"#;
        let body = format!(
            "{{\"kind\":\"rich_result\",\"v\":2,\"view\":\"genui\",\"options\":{{\"genui\":{{\"v\":1,\"ir\":{ir:?}}}}}}}",
            ir = ir
        );
        let answer = format!("```lb-widget\n{body}\n```");
        let (_stripped, body) = extract_widget_block(&answer).expect("extracted + normalized");
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(v["options"]["genui"]["ir"].is_object(), "ir must be the parsed object");
    }

    #[test]
    fn an_invalid_block_is_left_in_the_answer_and_no_widget_is_returned() {
        // The wrong IR dialect (the live 2026-07-06 defect): `type` instead of `component`.
        let bad = r#"{"kind":"rich_result","v":2,"view":"genui","options":{"genui":{"v":1,"ir":{
            "components":{"root":{"type":"stack"}}}}}}"#;
        let answer = format!("```lb-widget\n{bad}\n```");
        assert!(
            extract_widget_block(&answer).is_none(),
            "an invalid block must not yield a widget item"
        );
    }

    #[test]
    fn a_block_whose_body_is_not_a_rich_result_is_rejected() {
        // Looks like a widget fence but the body is some other JSON.
        let answer = "```lb-widget\n{\"kind\":\"query\",\"sql\":\"SELECT 1\"}\n```";
        assert!(extract_widget_block(answer).is_none());
    }

    #[test]
    fn opener_without_closer_returns_none_unchanged() {
        let answer = format!("```lb-widget\n{}", genui_body());
        assert!(extract_widget_block(&answer).is_none());
    }

    #[test]
    fn the_first_valid_block_wins() {
        let answer = format!(
            "```lb-widget\n{}\n```\nMore text\n\n```lb-widget\n{}\n```",
            genui_body(),
            table_body()
        );
        let (_stripped, body) = extract_widget_block(&answer).expect("extracted");
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["view"], "genui", "the first block (genui) wins");
    }

    #[test]
    fn fenced_block_with_extra_info_string_text_is_recognized() {
        // CommonMark allows trailing text in the info string (e.g. ```lb-widget title=foo).
        let answer = format!("```lb-widget rendered\n{}\n```", genui_body());
        assert!(extract_widget_block(&answer).is_some());
    }
}
