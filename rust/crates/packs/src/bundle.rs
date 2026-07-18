//! The bundle — a pack as it arrives over the wire, and the materialized [`Pack`] it resolves to.
//!
//! This replaces the prototype's filesystem loader (pack-core-scope §"Bundle-over-the-wire"): the
//! caller sends the manifest plus every referenced file in the call itself, so applying a pack needs
//! nothing but a session and caps — no node-filesystem coupling, which is what keeps "MCP is the
//! contract" literally true for a third party. A node-local path convenience rides the CLI, never
//! the verb.
//!
//! Everything here is pure: bytes in, a resolved `Pack` or a loud error out. The *parsing* rules
//! ported verbatim from the prototype's loader — rule id is the filename stem, rule display name is
//! the `// name:` first line, dashboard id is the `id` field inside the JSON record.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::manifest::Manifest;

/// The declared ceiling on a bundle's total file bytes (pack-core-scope §Risks: "cap the bundle,
/// honest error", with "big seed = generator script, not pack payload" the standing doctrine).
/// Counted over the manifest text plus every file body.
pub const MAX_BUNDLE_BYTES: usize = 8 * 1024 * 1024;

/// A pack as sent by a caller: the manifest text plus its referenced files, keyed by the
/// bundle-relative path the manifest names them by.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    /// The raw `pack.yaml` text. Kept raw (not re-serialized) because the pack checksum folds these
    /// exact bytes — a re-serialized manifest would change the hash and read as spurious drift.
    pub manifest: String,
    /// Every referenced file, `path -> contents`. Paths are exactly as the manifest writes them
    /// (`rules/fdd-sensor-flatline.rhai`, `schema.sql`, …).
    #[serde(default)]
    pub files: BTreeMap<String, String>,
}

/// A rule resolved out of the bundle.
#[derive(Debug, Clone)]
pub struct LoadedRule {
    /// The stable id — the filename stem (`rules/fdd-after-hours.rhai` → `fdd-after-hours`). The
    /// receipt records this, never the display name; drift detection depends on it.
    pub id: String,
    /// The display name — the first line stripped of its `// name:` prefix, or the id.
    pub name: String,
    pub body: String,
}

/// A dashboard resolved out of the bundle.
#[derive(Debug, Clone)]
pub struct LoadedDashboard {
    /// The id from the record's own `id` field, never the filename.
    pub id: String,
    pub json: serde_json::Value,
}

/// A fully-resolved pack: the parsed manifest plus every referenced file's contents. The plan, the
/// checksums, the linter, and the apply loop all read from here and nothing else.
#[derive(Debug, Clone)]
pub struct Pack {
    pub manifest: Manifest,
    /// The raw manifest text — folded into the pack checksum verbatim.
    pub manifest_raw: String,
    pub rules: Vec<LoadedRule>,
    pub dashboards: Vec<LoadedDashboard>,
    pub schema_sql: Option<String>,
    pub seed_sql: Option<String>,
    pub agent_context: Option<String>,
}

impl Bundle {
    /// Total declared size — the manifest text plus every file body.
    pub fn byte_len(&self) -> usize {
        self.manifest.len() + self.files.values().map(String::len).sum::<usize>()
    }

