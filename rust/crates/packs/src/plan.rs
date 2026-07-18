//! The ordered object plan + the checksums — pure, and the spine three things must agree on: the
//! `pack.validate` dry-run (what WOULD be created), the apply loop (what IS created, in order), and
//! the receipt (`objects: [{kind, id, checksum, outcome}]`). Deriving all three from ONE function is
//! what keeps "the dry run matches the apply matches the receipt" true by construction.
//!
//! Order matters: datasource first (rules query it), then rules, dashboards, channels, agent — so a
//! partial apply fails forward in a sensible sequence and the receipt records progress in apply
//! order.
//!
//! Ported verbatim from the proving prototype.

use sha2::{Digest, Sha256};

use crate::bundle::Pack;

/// The kind of a pack-owned object — the receipt's `kind` and a reader's deep-link discriminator.
/// Stable strings; never a named-pack branch (rule 10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Datasource,
    Rule,
    Dashboard,
    Channel,
    Agent,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Datasource => "datasource",
            Kind::Rule => "rule",
            Kind::Dashboard => "dashboard",
            Kind::Channel => "channel",
            Kind::Agent => "agent",
        }
    }
}

/// One planned object: its kind, stable id, and content checksum — the receipt's per-object hash and
/// the drift/clobber signal. The checksum is over the exact bytes the applier will send.
#[derive(Debug, Clone)]
pub struct PlannedObject {
    pub kind: Kind,
    pub id: String,
    pub checksum: String,
}

/// Derive the full ordered object plan for a pack. Pure — no I/O.
pub fn plan(pack: &Pack) -> Vec<PlannedObject> {
    let mut out = Vec::new();

    if let Some(ds) = &pack.manifest.datasource {
        // The datasource checksum folds the registration args AND the schema/seed SQL, so a changed
        // DDL is drift even though the datasource name is unchanged.
        let mut h = String::new();
        h.push_str(&ds.name);
        h.push_str(&ds.engine);
        if let Some(s) = &pack.schema_sql {
            h.push_str(s);
        }
        if let Some(s) = &pack.seed_sql {
            h.push_str(s);
        }
        out.push(PlannedObject {
            kind: Kind::Datasource,
            id: ds.name.clone(),
            checksum: checksum(&h),
        });
    }

    for r in &pack.rules {
        out.push(PlannedObject {
            kind: Kind::Rule,
            id: r.id.clone(),
            checksum: checksum(&r.body),
        });
    }

    for d in &pack.dashboards {
        out.push(PlannedObject {
            kind: Kind::Dashboard,
            id: d.id.clone(),
            checksum: checksum(&d.json.to_string()),
        });
    }

    for c in &pack.manifest.channels {
        out.push(PlannedObject {
            kind: Kind::Channel,
            id: c.name.clone(),
            checksum: checksum(&c.name),
        });
    }

    if let Some(ctx) = &pack.agent_context {
        // The agent context has no natural id; the pack name keys it (one context per workspace).
        out.push(PlannedObject {
            kind: Kind::Agent,
            id: pack.manifest.pack.clone(),
            checksum: checksum(ctx),
        });
    }

    out
}

/// Hex SHA-256 of `s` — the manifest checksum and every per-object checksum share this.
pub fn checksum(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    hex(&hasher.finalize())
}

/// The pack's overall content checksum — the receipt's `manifest_checksum` and the drift signal.
/// Folds the manifest YAML AND every referenced file's bytes (rule bodies, dashboard JSON,
/// schema/seed SQL, agent context), so "files changed but the version was not bumped" is detected
/// even when the change is in a sibling file rather than `pack.yaml` — the refusal matrix's whole
/// point.
pub fn content_checksum(pack: &Pack) -> String {
    let mut h = Sha256::new();
    h.update(pack.manifest_raw.as_bytes());
    for r in &pack.rules {
        h.update(r.id.as_bytes());
        h.update(r.body.as_bytes());
    }
    for d in &pack.dashboards {
        h.update(d.id.as_bytes());
        h.update(d.json.to_string().as_bytes());
    }
    for sql in [&pack.schema_sql, &pack.seed_sql].into_iter().flatten() {
        h.update(sql.as_bytes());
    }
    if let Some(ctx) = &pack.agent_context {
        h.update(ctx.as_bytes());
    }
    hex(&h.finalize())
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}
