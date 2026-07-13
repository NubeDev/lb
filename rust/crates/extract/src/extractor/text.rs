//! The plain-text / markdown passthrough extractor (doc-extraction scope). `text/plain` and
//! `text/markdown` are already the output format, so extraction is a UTF-8 decode + a title guess.
//! Not a no-op: it normalizes to the one contract (a valid-UTF-8 markdown body) and rejects
//! non-UTF-8 bytes honestly as `Failed`, so a mislabeled binary doesn't become a mojibake doc.

use crate::error::ExtractError;
use crate::model::{ExtractOpts, ExtractedDoc};
use crate::trait_def::Extractor;

/// Passthrough for text/markdown — decode UTF-8, title from the first non-empty line.
pub struct TextExtractor;

impl Extractor for TextExtractor {
    fn id(&self) -> &'static str {
        "text"
    }

    fn version(&self) -> u32 {
        1
    }

    fn extract(
        &self,
        bytes: &[u8],
        _mime: &str,
        _opts: &ExtractOpts,
    ) -> Result<Vec<ExtractedDoc>, ExtractError> {
        let text = std::str::from_utf8(bytes)
            .map_err(|e| ExtractError::failed(format!("not valid UTF-8 text: {e}")))?;
        Ok(vec![ExtractedDoc::whole(title_from(text), text)])
    }
}

/// The first non-empty line (trimmed, stripped of a leading markdown `#`), capped, as the title
/// hint. Empty input → an empty hint (the host falls back to the caller title or the media id).
pub(crate) fn title_from(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(|l| l.trim_start_matches('#').trim())
        .map(|l| l.chars().take(120).collect::<String>())
        .unwrap_or_default()
}
