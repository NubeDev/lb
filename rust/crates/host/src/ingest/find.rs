//! `series.find(tags)` — tag-driven series discovery, built on the tag graph (ingest scope). With
//! heterogeneous payloads there is no common schema to query by, so discovery happens through the
//! tag/graph: a faceted `key:value` query returns the series entities carrying ALL the facets. This
//! is the read-side analog of the label declaration ingest converts to edges at commit.
//!
//! It reuses `lb_tags::find` (the same primitive the UI calls), then keeps only the `series:` entity
//! references — so `series.find` is "find series tagged X", not "find any entity tagged X". Gated by
//! `mcp:series.find:call`; namespace-scoped (the hard wall). Raw verb — run after `caps::check`.

use lb_auth::Principal;
use lb_store::Store;
use lb_tags::{find as tag_find, Facet};

use super::authorize::authorize_ingest;
use super::error::IngestError;

/// The record-id prefix the ingest series live under (`series:[series, producer, seq]`). Discovery
/// returns the distinct series entities tagged with all `facets`.
const SERIES_PREFIX: &str = "series:";

/// Find the series in `ws` whose entity carries ALL `facets` (faceted intersection over the tag
/// graph). Returns the matching `series:` entity references; non-series entities sharing a tag are
/// filtered out, so this answers "which series match these dimensions".
pub async fn series_find(
    store: &Store,
    principal: &Principal,
    ws: &str,
    facets: &[Facet],
) -> Result<Vec<String>, IngestError> {
    authorize_ingest(principal, ws, "series.find")?;
    let hits = tag_find(store, ws, facets).await?;
    Ok(hits
        .into_iter()
        .filter(|e| e.starts_with(SERIES_PREFIX))
        .collect())
}
