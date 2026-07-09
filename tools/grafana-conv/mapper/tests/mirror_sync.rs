//! Mirror-sync guard (grafana-conversion scope, "Mirror-sync guard"). The vendored
//! `mapper/src/model.rs` must stay byte-synced with the host's
//! `rust/crates/host/src/dashboard/model.rs` (modulo the mirror header note
//! prepended to the vendored copy). A drift is a fold-in hazard: the emitted
//! shape silently diverging from what the host stores. This test fails loudly
//! before that happens.
//!
//! Resolution at test time (no path-dep into the host crate): we compare the
//! canonical content of the host file against the vendored file with its header
//! stripped. They must match.

use std::path::PathBuf;

const HOST_MODEL: &str = "../../../rust/crates/host/src/dashboard/model.rs";
const VENDORED_MODEL: &str = "src/model.rs";

/// Lines that exist ONLY in the vendored copy (the mirror header note). Stripped
/// before comparison. The header ends at the line of em-dashes (`//! ───…`) plus
/// the blank `//!` separator line that follows; everything after that is the
/// byte-for-byte mirror of the host file.
fn strip_mirror_header(vendored: &str) -> String {
    let marker = "─────────────────────────────────────────────────────────────────────────────";
    let rest = vendored
        .split_once(marker)
        .map(|(_, rest)| rest)
        .unwrap_or(vendored);
    // Drop the trailing `\n` of the marker line, the blank `//!` separator, and
    // the leading `\n` of the first body line.
    rest.trim_start_matches('\n')
        .trim_start_matches("//!\n")
        .to_string()
}
#[test]
fn vendored_model_matches_host() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let host_path = root.join(HOST_MODEL);
    let vendored_path = root.join(VENDORED_MODEL);

    let host = std::fs::read_to_string(&host_path).unwrap_or_else(|e| {
        panic!(
            "read host model {}: {} (is the repo layout intact?)",
            host_path.display(),
            e
        )
    });
    let vendored = std::fs::read_to_string(&vendored_path).expect("read vendored model");

    let stripped = strip_mirror_header(&vendored);
    let host_trimmed = host.trim_end();
    let stripped_trimmed = stripped.trim_end();

    if host_trimmed != stripped_trimmed {
        // Find the first diverging line for a useful message.
        let host_lines: Vec<&str> = host_trimmed.lines().collect();
        let vend_lines: Vec<&str> = stripped_trimmed.lines().collect();
        let mut first_diff = None;
        for i in 0..host_lines.len().max(vend_lines.len()) {
            if host_lines.get(i) != vend_lines.get(i) {
                first_diff = Some(i);
                break;
            }
        }
        let i = first_diff.unwrap_or(0);
        panic!(
            "VENDORED MIRROR DRIFT at line {}.\n\
             host:       {:?}\n\
             vendored:   {:?}\n\
             Re-copy rust/crates/host/src/dashboard/model.rs into mapper/src/model.rs\n\
             and re-prepend the mirror header note (see the vendored file's top doc).",
            i + 1,
            host_lines.get(i).copied().unwrap_or("<EOF>"),
            vend_lines.get(i).copied().unwrap_or("<EOF>"),
        );
    }
}
