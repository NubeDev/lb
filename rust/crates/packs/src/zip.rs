//! The zip transport envelope — one `.zip` archive unpacked into a [`Bundle`].
//!
//! A pack is distributed as ONE file: a customer is handed `ems.zip`, not a directory. The archive
//! is a transport envelope and nothing more — it is inflated to the same `{manifest, files}` the
//! JSON verb path takes, and discarded. No new pack semantics live here: `pack.validate` /
//! `pack.apply` see a bundle they cannot tell from a hand-assembled one.
//!
//! Everything here is pure (bytes in, a `Bundle` or a loud error out), so it is exercised without a
//! node — and it lives beside [`MAX_BUNDLE_BYTES`] deliberately, because the cap is enforced
//! **while inflating**, entry by entry. A zip that declares 4 KB and expands to a gigabyte must die
//! against the budget, not after it.
//!
//! The rules below are the SAME rules the browser-side reader enforces (rubix-ai's
//! `ui/src/lib/packs/readZip.ts`). Two implementations of one contract is a drift risk taken
//! knowingly: the browser must reject a bad archive before it wastes an upload, and the node must
//! never trust a client to have done so.
//!
//! Rule 10: nothing here knows a pack by name. An archive is data.

use std::collections::BTreeMap;
use std::io::Read;

use crate::bundle::{Bundle, MAX_BUNDLE_BYTES};

/// The manifest filename a bundle hoists out of `files` — the one member every pack archive must
/// carry at its root.
pub const MANIFEST_FILENAME: &str = "pack.yaml";

/// Inflate a pack archive into a [`Bundle`].
///
/// Rejections, each naming the offending member so an author can fix the archive rather than guess:
/// a member that escapes the pack root (`..` or an absolute path — zip-slip), a member that is not
/// UTF-8 text (packs are declarative text; a binary member means the wrong thing was zipped), a
/// total inflated size over [`MAX_BUNDLE_BYTES`] (checked as it inflates — the zip-bomb guard), and
/// an archive with no root `pack.yaml`.
///
/// A single top-level directory is stripped (`zip -r ems.zip ems/`, and what GitHub's "Download ZIP"
/// produces) — but only when the archive is not already pack-rooted and every member shares that one
/// segment, so an ambiguous archive fails on the manifest rule instead of being guessed at.
pub fn bundle_from_zip(bytes: &[u8]) -> Result<Bundle, String> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|e| format!("not a readable zip archive: {e}"))?;

    let mut entries: BTreeMap<String, String> = BTreeMap::new();
    let mut budget = MAX_BUNDLE_BYTES;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("unreadable archive member #{i}: {e}"))?;
        if entry.is_dir() {
            continue;
        }
        // `enclosed_name` is the zip crate's own zip-slip guard: it returns `None` for an absolute
        // path, a `..` component, or a name that is not valid UTF-8. We re-state the reason rather
        // than pass its `None` along, because "which member, and why" is the whole value of the
        // error to whoever built the archive.
        let name = entry
            .enclosed_name()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .ok_or_else(|| {
                format!(
                    "archive member {:?} is not a safe relative path — a pack bundle holds only \
                     relative paths inside its own root",
                    entry.name()
                )
            })?;
        if is_noise(&name) {
            continue;
        }

        // Inflate against a RUNNING budget: `take` refuses to read past what is left, so a zip bomb
        // is stopped mid-inflation instead of after it has already cost the memory.
        let mut buf = Vec::new();
        let read = entry
            .by_ref()
            .take(budget as u64 + 1)
            .read_to_end(&mut buf)
            .map_err(|e| format!("could not read archive member {name}: {e}"))?;
        if read > budget {
            return Err(over_cap_message());
        }
        budget -= read;

        let text = String::from_utf8(buf).map_err(|_| {
            format!(
                "archive member {name} is not UTF-8 text — a pack bundle carries only text files \
                 (remove binaries from the archive)"
            )
        })?;
        entries.insert(name, text);
    }

    let mut entries = strip_single_root(entries);
    let manifest = entries.remove(MANIFEST_FILENAME).ok_or_else(|| {
        format!(
            "no {MANIFEST_FILENAME} at the root of the archive — zip the pack's own folder, or its \
             contents"
        )
    })?;
    Ok(Bundle {
        manifest,
        files: entries,
    })
}

