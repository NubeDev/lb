//! `HostJobSeam` — the durable checkpoint/progress boundary for a job-backed rule run
//! (long-running-rules-scope), implemented over the `lb-jobs` transcript. The evaluating thread is
//! the only transcript writer during a run, so index allocation is a plain per-seam counter seeded
//! from the job's `cursor` at spawn. Sync→async bridged with `block_on`, like the other host seams.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use lb_jobs::TranscriptEvent;
use lb_rules::seam::{JobSeam, SeamError};

use crate::boot::Node;

pub struct HostJobSeam {
    node: Arc<Node>,
    ws: String,
    run_id: String,
    handle: tokio::runtime::Handle,
    /// The next transcript index to append (seeded from `job.cursor` at spawn; resume-safe).
    next: AtomicU32,
}

impl HostJobSeam {
    pub fn new(
        node: Arc<Node>,
        ws: String,
        run_id: String,
        handle: tokio::runtime::Handle,
        cursor: u32,
    ) -> Self {
        Self {
            node,
            ws,
            run_id,
            handle,
            next: AtomicU32::new(cursor),
        }
    }

    fn append(&self, event: TranscriptEvent) -> Result<(), SeamError> {
        let index = self.next.fetch_add(1, Ordering::AcqRel);
        self.handle
            .block_on(lb_jobs::append_event(
                &self.node.store,
                &self.ws,
                &self.run_id,
                index,
                event,
            ))
            .map_err(|e| SeamError::Failed(format!("job transcript write failed: {e}")))
    }

    /// Append a reserved settle checkpoint (`__result`/`__error`) from the async drive task —
    /// the trait's sync `checkpoint` would `block_on` inside the runtime and panic; this is the
    /// same append without the bridge.
    pub async fn record_reserved(
        &self,
        key: &str,
        value: &serde_json::Value,
    ) -> Result<(), SeamError> {
        let index = self.next.fetch_add(1, Ordering::AcqRel);
        lb_jobs::append_event(
            &self.node.store,
            &self.ws,
            &self.run_id,
            index,
            TranscriptEvent::Checkpoint {
                key: key.to_string(),
                value: value.to_string(),
            },
        )
        .await
        .map_err(|e| SeamError::Failed(format!("job transcript write failed: {e}")))
    }
}

impl JobSeam for HostJobSeam {
    fn checkpoint(&self, key: &str, value: &serde_json::Value) -> Result<(), SeamError> {
        self.append(TranscriptEvent::Checkpoint {
            key: key.to_string(),
            value: value.to_string(),
        })
    }

    fn progress(&self, pct: Option<u32>, msg: &str) -> Result<(), SeamError> {
        self.append(TranscriptEvent::Progress {
            pct,
            msg: msg.to_string(),
        })
    }
}
