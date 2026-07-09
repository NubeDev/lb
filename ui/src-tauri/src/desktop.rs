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

    // `full` feature: default to a DURABLE per-user store so a restart keeps the user's work
    // (desktop-persistent-store scope). Fill `LB_STORE_PATH` here — at the windowed binary boundary,
    // BEFORE `NodeHandle::boot` — so `Node::boot`→`open_store` opens the persistent SurrealKV engine.
    // Not inside `NodeHandle::boot`: the command/integration tests call that directly and must stay
    // in-memory + isolated. An explicit `LB_STORE_PATH` (or an explicit empty one) is honored as-is.
    #[cfg(feature = "full")]
    println!("full: {}", lazybones_shell::store::ensure_store_path());

    let handle = rt
        .block_on(NodeHandle::boot("acme"))
        .expect("node boots for the shell");

    // `full` feature: mount the SSE/HTTP gateway in-process on a loopback port + run the
    // boot seeders so the packaged shell is a 100% standalone node (login, MCP, SSE, the
    // lot) with no external node to talk to. The webview talks to this loopback origin over
    // HTTP exactly as the browser does against `make dev` (its `VITE_GATEWAY_URL` is baked
    // to match). The serve task is held for the app's life — dropping it stops serving.
    // Thin shell (`desktop` only, the default): this block is absent; the UI uses Tauri IPC.
    #[cfg(feature = "full")]
    let _gateway = {
        // Capture the node + ws off the handle before it moves into `shared` (full needs to
        // mount the gateway onto the SAME in-process node the IPC commands reach).
        let node = handle.node.clone();
        let ws = handle.ws.clone();
        let addr = lazybones_shell::full::resolve_addr();
        // A bind failure (port taken) is a hard error for the full mode — the app is useless
        // without the loopback gateway. Surface it but still open the window so the operator
        // sees the message rather than a silent exit; every UI call then fails loudly.
        match rt.block_on(lazybones_shell::full::boot_full(node, &ws, addr)) {
            Ok(jh) => Some(jh),
            Err(e) => {
                eprintln!("full: loopback gateway failed to bind {addr}: {e}");
                None
            }
        }
    };

    let shared: Shared = Arc::new(Mutex::new(handle));

    tauri::Builder::default()
        .manage(shared)
        .invoke_handler(tauri::generate_handler![
            channel_post,
            channel_history,
            channel_edit,
            channel_delete,
            agent_invoke
        ])
        .run(tauri::generate_context!())
        .expect("error running the Lazybones shell");
}
