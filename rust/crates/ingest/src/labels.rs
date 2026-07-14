//! Label→tag conversion at commit — the wire `Sample.labels` become real tag-graph edges on the
//! `series:<name>` entity, ONCE per series (series schema slice). The tag service stays the single
//! source of truth for dimensions: `series.find` discovers what ingest wrote because commit now
//! actually feeds the graph, closing the "labels carried but never converted" gap.
//!
//! Provenance is `Source::Producer` with the sample's own `ts` as the logical `at` and its
//! authenticated `producer` as `by`. The once-per-series latch is `series_meta.labels_applied`.

use lb_store::{Store, StoreError};
use lb_tags::{add as tag_add, AddError, Provenance, Source, Tag, DEFAULT_TAG_NODE_CAP};

use crate::meta::{labels_applied, mark_labels_applied};
use crate::sample::Sample;

/// Convert `sample.labels` into tag edges on `series:<name>` if this series hasn't been labeled
/// yet. Non-object / empty labels are a no-op (the latch is NOT set, so a later sample that does
/// carry labels still applies them). A tag-node cap hit skips the label, never fails the commit.
pub async fn apply_labels(store: &Store, ws: &str, sample: &Sample) -> Result<(), StoreError> {
    let Some(labels) = sample.labels.as_object().filter(|m| !m.is_empty()) else {
        return Ok(());
    };
    if labels_applied(store, ws, &sample.series).await? {
        return Ok(());
    }
    let entity = format!("series:{}", sample.series);
    let prov = Provenance::new(sample.ts, sample.producer.clone(), Source::Producer);
    for (key, value) in labels {
        let tag = Tag::new(key.clone(), value.clone());
        match tag_add(store, ws, &entity, &tag, &prov, DEFAULT_TAG_NODE_CAP).await {
            Ok(()) => {}
            // Cap hit: this label is dropped (bounded by design); the commit itself proceeds.
            Err(AddError::CapExceeded(_)) => {}
            Err(AddError::Store(e)) => return Err(e),
        }
    }
    mark_labels_applied(store, ws, &sample.series).await
}
