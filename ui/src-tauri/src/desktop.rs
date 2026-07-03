//! The windowed Tauri wiring (only compiled with `--features desktop`). It boots the node
//! into managed state and exposes the command layer as Tauri `#[command]`s, then runs the
//! window. Kept apart from the testable command logic so the headless build never pulls in
//! the webkit toolchain.
//!
//! The `#[tauri::command]` wrappers are intentionally one-liners over `lazybones_shell`'s
//! library verbs — the same `channel_post` / `channel_history` the TS client invokes.

use std::sync::Arc;

use lazybones_shell::{
    agent_invoke as lib_agent_invoke, channel_delete as lib_delete, channel_edit as lib_edit,
    channel_history as lib_history, channel_post as lib_post, AgentResult, NodeHandle,
};
use lb_inbox::Item;
use tokio::sync::Mutex;

type Shared = Arc<Mutex<NodeHandle>>;

// The command names below MUST match the TS `invoke("channel_post", …)` / `channel_history` /
// `channel_edit` / `channel_delete`.
#[tauri::command]
async fn channel_post(
    state: tauri::State<'_, Shared>,
    channel: String,
    item: Item,
) -> Result<Item, String> {
    let handle = state.lock().await;
    lib_post(&handle, &channel, item).await
}

#[tauri::command]
async fn channel_history(
    state: tauri::State<'_, Shared>,
    channel: String,
) -> Result<Vec<Item>, String> {
    let handle = state.lock().await;
    lib_history(&handle, &channel).await
}

#[tauri::command]
async fn channel_edit(
    state: tauri::State<'_, Shared>,
    channel: String,
    id: String,
    body: String,
    ts: u64,
) -> Result<Item, String> {
    let handle = state.lock().await;
    lib_edit(&handle, &channel, &id, &body, ts).await
}

#[tauri::command]
async fn channel_delete(
    state: tauri::State<'_, Shared>,
    channel: String,
    id: String,
) -> Result<(), String> {
    let handle = state.lock().await;
    lib_delete(&handle, &channel, &id).await
}

// active-agent-wiring Slice 5: the desktop peer of the gateway's `POST /agent/invoke`. Drives the
// workspace's ACTIVE agent (no runtime) and returns `{ jobId, answer }`. `ws`/`author`/`caps` the TS
// client also passes are ignored — the session principal + workspace are the wall (like the gateway).
#[tauri::command]
async fn agent_invoke(
    state: tauri::State<'_, Shared>,
    goal: String,
    job_id: Option<String>,
    skill: Option<String>,
    doc: Option<String>,
) -> Result<AgentResult, String> {
    let handle = state.lock().await;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    lib_agent_invoke(
        &handle,
        &goal,
        job_id.as_deref(),
        skill.as_deref(),
        doc.as_deref(),
        ts,
    )
    .await
}

pub fn run() {
    // Boot the node before the window comes up (the shell IS a node).
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let handle = rt
        .block_on(NodeHandle::boot("acme"))
        .expect("node boots for the shell");
    let shared: Shared = Arc::new(Mutex::new(handle));

    tauri::Builder::default()
        .manage(shared)
        .invoke_handler(tauri::generate_handler![
            channel_post,
            channel_history,
            channel_edit,
            channel_delete
        ])
        .run(tauri::generate_context!())
        .expect("error running the Lazybones shell");
}
