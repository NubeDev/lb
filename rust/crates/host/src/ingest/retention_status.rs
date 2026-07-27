//! `series.retention.status` — what governs a series, and when GC last ran (series-observability
//! scope).
//!
//! An **admin-plane** read, gated by `mcp:series.retention.status:call` alongside
//! `series.retention.list`. Split from `series.stats` deliberately: one combined verb would force a
//! single capability to cover both a data read and admin bookkeeping, and would stop a client
//! degrading per fact.
//!
//! **The winning policy is resolved SERVER-SIDE.** Longest-prefix-wins is host semantics, and every
//! client that needs the effective policy was otherwise reimplementing it (rubix-ai's modbus
//! extension carried its own `effectivePolicy()`). Two implementations of one rule drift, and the
//! drift is silent — the UI would confidently name the wrong governing prefix. `matched_prefix` is
//! returned alongside the policy precisely so a caller can say *inherited from `modbus.`* rather
//! than implying the series has its own row.
//!
//! No policy is reported as `policy: None` with `matched_prefix: None` — an explicit "no policy
//! governs this series", never a fabricated default. Likewise a node that has never run GC reports
//! `last_pass: None`; a node running no retention reactors (`BootConfig::reactors` off) reports it
//! forever, which is honest and correct rather than an error.

use lb_auth::Principal;
use lb_ingest::{last_pass, list_policies, resolve_policy, GcPassRecord, Policy};
use lb_store::Store;

use super::authorize::authorize_ingest;
use super::error::IngestError;

/// The effective retention picture for one subject, plus the workspace's last GC pass.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct RetentionStatus {
    /// The subject as asked for — a series id or a bare prefix, echoed back.
    pub series: String,
    /// The policy that governs `series` after longest-prefix resolution, or `None` when no stored
    /// policy matches. Never a synthesized default.
    pub policy: Option<Policy>,
    /// The prefix of the winning policy row — the same string as `policy.prefix`, surfaced as its
    /// own field so a caller renders "inherited from X" without reaching into the policy. `None`
    /// exactly when `policy` is `None`.
    pub matched_prefix: Option<String>,
    /// The workspace's last recorded GC pass, or `None` when none has run on this node.
    pub last_pass: Option<GcPassRecord>,
    /// The host's advisory cap on an UNPOLICED series (`lb_ingest::DEFAULT_MAX_SAMPLES`). Reported
    /// so a UI explaining an ungoverned series can name the real number instead of hand-waving
    /// about "the host's default". Advisory in this release: unpoliced series are warned about, not
    /// evicted (`warn_unpoliced`).
    pub default_max_samples: u64,
}

/// The retention status of `series` in `ws`. Gated by `mcp:series.retention.status:call`.
///
/// `series` may be a full series id OR a bare prefix: longest-prefix resolution is the same
/// operation either way (a prefix is just a series id that happens to end at a boundary), so a
/// settings page asking "what governs `modbus.`" and a detail page asking "what governs this
/// series" share one verb and one code path. Splitting them would mint a second resolution site —
/// exactly the drift this verb exists to eliminate.
pub async fn series_retention_status(
    store: &Store,
    principal: &Principal,
    ws: &str,
    series: &str,
) -> Result<RetentionStatus, IngestError> {
    authorize_ingest(principal, ws, "series.retention.status")?;
    let policies = list_policies(store, ws).await?;
    let winner = resolve_policy(&policies, series);
    Ok(RetentionStatus {
        series: series.to_string(),
        matched_prefix: winner.map(|p| p.prefix.clone()),
        policy: winner.cloned(),
        last_pass: last_pass(store, ws).await?,
        default_max_samples: lb_ingest::DEFAULT_MAX_SAMPLES,
    })
}
