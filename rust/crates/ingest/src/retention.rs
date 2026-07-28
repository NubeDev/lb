//! The per-series-prefix retention policy — the record the GC pass executes (series-retention
//! scope). A policy says: keep raw samples for `raw_for_ms`, downsample what falls off into the
//! listed rollup `tiers` (each kept for its own horizon), then evict. Workspace-scoped like every
//! series-plane record (the hard wall); administered only through the capability-gated
//! `series.retention.*` verbs in the host.

use lb_store::{Store, StoreError};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::filter::Filter;
use crate::method::Method;

/// The retention-policy table; one row per series-name prefix (id = prefix).
pub const RETENTION_TABLE: &str = "series_retention";

/// One rollup tier: bucket width, how long the tier's rows are kept (`0` = keep forever), and
/// optionally the single `method` value the tier reads as (series-normalize scope).
///
/// `method` is `None` by default, which is exactly today's behaviour: a bucketed read returns the
/// full stat row and no `value` column. Setting it adds the column — it never removes one.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Tier {
    pub width_ms: u64,
    pub keep_for_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<Method>,
}

/// A retention policy for every series whose name starts with `prefix`.
///
/// Two INDEPENDENT bounds on the same series, either of which evicts a sample:
///   - `raw_for_ms` — the TIME horizon ("how old is too old"). `0` disables it.
///   - `max_samples` — the COUNT cap ("how much is too much"), FIFO: the oldest samples over the
///     bound are evicted first. `0` disables it.
///
/// Both default to `0` (unbounded), so a policy row written before either axis existed keeps its
/// exact meaning. Time does not bound bytes — **rate** does, and rate is the producer's choice, not
/// the operator's; that is why the count axis exists (issue #65).
/// A third, INDEPENDENT axis arrived with series-normalize: `filter` bounds what is ever *stored*,
/// where the two above bound how long what was stored *lives*. Absent (the default) = store
/// everything, so every policy row written before this slice keeps its exact meaning.
/// `Default` is derived so an ADDITIVE field costs no call-site churn: every construction site can
/// spread `..Default::default()` and keep compiling the next time this struct grows. Adding
/// `updated_by`/`updated_ms` broke a dozen struct literals across the suite, which is a poor tax to
/// pay twice.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Policy {
    pub prefix: String,
    pub raw_for_ms: u64,
    /// FIFO cap on retained raw samples per series. `0` = unbounded.
    ///
    /// `#[serde(default)]` alone is NOT enough: the projection in [`list_policies`] names the column
    /// explicitly, so a row written **before this field existed** comes back as `max_samples: NONE`
    /// — a present-but-null value, which `default` never sees and `u64` refuses ("expected a 64-bit
    /// unsigned integer, found None"). The failure is not local: `run_gc` opens with
    /// `list_policies`, so ONE pre-cap row on an upgraded node aborted that workspace's entire
    /// retention pass. Coalescing NONE to the unbounded default keeps an older row meaning exactly
    /// what it meant when it was written. Pinned by `host/tests/series_prior_state_test`.
    #[serde(default, deserialize_with = "none_as_default")]
    pub max_samples: u64,
    /// Same NONE-vs-absent hazard as `max_samples` above, for the same reason.
    #[serde(default, deserialize_with = "none_as_default")]
    pub tiers: Vec<Tier>,
    /// Write-time predicates applied at COMMIT (never at staging append). `None` = store everything.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<Filter>,
    /// PROVENANCE: the principal that last wrote this row, and when (epoch ms).
    ///
    /// Stamped host-side from the authenticated caller and **never caller-supplied** — the same
    /// posture as the producer root on `ingest.write`, and for the same reason: a field a caller can
    /// forge answers nothing. `None` on every row written before this existed, which is honest —
    /// "we do not know" is the truth for those, and inventing an author would be worse.
    ///
    /// This exists because "who set this policy, and did they mean to drop the tier method?" was
    /// asked of a live node and could only be answered by ELIMINATING every writer in three repos.
    /// A policy is a data-lifecycle decision; it should say who made it.
    ///
    /// `Option` rather than `#[serde(default)]` on a bare value: the explicit projection in
    /// [`list_policies`] returns a column an older row never wrote as a PRESENT null, which
    /// `Option` deserializes to `None` correctly — the `none_as_default` dance the non-Option
    /// fields need does not apply here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_ms: Option<u64>,
}

