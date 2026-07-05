//! `strip_think` — remove `<think>…</think>` reasoning blocks from a completion's content.
//!
//! Several OpenAI-compatible models (notably Z.AI GLM) emit their chain-of-thought inline in the
//! message `content`, wrapped in `<think>…</think>`, ahead of the real answer. That reasoning is not
//! the answer — leaked into a channel message or a `dashboard`-authoring turn it reads as broken
//! ("`</think>` Let me try…"). The chat-completions wire has no separate reasoning channel we can
//! rely on across backends, so we strip the block from `content` at the adapter boundary — the one
//! place every OpenAI-compatible turn passes through.
//!
//! Conservative by design: strip only well-formed `<think>…</think>` pairs (case-insensitive, across
//! newlines). An unterminated `<think>` with no close is left untouched (better a visible tag than a
//! swallowed answer). Content with no think block is returned unchanged.

/// Strip every `<think>…</think>` block (case-insensitive, multiline) and trim surrounding blank
/// space. Returns the answer the user should see.
pub fn strip_think(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    let mut rest = content;
    loop {
        // Find the next case-insensitive `<think>` open tag.
        let Some(open) = find_ci(rest, "<think>") else {
            out.push_str(rest);
            break;
        };
        // Everything before the open tag is kept.
        out.push_str(&rest[..open]);
        let after_open = &rest[open + "<think>".len()..];
        // A matching close tag — else leave the unterminated remainder verbatim (don't swallow it).
        match find_ci(after_open, "</think>") {
            Some(close) => rest = &after_open[close + "</think>".len()..],
            None => {
                out.push_str(&rest[open..]);
                break;
            }
        }
    }
    out.trim().to_string()
}

/// Case-insensitive substring search returning the byte offset of the first match. `needle` is ASCII
/// (`<think>` / `</think>`), so a byte-wise ASCII-lowercased compare is correct and allocation-light.
fn find_ci(haystack: &str, needle: &str) -> Option<usize> {
    let hay = haystack.as_bytes();
    let need = needle.as_bytes();
    if need.is_empty() || hay.len() < need.len() {
        return None;
    }
    (0..=hay.len() - need.len()).find(|&i| {
        hay[i..i + need.len()]
            .iter()
            .zip(need)
            .all(|(h, n)| h.eq_ignore_ascii_case(n))
    })
}

#[cfg(test)]
mod tests {
    use super::strip_think;

    #[test]
    fn strips_a_leading_think_block() {
        let got = strip_think("<think>let me plan this</think>The answer is 42.");
        assert_eq!(got, "The answer is 42.");
    }

    #[test]
    fn strips_multiline_and_case_insensitive() {
        let got = strip_think("<THINK>\nreasoning\nover lines\n</Think>\n\nDone.");
        assert_eq!(got, "Done.");
    }

    #[test]
    fn strips_multiple_blocks() {
        let got = strip_think("<think>a</think>one <think>b</think>two");
        assert_eq!(got, "one two");
    }

    #[test]
    fn leaves_plain_content_untouched() {
        assert_eq!(strip_think("just an answer"), "just an answer");
    }

    #[test]
    fn leaves_an_unterminated_think_verbatim() {
        // Better a visible tag than a swallowed answer — don't strip a half-open block.
        let got = strip_think("<think>never closes the answer");
        assert_eq!(got, "<think>never closes the answer");
    }
}
