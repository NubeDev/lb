//! The zip transport envelope for a signed [`Artifact`] — an alternative wire shape to the JSON one,
//! for the SAME struct (registry scope, extension-artifact-upload-size risk).
//!
//! The JSON wire shape encodes `Artifact::wasm: Vec<u8>` as a decimal-integer JSON array (no custom
//! serde impl on that field) — harmless for a small wasm module, but an ~8x size blowup for a real
//! native (Tier-2) sidecar binary running tens to hundreds of MB. That inflation broke both a browser
//! upload (building a hundred-million-element JS array) and a direct `curl` upload (past the
//! gateway's configured body ceiling) against a real 191 MB extension. This module fixes the
//! *transport* only: [`artifact_from_zip`]/[`artifact_to_zip`] convert between the exact same
//! [`Artifact`] the JSON path produces and a `.zip` container that carries the binary as a real,
//! uncompressed archive member instead of JSON text.
//!
//! Deliberately mirrors `lb_packs::zip::bundle_from_zip`'s shape (budget-guarded reads, noise-entry
//! filtering, `enclosed_name()` zip-slip guard) — same problem (untrusted zip in, structured Rust
//! value out), same answer.
//!
//! **No trust decision happens here.** `artifact_from_zip` is exactly as untrusted as
//! `serde_json::from_str::<Artifact>` — it only reconstructs the struct. The caller MUST still run
//! the result through `verify_artifact`/`verify_artifact_with` before anything is cached or
//! installed; a broken/adversarial *container* (this module's concern) is a different failure than a
//! tampered *signed claim* (the `verify` module's concern), which is why they report through
//! different [`RegistryError`] variants — [`RegistryError::Transport`] here, `Unverified`/`Malformed`
//! only from `verify`.

use std::io::{Read, Write};
use std::path::Path;

use crate::model::Artifact;
use crate::RegistryError;

/// The fixed entry name every artifact zip carries its payload bytes under. Tier-agnostic —
/// `Artifact::wasm` holds a wasm component or a native executable depending on the manifest's
/// `[runtime].tier`, so the unpacker never needs to consult the manifest before it knows what to look
/// for.
pub const PAYLOAD_ENTRY_NAME: &str = "payload.bin";

/// The ZIP spec's own hard ceiling on an end-of-central-directory comment: the field is length
/// prefixed with a `u16`. Asserted explicitly on pack rather than left to silently truncate/corrupt.
const MAX_COMMENT_BYTES: usize = u16::MAX as usize;

/// The non-payload `Artifact` fields, carried as JSON in the zip's EOCD comment. Same fields as the
/// JSON wire shape minus `wasm` (which lives in the one archive member instead), and `signature`
/// becomes `signature_hex` — hex, matching `digest_hex`'s existing convention right next to it,
/// rather than repeating the same decimal-array-of-bytes shape this whole module exists to avoid.
#[derive(serde::Serialize, serde::Deserialize)]
struct CommentMeta {
    ext_id: String,
    version: String,
    manifest_toml: String,
    digest_hex: String,
    publisher_key_id: String,
    signature_hex: String,
}

