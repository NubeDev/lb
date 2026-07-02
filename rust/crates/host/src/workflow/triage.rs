//! `triage` — ask the S5 central agent to read the issue (+ granted substrate) and draft a scope
//! doc, then share it to the team and post a summary to the channel (vision §3 steps 2–4,
//! coding-workflow scope).
//!
//! The workflow does NOT reason itself — it calls the agent over the SAME `invoke` path an edge user
//! uses (edge-invoke parity, vision §5.7), so the agent's `agent ∩ caller` scoping and routed seam
//! are unchanged. The drafted body becomes a first-class workspace **doc** (`put_doc`), shared to the
//! `team` (`share_doc`) — both as the CALLER (ownership/ membership resolve as the caller, S4). A
//! short summary is posted to the channel as **motion** (the durable record is the doc, §3.3).
//!
//! Authorization: `mcp:workflow.triage:call`, workspace-first (the deny path), then the agent's own
//! invoke gate + intersection inside `invoke`, then the asset/channel gates inside their verbs.

use lb_auth::Principal;
use lb_inbox::Item;

use super::authorize::authorize_workflow;
use super::error::WorkflowError;
use crate::agent::{invoke, AllowedTool, Invocation, ModelAccess};
use crate::assets::{put_doc, share_doc};
use crate::boot::Node;
use crate::channel::post;

/// What `triage` produces: the id of the shared scope doc.
pub struct Triaged {
    pub scope_doc: String,
}

/// Triage issue `issue_id` in workspace `ws` as `caller`: drive the agent to draft a scope doc into
/// `doc_id`, share it to `team`, and post a summary to `channel`. `agent_caps` is the agent actor's
/// own grant (effective = `agent_caps ∩ caller`). `tools`/`skill`/`doc` are the agent's substrate.
#[allow(clippy::too_many_arguments)]
pub async fn triage<M: ModelAccess>(
    node: &Node,
    model: &M,
    caller: &Principal,
    agent_caps: &[String],
    ws: &str,
    issue_id: &str,
    channel: &str,
    doc_id: &str,
    team: &str,
    skill: Option<&str>,
    tools: &[AllowedTool],
    ts: u64,
) -> Result<Triaged, WorkflowError> {
    authorize_workflow(caller, ws, "triage")?;

    // Drive the agent (same routed path as an edge user). Its answer is the drafted scope body.
    let goal = format!("Triage GitHub issue {issue_id} and draft a scope doc for it.");
    let body = invoke(
        node,
        model,
        caller,
        agent_caps,
        ws,
        Invocation {
            job_id: &format!("triage-{issue_id}"),
            goal: &goal,
            skill,
            doc: None,
            tools,
            ts,
        },
    )
    .await?;

    // The draft becomes a first-class doc, owned by the caller, shared to the team (S4). The
    // body is markdown (document-store typed content); an empty tag list is the v1 flat set.
    let title = format!("Scope: issue {issue_id}");
    put_doc(
        &node.store,
        caller,
        ws,
        doc_id,
        &title,
        &body,
        lb_assets::ContentType::Markdown,
        &[],
        ts,
    )
    .await
    .map_err(asset_err)?;
    share_doc(&node.store, caller, ws, doc_id, team)
        .await
        .map_err(asset_err)?;

    // A short summary to the channel — motion; the durable record is the doc.
    let summary = Item::new(
        format!("triage-{issue_id}"),
        channel,
        caller.sub(),
        format!("Triaged issue {issue_id} → scope doc {doc_id} shared to {team}"),
        ts,
    );
    let _ = post(node, caller, ws, channel, summary)
        .await
        .map_err(channel_err)?;

    Ok(Triaged {
        scope_doc: doc_id.to_string(),
    })
}

fn asset_err(e: crate::assets::AssetError) -> WorkflowError {
    use crate::assets::AssetError;
    match e {
        AssetError::Denied => WorkflowError::Denied,
        AssetError::TooLarge => WorkflowError::Denied,
        AssetError::Reserved => WorkflowError::Denied,
        AssetError::NotFound => WorkflowError::NotFound,
        AssetError::Store(s) => WorkflowError::Store(s),
    }
}

fn channel_err(e: crate::channel::ChannelError) -> WorkflowError {
    use crate::channel::ChannelError;
    match e {
        ChannelError::Denied => WorkflowError::Denied,
        ChannelError::Store(s) => WorkflowError::Store(s),
        other => WorkflowError::Store(lb_store::StoreError::Decode(other.to_string())),
    }
}