/// Archive members that are packaging noise, never pack content — dropped before any other rule so
/// a macOS-zipped pack (which always carries `__MACOSX/`) is not rejected for a binary member.
fn is_noise(name: &str) -> bool {
    name.starts_with("__MACOSX/")
        || name
            .split('/')
            .any(|seg| seg == ".DS_Store" || seg == "Thumbs.db")
}

/// Drop the single top-level directory an archive may wrap the pack in. See [`bundle_from_zip`] for
/// when this applies (and, deliberately, when it does not).
fn strip_single_root(entries: BTreeMap<String, String>) -> BTreeMap<String, String> {
    if entries.contains_key(MANIFEST_FILENAME) || entries.is_empty() {
        return entries;
    }
    let roots: std::collections::BTreeSet<&str> = entries
        .keys()
        .map(|k| k.split('/').next().unwrap_or(""))
        .collect();
    let [root] = roots.into_iter().collect::<Vec<_>>()[..] else {
        return entries;
    };
    let prefix = format!("{root}/");
    if !entries.keys().all(|k| k.starts_with(&prefix)) {
        return entries;
    }
    entries
        .into_iter()
        .map(|(k, v)| (k[prefix.len()..].to_string(), v))
        .collect()
}

/// The over-cap message names the cap AND the way out — the standing doctrine is "big seed =
/// generator script, not pack payload", and a bare "too large" leaves the author nothing to act on.
fn over_cap_message() -> String {
    format!(
        "archive inflates past the {MAX_BUNDLE_BYTES}-byte bundle limit — a large seed belongs in a \
         generator script, not the pack payload"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    /// Build a zip in memory from `(name, bytes)` members — the archive a caller would upload.
    fn zip_of(members: &[(&str, &[u8])]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let opts = SimpleFileOptions::default();
        for (name, body) in members {
            w.start_file(*name, opts).expect("start");
            w.write_all(body).expect("write");
        }
        w.finish().expect("finish").into_inner()
    }

    fn text_zip(members: &[(&str, &str)]) -> Vec<u8> {
        let owned: Vec<(&str, &[u8])> = members.iter().map(|(n, b)| (*n, b.as_bytes())).collect();
        zip_of(&owned)
    }

    #[test]
    fn hoists_the_manifest_and_keeps_every_other_file() {
        let b = bundle_from_zip(&text_zip(&[
            ("pack.yaml", "pack: demo"),
            ("rules/a.rhai", "rule a"),
            ("schema.sql", "create table t(x int);"),
        ]))
        .expect("bundle");
        assert_eq!(b.manifest, "pack: demo");
        assert_eq!(b.files.len(), 2);
        assert_eq!(b.files["rules/a.rhai"], "rule a");
        assert!(!b.files.contains_key("pack.yaml"));
    }

    /// The `zip -r ems.zip ems/` and GitHub "Download ZIP" shape.
    #[test]
    fn strips_a_single_top_level_directory() {
        let b = bundle_from_zip(&text_zip(&[
            ("demo/pack.yaml", "pack: demo"),
            ("demo/rules/a.rhai", "rule a"),
        ]))
        .expect("bundle");
        assert_eq!(b.manifest, "pack: demo");
        assert_eq!(b.files.keys().collect::<Vec<_>>(), vec!["rules/a.rhai"]);
    }

    /// Two top-level folders is ambiguous — left unstripped so it fails on the honest manifest rule.
    #[test]
    fn refuses_to_guess_between_two_top_level_folders() {
        let err = bundle_from_zip(&text_zip(&[
            ("a/pack.yaml", "pack: a"),
            ("b/pack.yaml", "pack: b"),
        ]))
        .expect_err("ambiguous");
        assert!(err.contains("no pack.yaml at the root"), "{err}");
    }

    #[test]
    fn rejects_zip_slip() {
        let err = bundle_from_zip(&text_zip(&[
            ("pack.yaml", "pack: demo"),
            ("../escape.rhai", "pwned"),
        ]))
        .expect_err("zip-slip");
        assert!(err.contains("safe relative path"), "{err}");
    }

    #[test]
    fn rejects_an_absolute_member() {
        let err = bundle_from_zip(&text_zip(&[("/etc/passwd", "root:x")])).expect_err("absolute");
        assert!(err.contains("safe relative path"), "{err}");
    }

    #[test]
    fn rejects_a_binary_member_by_name() {
        let err = bundle_from_zip(&zip_of(&[
            ("pack.yaml", b"pack: demo".as_slice()),
            ("logo.png", &[0xff, 0xfe, 0x00]),
        ]))
        .expect_err("binary");
        assert!(err.contains("logo.png"), "{err}");
        assert!(err.contains("not UTF-8 text"), "{err}");
    }

    #[test]
    fn rejects_an_archive_with_no_manifest() {
        let err =
            bundle_from_zip(&text_zip(&[("rules/a.rhai", "rule a")])).expect_err("no manifest");
        assert!(err.contains("no pack.yaml at the root"), "{err}");
    }

    /// The zip-bomb guard: a tiny archive of highly compressible bytes must die against the budget
    /// as it inflates, naming the limit.
    #[test]
    fn rejects_an_archive_that_inflates_past_the_cap() {
        let big = "a".repeat(MAX_BUNDLE_BYTES + 1);
        let bytes = text_zip(&[("pack.yaml", "pack: demo"), ("seed.sql", &big)]);
        assert!(
            bytes.len() < MAX_BUNDLE_BYTES,
            "the archive itself must be small — that is the point of the test"
        );
        let err = bundle_from_zip(&bytes).expect_err("bomb");
        assert!(err.contains("bundle limit"), "{err}");
    }

    /// The budget is TOTAL, not per-member: many members that each fit still cannot sum past it.
    #[test]
    fn the_cap_is_the_total_across_members() {
        let chunk = "b".repeat(MAX_BUNDLE_BYTES / 2);
        let err = bundle_from_zip(&text_zip(&[
            ("pack.yaml", "pack: demo"),
            ("one.sql", &chunk),
            ("two.sql", &chunk),
            ("three.sql", &chunk),
        ]))
        .expect_err("over cap");
        assert!(err.contains("bundle limit"), "{err}");
    }

    #[test]
    fn ignores_macos_packaging_noise() {
        let b = bundle_from_zip(&zip_of(&[
            ("pack.yaml", b"pack: demo".as_slice()),
            ("__MACOSX/._pack.yaml", &[0x00, 0x01]),
            (".DS_Store", &[0x00, 0x01]),
        ]))
        .expect("bundle");
        assert!(b.files.is_empty());
    }

    #[test]
    fn rejects_bytes_that_are_not_a_zip() {
        let err = bundle_from_zip(b"not a zip at all").expect_err("not a zip");
        assert!(err.contains("not a readable zip archive"), "{err}");
    }

    /// The whole point of the envelope: an uploaded archive resolves to the same `Pack` the JSON
    /// bundle path produces, so nothing downstream can tell how it arrived.
    #[test]
    fn the_inflated_bundle_resolves_like_any_other() {
        let manifest = "pack: demo\ntitle: Demo\nversion: 1\nrules:\n  - rules/a.rhai\n";
        let from_zip = bundle_from_zip(&text_zip(&[
            ("pack.yaml", manifest),
            ("rules/a.rhai", "// name: A\nlet x = 1;"),
        ]))
        .expect("bundle");
        let direct = Bundle {
            manifest: manifest.to_string(),
            files: [(
                "rules/a.rhai".to_string(),
                "// name: A\nlet x = 1;".to_string(),
            )]
            .into_iter()
            .collect(),
        };
        assert_eq!(from_zip.manifest, direct.manifest);
        assert_eq!(from_zip.files, direct.files);
        let a = from_zip.resolve().expect("resolve");
        let b = direct.resolve().expect("resolve");
        assert_eq!(a.manifest_raw, b.manifest_raw);
        assert_eq!(a.rules.len(), b.rules.len());
        assert_eq!(a.rules[0].id, "a");
    }
}
