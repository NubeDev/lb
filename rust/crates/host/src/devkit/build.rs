use std::path::Path;
use std::sync::Arc;

use lb_auth::Principal;
use lb_bus::publish;
use lb_jobs::{complete, create, Job, JobStatus};
use serde::Serialize;
use tokio::sync::mpsc;

use crate::Node;

use super::builder::select_toolchain;
use super::{authorize_devkit, DevkitError};

#[derive(Debug, Clone, Serialize)]
pub struct BuildStarted {
    pub job_id: String,
    pub log_subject: String,
}

pub async fn devkit_build(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    path: &Path,
    ts: u64,
) -> Result<BuildStarted, DevkitError> {
    authorize_devkit(principal, ws, "build")?;
    let root = lb_devkit::default_devkit_root();
    let safe = lb_devkit::resolve_under_root(root, path)
        .map_err(|e| DevkitError::BadInput(e.to_string()))?;
    let job_id = format!("devkit-build-{ts}");
    let log_subject = format!("devkit/build/{job_id}");
    let job = Job::new(
        job_id.clone(),
        "devkit-build",
        safe.display().to_string(),
        ts,
    );
    create(&node.store, ws, &job).await?;

    // Selected by config (README §3 rule 1), not a branch `build_extension` or the job/log
    // contract knows about — devkit-container-build-scope.md.
    let toolchain = select_toolchain(&node, ws).await;

    let node = Arc::clone(node);
    let ws = ws.to_string();
    let job_id_for_task = job_id.clone();
    let rel_subject = format!("ext/{log_subject}");
    tokio::spawn(async move {
        let (tx, mut rx) = mpsc::channel::<String>(128);
        let publisher_node = Arc::clone(&node);
        let publisher_ws = ws.clone();
        let publisher_subject = rel_subject.clone();
        let publisher = tokio::spawn(async move {
            while let Some(line) = rx.recv().await {
                let payload = line_payload(&line);
                let _ = publish(
                    &publisher_node.bus,
                    &publisher_ws,
                    &publisher_subject,
                    &payload,
                )
                .await;
            }
        });
        let result = tokio::task::spawn_blocking(move || {
            lb_devkit::build_extension(&safe, toolchain.as_ref(), &mut |line| {
                let _ = tx.blocking_send(line);
            })
        })
        .await
        .unwrap_or_else(|e| Err(anyhow::anyhow!("build task failed: {e}")));
        let status = if result.is_ok() {
            let payload = line_payload("devkit build: done");
            let _ = publish(&node.bus, &ws, &rel_subject, &payload).await;
            JobStatus::Done
        } else {
            let msg = result.err().map(|e| e.to_string()).unwrap_or_default();
            let payload = line_payload(&msg);
            let _ = publish(&node.bus, &ws, &rel_subject, &payload).await;
            let payload = line_payload("devkit build: failed");
            let _ = publish(&node.bus, &ws, &rel_subject, &payload).await;
            JobStatus::Failed
        };
        let _ = publisher.await;
        let _ = complete(&node.store, &ws, &job_id_for_task, status).await;
    });

    Ok(BuildStarted {
        job_id,
        log_subject,
    })
}

fn line_payload(line: &str) -> Vec<u8> {
    serde_json::to_vec(line).unwrap_or_else(|_| b"\"log encode failed\"".to_vec())
}
