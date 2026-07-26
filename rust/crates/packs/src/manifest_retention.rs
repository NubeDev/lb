//! The `retention:` block of a pack manifest — a dependency-free MIRROR of the
//! `series.retention.set` verb args (`lb_ingest::Policy`), so a policy that validates in a pack is
//! byte-for-byte the one the verb takes.
//!
//! **Why a mirror and not the real types.** `lb-packs` is the pure, dependency-light half of the
//! pack engine; taking a dep on `lb-ingest` (and through it the store) to reuse four small structs
//! would invert the layering. The cost is that `method` and `range.mode` are held as `String` rather
//! than the real enums — so [`crate::validate`] carries the lint that rejects an unknown name at
//! validate time, where the author is still looking, instead of letting the apply-side conversion
//! drop it silently (the closed-struct trap).
//!
//! `deny_unknown_fields` throughout: a typo'd key is a loud parse error, never a swallowed line.

use serde::{Deserialize, Serialize};

/// One series retention policy to seed (`pack-retention-scope.md`). The field shape MIRRORS the
/// `series.retention.set` verb args byte-for-byte (`lb_ingest::Policy`) so a policy that validates in
/// the verb deserializes here and back — the apply arm converts this straight into that `Policy`.
/// Keyed by `prefix` (its natural id): the retention policy for a series-name prefix.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RetentionPolicy {
    /// The series-name PREFIX this policy governs (e.g. `modbus.`). The longest matching prefix wins
    /// (series-retention scope). Also the receipt object id (`retention:<prefix>`).
    pub prefix: String,
    /// Keep raw samples this many ms before rolling them up + evicting. `0` disables the time horizon.
    #[serde(default)]
    pub raw_for_ms: u64,
    /// FIFO count cap on raw samples per series (`0` = unbounded). The oldest over the cap are evicted.
    #[serde(default)]
    pub max_samples: u64,
    /// Downsample tiers: what falls off `raw_for_ms` rolls into these, each kept for its own horizon.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tiers: Vec<RetentionTier>,
    /// Write-time normalize predicates — what is ever STORED, as distinct from how long it lives
    /// (series-normalize scope). Absent = store everything.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<RetentionFilter>,
}

/// One downsample tier of a [`RetentionPolicy`] — mirrors `lb_ingest::Tier`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RetentionTier {
    /// Bucket width (ms) this tier rolls raw into.
    pub width_ms: u64,
    /// How long (ms) this tier's rollup rows are kept before eviction.
    pub keep_for_ms: u64,
    /// The single value this tier reads as: `avg|min|max|sum|count|last|first|nearest`. Absent =
    /// the full stat row (today's behaviour). Held as a `String` because this manifest is a
    /// dependency-free MIRROR of the verb args; `validate` rejects an unknown name before apply, so
    /// a typo is a loud lint rather than a silent no-op.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
}

/// The `filter` block of a [`RetentionPolicy`] — mirrors `lb_ingest::Filter`. Every field defaults
/// to inert, so an absent block and an empty one both mean "store everything".
#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RetentionFilter {
    /// Accept-but-store-nothing mute.
    #[serde(default)]
    pub drop: bool,
    /// Keep at most one stored sample per N ms per `(series, producer)` — the FIRST of each interval.
    #[serde(default)]
    pub min_interval_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadband: Option<RetentionDeadband>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<RetentionRange>,
}

/// The change threshold below which a sample is redundant — mirrors `lb_ingest::Deadband`.
#[derive(Debug, Clone, Copy, PartialEq, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RetentionDeadband {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abs: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pct: Option<f64>,
}

/// A value band — mirrors `lb_ingest::Range`. `mode` is `drop` (default) or `clamp`.
#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RetentionRange {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    /// `"drop"` (default) or `"clamp"`. A `String` for the same mirror reason as `Tier::method`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}
