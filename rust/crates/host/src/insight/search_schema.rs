//! The insight search's tag indexes, built ONCE per workspace per node — never once per request.
//!
//! **Why this exists.** The first search in a workspace has to define one full-text index per
//! configured search column's tag keys (`BootConfig::insight_search_columns`), and building them is slow (seconds on a
//! small store). The Detections page sends several reads the moment a search starts (the page, the
//! severity counts, the tag band), and each one defining the same indexes in its own transaction made
//! them conflict: measured on a fresh on-disk store, five concurrent first searches ALL failed after
//! ~9 s with `Transaction write conflict`. So one caller builds and every other caller waits for that
//! build (`tokio::sync::OnceCell`), and a failed build is retried a bounded number of times, which is
//! safe because every statement is `IF NOT EXISTS`.
//!
//! The node also starts the build for its boot workspace in the background at boot, so the first
//! reader does not pay for it. A failed build leaves the cell empty, so the next search tries again.
//!
//! One responsibility: build a workspace's search indexes once, safely under concurrency.

use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use lb_store::Store;
use tokio::sync::OnceCell;

use super::error::InsightSvcError;

/// Attempts at a build before the caller sees the error.
const ATTEMPTS: u32 = 3;
/// The wait before the first retry; doubled before each later one.
const FIRST_BACKOFF: Duration = Duration::from_millis(200);

/// Per-workspace "the search indexes exist" flags, owned by the [`crate::boot::Node`].
#[derive(Debug, Default)]
pub struct SearchSchema {
    ready: DashMap<String, Arc<OnceCell<()>>>,
}

impl SearchSchema {
    /// Ensure the `columns`' indexes exist in `ws`. Concurrent callers share ONE build; once it
    /// succeeded, this is a map lookup. `columns` is the node's fixed configured list, so one cell per
    /// workspace is enough.
    pub async fn ensure(
        &self,
        store: &Store,
        ws: &str,
        columns: &[Vec<String>],
    ) -> Result<(), InsightSvcError> {
        if columns.is_empty() {
            return Ok(());
        }
        // Clone the cell out: never hold a map guard across the await below.
        let cell = self.ready.entry(ws.to_string()).or_default().clone();
        cell.get_or_try_init(|| build(store, ws, columns)).await?;
        Ok(())
    }
}

/// Define the indexes, retrying a store error with exponential backoff.
async fn build(store: &Store, ws: &str, columns: &[Vec<String>]) -> Result<(), InsightSvcError> {
    let mut wait = FIRST_BACKOFF;
    let mut attempt = 1;
    loop {
        match lb_insights::ensure_search_indexes(store, ws, columns).await {
            Ok(()) => return Ok(()),
            Err(e) if attempt < ATTEMPTS => {
                tracing::warn!(ws, attempt, error = %e, "insight search indexes not built yet; retrying");
                tokio::time::sleep(wait).await;
                wait *= 2;
                attempt += 1;
            }
            Err(e) => return Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Many first searches at once share one build and all succeed (the failure this file fixes).
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_first_callers_share_one_build() {
        let store = Store::memory().await.unwrap();
        let schema = Arc::new(SearchSchema::default());
        let keys: Vec<Vec<String>> = ["tag:site", "tag:state", "tag:subsystem"]
            .map(|k| vec![k.to_string()])
            .to_vec();
        // Collected first: every call is spawned (and racing) before any is awaited.
        let calls: Vec<_> = (0..8)
            .map(|_| {
                let (schema, store, keys) = (schema.clone(), store.clone(), keys.clone());
                tokio::spawn(async move { schema.ensure(&store, "ops", &keys).await })
            })
            .collect();
        for call in calls {
            call.await
                .unwrap()
                .expect("every concurrent first search succeeds");
        }
        assert!(schema.ready.get("ops").is_some_and(|c| c.initialized()));
    }
}
