//! Embed the developer-authored core-skill corpus (`docs/skills/*/SKILL.md`) into the binary at
//! build time (core-skills scope: "the node embeds the corpus at build time"). Each `SKILL.md` is a
//! runnable operating manual for one platform surface; this turns the repo docs into an in-binary
//! `&[(name, description, body)]` slice the boot seeder writes as `skill:core.<name>@<version>`.
//!
//! Two rewrites the scope calls for happen HERE, at embed time, so the loaded body is fit for an
//! agent on a customer node (not a repo reader):
//!   - **strip the YAML frontmatter** — `name`/`description` are lifted into their own columns; the
//!     body is the prose after the closing `---` (frontmatter is metadata, not instruction text);
//!   - **flag repo-relative links** — a `](../…)` / `](docs/…)` link dangles once the doc is loaded
//!     away from the repo, so each is rewritten to a plain-text marker `[text](repo-relative: …)` so
//!     the agent sees the reference without a broken link (corpus-drift risk in the scope).
//!
//! The generated file is written to `$OUT_DIR/core_skills_corpus.rs` and `include!`d by
//! `skill/corpus.rs`. Re-run only when a `SKILL.md` changes (the `rerun-if-changed` lines below).

use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    // The corpus lives at the repo root under docs/skills/; this crate is rust/crates/assets, so the
    // repo root is three levels up from CARGO_MANIFEST_DIR.
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let skills_dir = manifest_dir.join("../../../docs/skills");
    // The platform's own e2e RUNBOOKS also seed as core skills (agent-personas #2 persona-grounding):
    // a persona-grounded run learns "how do I verify this?" from the runbook, not from crawling the
    // codebase. They live under docs/testing/ (top-level + per-area READMEs) and already carry
    // skill-shaped `e2e-*` frontmatter — pulled directly from where they live (no copy that can drift).
    let testing_dir = manifest_dir.join("../../../docs/testing");

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let dest = out_dir.join("core_skills_corpus.rs");

    // Rebuild if either corpus directory listing changes (a skill/runbook added/removed).
    println!("cargo:rerun-if-changed={}", skills_dir.display());
    println!("cargo:rerun-if-changed={}", testing_dir.display());

    let mut entries: Vec<(String, String, String)> = Vec::new();

    // (1) The developer-authored skills corpus: EVERY `docs/skills/<name>/SKILL.md`. No allow-list —
    // a new dir auto-embeds. ANTI-ROT ASSERTION (persona-grounding scope): a skills subdir that is
    // missing its `SKILL.md` FAILS the build. Before, a missing SKILL.md was silently skipped, so a
    // half-authored skill could ship as "absent" and a persona pinning it would fail-closed at run
    // time with no build-time signal. Now the corpus cannot silently rot: either the dir has a valid
    // SKILL.md or the build stops.
    if let Ok(read) = fs::read_dir(&skills_dir) {
        let mut dirs: Vec<PathBuf> = read
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_dir())
            .collect();
        dirs.sort(); // deterministic corpus order (testing §3: no incidental ordering).
        for dir in dirs {
            let skill_md = dir.join("SKILL.md");
            if !skill_md.exists() {
                panic!(
                    "core-skills corpus: {} has no SKILL.md — every docs/skills/<name>/ dir must \
                     carry one (anti-rot gate, persona-grounding scope). Add the SKILL.md or remove \
                     the directory.",
                    dir.display()
                );
            }
            // Rebuild if any embedded SKILL.md changes.
            println!("cargo:rerun-if-changed={}", skill_md.display());
            let raw = fs::read_to_string(&skill_md)
                .unwrap_or_else(|e| panic!("read {}: {e}", skill_md.display()));
            match parse_skill(&raw) {
                Some((name, description, body)) => {
                    entries.push((name, description, flag_repo_links(&body)))
                }
                None => panic!(
                    "core-skills corpus: {} has no `name`/`description` frontmatter",
                    skill_md.display()
                ),
            }
        }
    }

    // (2) The e2e runbooks under docs/testing/**: any `.md` with skill-shaped frontmatter (the
    // top-level `e2e-backend.md`/`e2e-frontend.md` + the per-area `<area>/README.md`). A `.md` WITHOUT
    // frontmatter (the `docs/testing/README.md` index) is skipped — it is a human index, not a skill.
    // The frontmatter `name:` (already `e2e-*`) becomes `core.<name>`, exactly like a skills dir.
    let mut testing_files = collect_markdown(&testing_dir);
    testing_files.sort();
    for md in testing_files {
        println!("cargo:rerun-if-changed={}", md.display());
        let raw = fs::read_to_string(&md).unwrap_or_else(|e| panic!("read {}: {e}", md.display()));
        if let Some((name, description, body)) = parse_skill(&raw) {
            entries.push((name, description, flag_repo_links(&body)));
        }
        // No frontmatter → not a skill (e.g. the README index); silently skip (unlike the skills dir,
        // a bare `.md` under testing/ is legitimately allowed to be non-skill prose).
    }

    // Guard against an accidental id collision between the two roots (a skills dir named `e2e-foo`
    // would clash with a runbook of the same frontmatter name). Deterministic, build-time.
    let mut names: Vec<&str> = entries.iter().map(|(n, _, _)| n.as_str()).collect();
    names.sort_unstable();
    for pair in names.windows(2) {
        if pair[0] == pair[1] {
            panic!(
                "core-skills corpus: duplicate skill name {:?} across docs/skills + docs/testing — \
                 names must be unique (they become core.<name>)",
                pair[0]
            );
        }
    }

    let mut src = String::new();
    src.push_str("// @generated by build.rs — the embedded core-skill corpus. Do not edit.\n");
    src.push_str("pub static CORE_SKILLS: &[(&str, &str, &str)] = &[\n");
    for (name, description, body) in &entries {
        src.push_str("    (");
        push_str_lit(&mut src, name);
        src.push_str(", ");
        push_str_lit(&mut src, description);
        src.push_str(", ");
        push_str_lit(&mut src, body);
        src.push_str("),\n");
    }
    src.push_str("];\n");

    fs::write(&dest, src).unwrap_or_else(|e| panic!("write {}: {e}", dest.display()));
}