/// Deserialize a field that may arrive as `NONE` (a column an older row never wrote, projected by
/// name) as its type's default. `#[serde(default)]` covers an ABSENT key; this covers a PRESENT null
/// one — the two are different bugs and only one of them survives an upgrade.
fn none_as_default<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

impl Policy {
    /// The tier at exactly `width_ms`, if the policy declares one.
    pub fn tier_at(&self, width_ms: u64) -> Option<&Tier> {
        self.tiers.iter().find(|t| t.width_ms == width_ms)
    }

    /// The method a bucketed read at `width_ms` should use: the tier at exactly that width, else the
    /// FINEST tier that declares one.
    ///
    /// **Why not exact-match only.** A method says how a bucket's samples become one value — it is a
    /// property of the SERIES' meaning, not of a particular width. A tier's width only decides what
    /// is physically stored. Requiring an exact match means a coil configured `last` reads as a step
    /// chart at exactly 15 min and, the moment a dashboard zooms to 1 min, resolves no method at all
    /// and the caller falls back to `avg` — averaging a coil, which is precisely the nonsense `last`
    /// exists to prevent. Every method here is exact at any width (they re-aggregate from stored
    /// stats or a kept representative), so applying the configured one at the read's width is both
    /// safe and what the operator asked for. Verified live: a 60 s read of a 900 s `avg` tier
    /// returned no `value` at all before this.
    pub fn method_for(&self, width_ms: u64) -> Option<Method> {
        if let Some(m) = self.tier_at(width_ms).and_then(|t| t.method) {
            return Some(m);
        }
        self.tiers
            .iter()
            .filter(|t| t.method.is_some())
            .min_by_key(|t| t.width_ms)
            .and_then(|t| t.method)
    }
}

/// The policy governing `series`: the LONGEST matching prefix, or `None` if no policy covers it.
///
/// One series is governed by exactly ONE policy. Without this rule a series under both `fleet.` and
/// `fleet.eu.` would be processed twice and the tighter bound would win *by accident* — with a
/// filter or a count cap in play, that ambiguity silently discards real samples. The GC pass and the
/// commit filter resolve precedence through this one function so they can never disagree.
pub fn resolve_policy<'a>(policies: &'a [Policy], series: &str) -> Option<&'a Policy> {
    policies
        .iter()
        .filter(|p| series.starts_with(&p.prefix))
        .max_by_key(|p| p.prefix.len())
}

/// Upsert the policy at its prefix (one policy per prefix; a re-set overwrites).
pub async fn set_policy(store: &Store, ws: &str, policy: &Policy) -> Result<(), StoreError> {
    store
        .query_ws(
            ws,
            &format!("UPSERT type::thing('{RETENTION_TABLE}', $prefix) CONTENT $row"),
            vec![
                ("prefix".into(), Value::String(policy.prefix.clone())),
                ("row".into(), json!(policy)),
            ],
        )
        .await?;
    Ok(())
}

/// All policies in `ws`, ordered by prefix.
pub async fn list_policies(store: &Store, ws: &str) -> Result<Vec<Policy>, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            // Every policy field is projected explicitly — a field added to `Policy` but NOT to this
            // list reads back as its serde default forever (the closed-struct trap: the row on disc
            // is correct, the struct in memory silently isn't).
            &format!(
                "SELECT prefix, raw_for_ms, max_samples, tiers, filter, updated_by, updated_ms \
                 FROM {RETENTION_TABLE} \
                 ORDER BY prefix ASC"
            ),
            vec![],
        )
        .await?;
    resp.take(0).map_err(|e| StoreError::Decode(e.to_string()))
}

/// Delete the policy at `prefix` (series covered by it revert to keep-forever).
pub async fn delete_policy(store: &Store, ws: &str, prefix: &str) -> Result<(), StoreError> {
    store
        .query_ws(
            ws,
            &format!("DELETE type::thing('{RETENTION_TABLE}', $prefix)"),
            vec![("prefix".into(), Value::String(prefix.to_string()))],
        )
        .await?;
    Ok(())
}
