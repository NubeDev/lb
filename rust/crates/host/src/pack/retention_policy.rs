//! Convert a pack manifest's `retention:` block into the `lb_ingest` types `series.retention.set`
//! takes — a field-for-field move, and **the one place a shape drift between the mirror and the real
//! policy would surface** (`lb_packs::manifest_retention` explains why the mirror exists).
//!
//! `method` and `range.mode` arrive as `String`. An unknown name cannot reach here: `pack.validate`
//! errors the apply out first (`lb_packs::validate_retention`), so a typo is a lint the author sees
//! rather than a field silently dropped. The fallbacks below therefore never fire in practice; they
//! keep a hypothetical un-linted path at today's behaviour instead of panicking the node.

use lb_packs::RetentionPolicy;

/// Convert the pack manifest's [`RetentionPolicy`] into the ingest [`lb_ingest::Policy`] the setter
/// takes. The two share field names by design (the manifest struct is the verb's mirror), so this is
/// a field-for-field move — the one place a shape drift between them would surface.
pub(super) fn to_ingest_policy(p: &RetentionPolicy) -> lb_ingest::Policy {
    lb_ingest::Policy {
        prefix: p.prefix.clone(),
        raw_for_ms: p.raw_for_ms,
        max_samples: p.max_samples,
        tiers: p
            .tiers
            .iter()
            .map(|t| lb_ingest::Tier {
                width_ms: t.width_ms,
                keep_for_ms: t.keep_for_ms,
                // An unknown name cannot reach here: `packs::validate` errors the apply out first
                // (a lint an author sees, not a silent no-op). Falling back to `None` rather than
                // panicking keeps a hypothetical un-linted path at today's behaviour — the full
                // stat row — instead of taking the node down.
                method: t
                    .method
                    .as_deref()
                    .and_then(|m| lb_ingest::Method::parse(m).ok()),
                // Where the tier's buckets start. Absent stays absent — the UTC epoch grid — so a
                // pack.yaml written before this field existed applies to exactly the policy it
                // always did. Unlike `method` there is no lint on the way through: the mirror holds
                // it as the same integer the verb takes, so there is no name to get wrong.
                align: t.align.map(|a| lb_ingest::Align {
                    origin_ms: a.origin_ms,
                }),
            })
            .collect(),
        filter: p.filter.as_ref().map(to_ingest_filter),
        // Provenance is stamped by `series_retention_set` from the APPLYING principal. A converter
        // has no author to name, and the manifest cannot supply one (`lb_packs::RetentionPolicy` is
        // `deny_unknown_fields`, so `updated_by:` in a pack.yaml is a line-numbered error) — which is
        // the correct outcome for a field that must never be caller-supplied.
        updated_by: None,
        updated_ms: None,
    }
}

/// Convert the manifest's mirror of the `filter` block. Same field-for-field posture as
/// [`to_ingest_policy`] — the one place a drift from `lb_ingest::Filter` surfaces.
fn to_ingest_filter(f: &lb_packs::RetentionFilter) -> lb_ingest::Filter {
    lb_ingest::Filter {
        drop: f.drop,
        min_interval_ms: f.min_interval_ms,
        deadband: f.deadband.map(|d| lb_ingest::Deadband {
            abs: d.abs,
            pct: d.pct,
        }),
        range: f.range.as_ref().map(|r| lb_ingest::Range {
            min: r.min,
            max: r.max,
            // `validate` rejects an unknown mode; the default here matches the verb's own default.
            mode: match r.mode.as_deref() {
                Some("clamp") => lb_ingest::RangeMode::Clamp,
                _ => lb_ingest::RangeMode::Drop,
            },
        }),
    }
}
