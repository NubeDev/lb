//! A logo carried INLINE as a `data:` URI — the form a brand's `logo_asset_id` takes when a brand
//! editor stores a small logo on the record itself, and the form `ExportOptions::logo` (the
//! caller's logo of last resort) always takes.
//!
//! One responsibility: `data:<mime>;base64,<payload>` → the mime and the bytes, with the one limit
//! that keeps a request body from smuggling an arbitrarily large image into a Typst compile.

use base64::Engine as _;

use super::error::ReportError;

/// The largest inline logo an export accepts, decoded. Generous for a letterhead mark (a brand
/// editor caps its own uploads far lower) and small enough that it is never the size of a request.
pub(super) const MAX_INLINE_LOGO_BYTES: usize = 1024 * 1024;

/// The `(mime, bytes)` of an inline logo, or `None` when `value` is not a decodable base64
/// `data:` URI — an asset id, an empty string, a URL, a payload that does not decode.
pub(super) fn inline_logo(value: &str) -> Option<(String, Vec<u8>)> {
    let (meta, payload) = value.strip_prefix("data:")?.split_once(',')?;
    let mime = meta.strip_suffix(";base64")?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload.trim())
        .ok()?;
    Some((mime.to_owned(), bytes))
}

/// The caller's fallback logo (`options.logo`), validated. Empty ⇒ `None`.
///
/// # Errors
/// [`ReportError::BadInput`] naming `options.logo` when it is set but is not an image `data:` URI
/// that decodes, or decodes to more than [`MAX_INLINE_LOGO_BYTES`]. Loud on purpose: the caller
/// asked for a logo, and a PDF with a silent gap where it should be is the failure they cannot see.
pub(super) fn fallback_logo(value: &str) -> Result<Option<(String, Vec<u8>)>, ReportError> {
    if value.is_empty() {
        return Ok(None);
    }
    // The length guard runs on the ENCODED text, before decoding: base64 is 4 characters per 3
    // bytes, so anything past this cannot decode to within the limit and is not worth allocating.
    if value.len() > MAX_INLINE_LOGO_BYTES / 3 * 4 + 128 {
        return Err(ReportError::BadInput(format!(
            "options.logo is larger than the {} KiB an inline logo may be",
            MAX_INLINE_LOGO_BYTES / 1024
        )));
    }
    match inline_logo(value) {
        Some((mime, bytes)) if mime.starts_with("image/") => Ok(Some((mime, bytes))),
        _ => Err(ReportError::BadInput(
            "options.logo must be an image data: URI (data:image/...;base64,...)".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_inline_logo_is_decoded_and_anything_else_is_not_one() {
        let (mime, bytes) = inline_logo("data:image/png;base64,AAEC").unwrap();
        assert_eq!((mime.as_str(), bytes), ("image/png", vec![0, 1, 2]));
        assert!(inline_logo("asset-123").is_none());
        assert!(inline_logo("").is_none());
        assert!(inline_logo("data:image/png,rawtext").is_none());
    }

    #[test]
    fn the_fallback_is_absent_when_empty_and_refused_when_not_an_image() {
        assert!(fallback_logo("").unwrap().is_none());
        assert!(fallback_logo("data:image/svg+xml;base64,AAEC")
            .unwrap()
            .is_some());
        for bad in [
            "logo.png",
            "data:text/html;base64,AAEC",
            "data:image/png;base64,***",
        ] {
            assert!(format!("{:?}", fallback_logo(bad).unwrap_err()).contains("options.logo"));
        }
    }
}