    /// Resolve the bundle into a [`Pack`]: parse the manifest, then pull every referenced file out
    /// of `files`.
    ///
    /// A missing referenced file is a hard error, not a silent skip: a pack that names
    /// `rules/x.rhai` and ships no such file is broken, and the author must learn that from
    /// `pack.validate` in CI, loudly.
    pub fn resolve(&self) -> Result<Pack, String> {
        if self.byte_len() > MAX_BUNDLE_BYTES {
            return Err(format!(
                "bundle is {} bytes, over the {MAX_BUNDLE_BYTES}-byte limit — a large seed belongs \
                 in a generator script, not the pack payload",
                self.byte_len()
            ));
        }

        let manifest = Manifest::parse(&self.manifest)
            .map_err(|e| format!("pack.yaml is not valid YAML: {e}"))?;

        let mut rules = Vec::with_capacity(manifest.rules.len());
        for path in &manifest.rules {
            let body = self.file(path)?;
            rules.push(LoadedRule {
                id: stem(path),
                name: rule_name(body).unwrap_or_else(|| stem(path)),
                body: body.to_string(),
            });
        }

        let mut dashboards = Vec::with_capacity(manifest.dashboards.len());
        for path in &manifest.dashboards {
            let text = self.file(path)?;
            let json: serde_json::Value = serde_json::from_str(text)
                .map_err(|e| format!("dashboard '{path}' is not valid JSON: {e}"))?;
            let id = json
                .get("id")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    format!(
                        "dashboard '{path}' has no string `id` field — the id is the stable \
                             object identity the receipt records, so it cannot be inferred"
                    )
                })?
                .to_string();
            dashboards.push(LoadedDashboard { id, json });
        }

        let (schema_sql, seed_sql) = match &manifest.datasource {
            Some(ds) => (
                ds.schema
                    .as_deref()
                    .map(|p| self.file(p))
                    .transpose()?
                    .map(str::to_string),
                ds.seed
                    .as_deref()
                    .map(|p| self.file(p))
                    .transpose()?
                    .map(str::to_string),
            ),
            None => (None, None),
        };

        let agent_context = manifest
            .agent
            .as_ref()
            .map(|a| self.file(&a.context))
            .transpose()?
            .map(str::to_string);

        Ok(Pack {
            manifest,
            manifest_raw: self.manifest.clone(),
            rules,
            dashboards,
            schema_sql,
            seed_sql,
            agent_context,
        })
    }

    fn file(&self, path: &str) -> Result<&str, String> {
        self.files.get(path).map(String::as_str).ok_or_else(|| {
            format!("the manifest references '{path}' but the bundle has no such file")
        })
    }
}

/// The filename stem of a bundle-relative path — the rule id convention.
fn stem(path: &str) -> String {
    let base = path.rsplit('/').next().unwrap_or(path);
    base.rsplit_once('.').map_or(base, |(s, _)| s).to_string()
}

/// The display name from a rule body's leading `// name:` line, if present and non-empty.
fn rule_name(body: &str) -> Option<String> {
    let first = body.lines().next()?.trim();
    let name = first
        .strip_prefix("//")?
        .trim()
        .strip_prefix("name:")?
        .trim();
    (!name.is_empty()).then(|| name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle(manifest: &str, files: &[(&str, &str)]) -> Bundle {
        Bundle {
            manifest: manifest.into(),
            files: files
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect(),
        }
    }

    #[test]
    fn resolves_rules_with_stem_ids_and_name_comments() {
        let b = bundle(
            "pack: p\ntitle: P\nversion: 1\nrules: [rules/flatline.rhai]\n",
            &[(
                "rules/flatline.rhai",
                "// name: Sensor Flatline\nlet x = 1;",
            )],
        );
        let pack = b.resolve().unwrap();
        assert_eq!(pack.rules[0].id, "flatline");
        assert_eq!(pack.rules[0].name, "Sensor Flatline");
    }

    #[test]
    fn a_rule_without_a_name_comment_falls_back_to_its_id() {
        let b = bundle(
            "pack: p\ntitle: P\nversion: 1\nrules: [rules/plain.rhai]\n",
            &[("rules/plain.rhai", "let x = 1;")],
        );
        let pack = b.resolve().unwrap();
        assert_eq!(pack.rules[0].name, "plain");
    }

    #[test]
    fn a_missing_referenced_file_is_a_loud_error() {
        let b = bundle(
            "pack: p\ntitle: P\nversion: 1\nrules: [rules/gone.rhai]\n",
            &[],
        );
        let err = b.resolve().unwrap_err();
        assert!(err.contains("rules/gone.rhai"), "{err}");
    }

    #[test]
    fn a_dashboard_without_an_id_is_a_loud_error() {
        let b = bundle(
            "pack: p\ntitle: P\nversion: 1\ndashboards: [d/x.json]\n",
            &[("d/x.json", r#"{"title":"no id here"}"#)],
        );
        let err = b.resolve().unwrap_err();
        assert!(err.contains("`id`"), "{err}");
    }

    #[test]
    fn an_oversize_bundle_is_refused_with_an_honest_error() {
        let big = "x".repeat(MAX_BUNDLE_BYTES + 1);
        let b = bundle("pack: p\ntitle: P\nversion: 1\n", &[("seed.sql", &big)]);
        let err = b.resolve().unwrap_err();
        assert!(err.contains("over the"), "{err}");
    }
}