/// Unpack a zip-transport artifact into the SAME [`Artifact`] the JSON wire path produces.
///
/// Requires exactly one non-noise entry, named [`PAYLOAD_ENTRY_NAME`], written with `Stored`
/// compression (never `Deflated` — refused outright, so an entry's uncompressed size can never
/// exceed its own declared size: the zip-bomb inflation-ratio attack is closed by construction, not
/// merely budgeted). `max_payload_bytes` bounds how many bytes are read for that entry regardless of
/// what the zip's local header claims — a running-budget guard, same idiom as
/// `lb_packs::bundle_from_zip`'s — since a hostile central directory could still lie about size.
///
/// Every rejection names what was found, so a bad artifact is fixable rather than a guess.
pub fn artifact_from_zip(bytes: &[u8], max_payload_bytes: u64) -> Result<Artifact, RegistryError> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|e| RegistryError::Transport(format!("not a readable zip archive: {e}")))?;

    let comment = archive.comment();
    if comment.is_empty() {
        return Err(RegistryError::Transport(
            "archive has no EOCD comment — an artifact zip carries its signed metadata there"
                .into(),
        ));
    }
    let meta: CommentMeta = serde_json::from_slice(comment).map_err(|e| {
        RegistryError::Transport(format!(
            "EOCD comment is not the expected artifact metadata: {e}"
        ))
    })?;

    let mut payload: Option<Vec<u8>> = None;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| RegistryError::Transport(format!("unreadable member #{i}: {e}")))?;
        if entry.is_dir() || is_noise(entry.name()) {
            continue;
        }
        let name = entry.enclosed_name().ok_or_else(|| {
            RegistryError::Transport(format!(
                "archive member {:?} is not a safe relative path",
                entry.name()
            ))
        })?;
        if name != Path::new(PAYLOAD_ENTRY_NAME) {
            return Err(RegistryError::Transport(format!(
                "unexpected member {name:?} — an artifact zip carries exactly one \
                 `{PAYLOAD_ENTRY_NAME}` entry"
            )));
        }
        if payload.is_some() {
            return Err(RegistryError::Transport(format!(
                "more than one `{PAYLOAD_ENTRY_NAME}` entry"
            )));
        }
        if entry.compression() != zip::CompressionMethod::Stored {
            return Err(RegistryError::Transport(
                "payload entry must be stored (uncompressed) — deflate is refused so an entry \
                 cannot claim to inflate past its declared size"
                    .into(),
            ));
        }

        let mut buf = Vec::new();
        let read = entry
            .by_ref()
            .take(max_payload_bytes + 1)
            .read_to_end(&mut buf)
            .map_err(|e| RegistryError::Transport(format!("reading payload: {e}")))?;
        if read as u64 > max_payload_bytes {
            return Err(RegistryError::Transport(
                "payload exceeds the configured upload ceiling".into(),
            ));
        }
        payload = Some(buf);
    }
    let wasm = payload.ok_or_else(|| {
        RegistryError::Transport(format!("no `{PAYLOAD_ENTRY_NAME}` entry in the archive"))
    })?;

    let signature = hex_decode(&meta.signature_hex)
        .map_err(|e| RegistryError::Transport(format!("signature_hex: {e}")))?;

    Ok(Artifact {
        ext_id: meta.ext_id,
        version: meta.version,
        manifest_toml: meta.manifest_toml,
        wasm,
        digest_hex: meta.digest_hex,
        publisher_key_id: meta.publisher_key_id,
        signature,
    })
}

/// The inverse: pack an already-signed [`Artifact`] into the zip-transport container. Never
/// re-derives or re-signs anything — `artifact` is the caller's fully-signed value already; this
/// only re-encodes it as bytes. One `Stored` entry ([`PAYLOAD_ENTRY_NAME`]) carrying `artifact.wasm`
/// verbatim; every other field folded into the EOCD comment as JSON.
pub fn artifact_to_zip(artifact: &Artifact) -> Result<Vec<u8>, RegistryError> {
    let meta = CommentMeta {
        ext_id: artifact.ext_id.clone(),
        version: artifact.version.clone(),
        manifest_toml: artifact.manifest_toml.clone(),
        digest_hex: artifact.digest_hex.clone(),
        publisher_key_id: artifact.publisher_key_id.clone(),
        signature_hex: hex_encode(&artifact.signature),
    };
    let comment = serde_json::to_vec(&meta)
        .map_err(|e| RegistryError::Transport(format!("encoding artifact metadata: {e}")))?;
    if comment.len() > MAX_COMMENT_BYTES {
        return Err(RegistryError::Transport(format!(
            "artifact metadata ({} bytes) exceeds the {MAX_COMMENT_BYTES}-byte zip comment limit",
            comment.len()
        )));
    }

    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .large_file(artifact.wasm.len() as u64 > u32::MAX as u64);
    writer
        .start_file(PAYLOAD_ENTRY_NAME, opts)
        .map_err(|e| RegistryError::Transport(format!("starting payload entry: {e}")))?;
    writer
        .write_all(&artifact.wasm)
        .map_err(|e| RegistryError::Transport(format!("writing payload entry: {e}")))?;
    writer.set_raw_comment(comment.into_boxed_slice());
    let cursor = writer
        .finish()
        .map_err(|e| RegistryError::Transport(format!("finishing archive: {e}")))?;
    Ok(cursor.into_inner())
}