/// Recursively collect every `.md` file under `root` (one level of subdirectory is enough for the
/// runbook layout — `docs/testing/*.md` + `docs/testing/<area>/*.md` — but the walk is fully
/// recursive so a deeper runbook still seeds). Returns an empty vec if `root` is absent.
fn collect_markdown(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(read) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in read.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
                out.push(path);
            }
        }
    }
    out
}

/// Split a `SKILL.md` into `(name, description, body)`: parse the leading `--- … ---` YAML
/// frontmatter for `name`/`description`, and return the prose after it as the body (frontmatter
/// stripped). Returns `None` if there is no frontmatter or either key is missing.
fn parse_skill(raw: &str) -> Option<(String, String, String)> {
    let rest = raw.strip_prefix("---")?;
    // The frontmatter ends at the next line that is exactly `---`.
    let end = rest.find("\n---")?;
    let front = &rest[..end];
    // Body is everything after the closing `---` line.
    let after = &rest[end + 4..];
    let body = after.trim_start_matches(['\r', '\n']).to_string();

    let name = yaml_scalar(front, "name")?;
    let description = yaml_scalar(front, "description")?;
    Some((name, description, body))
}

/// Extract a YAML scalar for `key` from a frontmatter block. Handles a plain `key: value` and the
/// folded/blocked forms (`key: >-` / `key: |`) where the value continues on indented lines — enough
/// for the SKILL.md corpus (a `description:` is often a folded multi-line scalar).
fn yaml_scalar(front: &str, key: &str) -> Option<String> {
    let lines: Vec<&str> = front.lines().collect();
    let prefix = format!("{key}:");
    let idx = lines
        .iter()
        .position(|l| l.trim_start().starts_with(&prefix))?;
    let first = lines[idx].trim_start()[prefix.len()..].trim();
    if !first.is_empty() && first != ">-" && first != ">" && first != "|" && first != "|-" {
        // Plain inline scalar — strip surrounding quotes if any.
        return Some(first.trim_matches(['"', '\'']).to_string());
    }
    // Folded/blocked scalar: gather the following more-indented lines, joining with spaces (folded).
    let mut parts: Vec<String> = Vec::new();
    for l in &lines[idx + 1..] {
        if l.trim().is_empty() {
            continue;
        }
        // A line at column 0 (or a `key:` at base indent) ends the scalar.
        if !l.starts_with(char::is_whitespace) {
            break;
        }
        parts.push(l.trim().to_string());
    }
    if parts.is_empty() {
        return None;
    }
    Some(parts.join(" "))
}

/// Rewrite repo-relative markdown links (`](../…)`, `](docs/…)`, `](/docs/…)`) into a plain-text
/// marker so a loaded skill body carries the reference without a dangling link (corpus-drift risk).
/// An absolute `http(s)://` link is left untouched.
fn flag_repo_links(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let bytes = body.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Look for the `](` that opens a markdown link target.
        if bytes[i] == b']' && i + 1 < bytes.len() && bytes[i + 1] == b'(' {
            // Find the matching close paren.
            if let Some(close_rel) = body[i + 2..].find(')') {
                let target = &body[i + 2..i + 2 + close_rel];
                if is_repo_relative(target) {
                    out.push_str("](repo-relative: ");
                    out.push_str(target);
                    out.push(')');
                    i = i + 2 + close_rel + 1;
                    continue;
                }
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// A link target that points inside the repo (would dangle once loaded away from it).
fn is_repo_relative(target: &str) -> bool {
    let t = target.trim();
    if t.starts_with("http://") || t.starts_with("https://") || t.starts_with('#') {
        return false;
    }
    t.starts_with("../")
        || t.starts_with("./")
        || t.starts_with("docs/")
        || t.starts_with("/docs")
        || t.contains("/docs/")
        || t.ends_with(".md")
        || t.ends_with(".rs")
}

/// Emit `s` as a Rust raw or escaped string literal into `src`. Uses a `r#"…"#` raw literal when the
/// content has no `"#` sequence (the common case — skill bodies are prose); otherwise escapes.
fn push_str_lit(src: &mut String, s: &str) {
    if !s.contains("\"#") {
        src.push_str("r#\"");
        src.push_str(s);
        src.push_str("\"#");
    } else {
        src.push('"');
        for c in s.chars() {
            match c {
                '"' => src.push_str("\\\""),
                '\\' => src.push_str("\\\\"),
                _ => src.push(c),
            }
        }
        src.push('"');
    }
}