/// Archive members that are packaging noise, never a real payload — dropped before any other rule,
/// same idiom as `lb_packs::zip::is_noise`.
fn is_noise(name: &str) -> bool {
    name.starts_with("__MACOSX/")
        || name
            .split('/')
            .any(|seg| seg == ".DS_Store" || seg == "Thumbs.db")
}

/// Lowercase hex — same convention `digest_hex` already uses, applied here to `signature` so the
/// EOCD comment stays plain, inspectable JSON text instead of a decimal-integer array (the exact
/// shape this module exists to avoid).
fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from_digit((b >> 4) as u32, 16).unwrap());
        out.push(char::from_digit((b & 0xf) as u32, 16).unwrap());
    }
    out
}

fn hex_decode(s: &str) -> Result<Vec<u8>, String> {
    if !s.len().is_multiple_of(2) {
        return Err("hex length must be even".into());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_artifact(wasm: Vec<u8>) -> Artifact {
        Artifact {
            ext_id: "federation".into(),
            version: "0.1.0".into(),
            manifest_toml: "id = \"federation\"\nversion = \"0.1.0\"\n".into(),
            wasm,
            digest_hex: "a".repeat(64),
            publisher_key_id: "dev-publisher".into(),
            signature: vec![7u8; 64],
        }
    }

    const HUGE: u64 = u64::MAX / 2;

    #[test]
    fn round_trips_a_small_artifact() {
        let a = sample_artifact(b"\0asm\x01\x00\x00\x00".to_vec());
        let zip = artifact_to_zip(&a).expect("pack");
        let back = artifact_from_zip(&zip, HUGE).expect("unpack");
        assert_eq!(a, back);
    }

    #[test]
    fn round_trips_an_empty_payload() {
        let a = sample_artifact(Vec::new());
        let zip = artifact_to_zip(&a).expect("pack");
        let back = artifact_from_zip(&zip, HUGE).expect("unpack");
        assert_eq!(a, back);
    }

    #[test]
    fn is_dramatically_smaller_than_the_json_wire_shape() {
        // The exact defect being fixed: JSON's decimal-int-array encoding of `wasm` inflates
        // compact serde_json output ~4x (and lb-pack's actual pretty-printed output further —
        // ~8x, measured against a real 191 MB binary). A zip-transport artifact for the same
        // payload must stay close to 1x, so even this compact-JSON baseline comparison should
        // show the zip well under half the JSON size.
        let a = sample_artifact(vec![0xABu8; 5 * 1024 * 1024]);
        let json_len = serde_json::to_vec(&a).expect("json").len();
        let zip_len = artifact_to_zip(&a).expect("pack").len();
        assert!(
            zip_len < json_len / 3,
            "zip ({zip_len}) should be well under 1/3 of json ({json_len})"
        );
    }

    #[test]
    fn rejects_bytes_that_are_not_a_zip() {
        let err = artifact_from_zip(b"not a zip at all", HUGE).unwrap_err();
        assert!(matches!(err, RegistryError::Transport(_)));
    }

    #[test]
    fn rejects_an_archive_with_no_comment() {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        w.start_file(PAYLOAD_ENTRY_NAME, zip::write::SimpleFileOptions::default())
            .unwrap();
        w.write_all(b"hi").unwrap();
        let bytes = w.finish().unwrap().into_inner();
        let err = artifact_from_zip(&bytes, HUGE).unwrap_err();
        assert!(matches!(err, RegistryError::Transport(msg) if msg.contains("no EOCD comment")));
    }

    #[test]
    fn rejects_a_malformed_comment() {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        w.start_file(PAYLOAD_ENTRY_NAME, zip::write::SimpleFileOptions::default())
            .unwrap();
        w.write_all(b"hi").unwrap();
        w.set_raw_comment(b"not json".to_vec().into_boxed_slice());
        let bytes = w.finish().unwrap().into_inner();
        let err = artifact_from_zip(&bytes, HUGE).unwrap_err();
        assert!(matches!(err, RegistryError::Transport(msg) if msg.contains("metadata")));
    }

    #[test]
    fn rejects_a_missing_payload_entry() {
        let meta = CommentMeta {
            ext_id: "x".into(),
            version: "0.1.0".into(),
            manifest_toml: "id=\"x\"".into(),
            digest_hex: "a".repeat(64),
            publisher_key_id: "k".into(),
            signature_hex: "bb".repeat(64),
        };
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        w.start_file("not-payload.bin", zip::write::SimpleFileOptions::default())
            .unwrap();
        w.write_all(b"hi").unwrap();
        w.set_raw_comment(serde_json::to_vec(&meta).unwrap().into_boxed_slice());
        let bytes = w.finish().unwrap().into_inner();
        let err = artifact_from_zip(&bytes, HUGE).unwrap_err();
        assert!(matches!(err, RegistryError::Transport(msg) if msg.contains("unexpected member")));
    }

    #[test]
    fn rejects_a_deflated_payload_entry() {
        let meta = CommentMeta {
            ext_id: "x".into(),
            version: "0.1.0".into(),
            manifest_toml: "id=\"x\"".into(),
            digest_hex: "a".repeat(64),
            publisher_key_id: "k".into(),
            signature_hex: "bb".repeat(64),
        };
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        w.start_file(PAYLOAD_ENTRY_NAME, opts).unwrap();
        w.write_all(b"hello world hello world hello world").unwrap();
        w.set_raw_comment(serde_json::to_vec(&meta).unwrap().into_boxed_slice());
        let bytes = w.finish().unwrap().into_inner();
        let err = artifact_from_zip(&bytes, HUGE).unwrap_err();
        assert!(matches!(err, RegistryError::Transport(msg) if msg.contains("stored")));
    }

    #[test]
    fn rejects_a_payload_over_the_configured_ceiling() {
        let a = sample_artifact(vec![0u8; 1024]);
        let zip = artifact_to_zip(&a).expect("pack");
        let err = artifact_from_zip(&zip, 100).unwrap_err();
        assert!(matches!(err, RegistryError::Transport(msg) if msg.contains("ceiling")));
    }

    #[test]
    fn rejects_metadata_over_the_comment_limit() {
        let mut a = sample_artifact(vec![0u8; 8]);
        a.manifest_toml = "x".repeat(MAX_COMMENT_BYTES + 1);
        let err = artifact_to_zip(&a).unwrap_err();
        assert!(matches!(err, RegistryError::Transport(msg) if msg.contains("comment limit")));
    }

    #[test]
    fn ignores_macos_packaging_noise() {
        let a = sample_artifact(b"payload".to_vec());
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        w.start_file(PAYLOAD_ENTRY_NAME, opts).unwrap();
        w.write_all(&a.wasm).unwrap();
        w.start_file("__MACOSX/._payload.bin", opts).unwrap();
        w.write_all(&[0x00, 0x01]).unwrap();
        w.start_file(".DS_Store", opts).unwrap();
        w.write_all(&[0x00]).unwrap();
        let meta = CommentMeta {
            ext_id: a.ext_id.clone(),
            version: a.version.clone(),
            manifest_toml: a.manifest_toml.clone(),
            digest_hex: a.digest_hex.clone(),
            publisher_key_id: a.publisher_key_id.clone(),
            signature_hex: hex_encode(&a.signature),
        };
        w.set_raw_comment(serde_json::to_vec(&meta).unwrap().into_boxed_slice());
        let bytes = w.finish().unwrap().into_inner();
        let back = artifact_from_zip(&bytes, HUGE).expect("unpack");
        assert_eq!(back, a);
    }

    #[test]
    fn deterministic_packing_for_the_same_input() {
        let a = sample_artifact(b"stable bytes".to_vec());
        assert_eq!(
            artifact_to_zip(&a).unwrap(),
            artifact_to_zip(&a).unwrap(),
            "same input must produce byte-identical output"
        );
    }
}
