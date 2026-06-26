//! The host — the kernel that wires the spine together (core scope, README §4).
//!
//! Boots the embedded store (SurrealDB) and bus (Zenoh peer), builds the runtime engine,
//! loads extensions through the loader + runtime, and registers their tools in the MCP
//! registry. Role selection is config (symmetric nodes, §3.1) — the host itself has no
//! `if cloud`; the `node` binary picks which role crates to mount.
//!
//! What the host exposes is the *spine*: a [`Node`] holding store + bus + the MCP registry,
//! and `load_extension` to bring a component online. Tool calls go through `lb_mcp::call`.

mod agent;
mod assets;
mod boot;
mod channel;
mod install;
mod installed;
mod load;
mod native;
mod registry;
mod reload;
mod remote;
mod role;
mod serve;
mod sync;
mod workflow;

pub use agent::{
    agent_call_key, invoke, invoke_remote, resume, run_session, serve_agent, AgentError,
    AgentInvokeReply, AgentInvokeRequest, AgentServer, AllowedTool, CallOutcome, Invocation,
    ModelAccess, ProposedCall, Turn, MAX_STEPS,
};
pub use assets::{
    add_member, call_asset_tool, get_doc, grant_skill, link_doc, list_docs, load_skill, put_doc,
    put_skill, revoke_skill, share_doc, AssetError,
};
pub use boot::{Node, NodeError};
pub use channel::{
    history, join, post, subscribe_channel, watch, ChannelError, ChannelPresence, ChannelSub,
    PresenceFeed,
};
pub use install::install_extension;
pub use installed::installed;
pub use load::{load_extension, LoadError, Loaded};
pub use native::{
    authorize_native, build_spec, call_native_tool, call_sidecar, install_native, read_status,
    restart_native, status_native, stop_native, Lifecycle, NativeServiceError, NativeStatus,
    SidecarMap, Supervised,
};
pub use registry::{
    authorize_registry, cache_artifact, call_registry_tool, install_from_registry,
    install_native_from_registry, list_catalog, pull, read_cached, record_catalog,
    resolve as resolve_catalog, RegistryServiceError, Source,
};
pub use reload::reload_extension;
pub use remote::register_remote_extension;
pub use role::Role;
pub use serve::{serve_ext, ToolServer};
pub use sync::{replay_history, sync_channel, ChannelSync};
pub use workflow::{
    call_workflow_tool, emit_effect, ingest_issue, ingest_via_bridge, relay_outbox,
    request_approval, resolve_approval, start_coding_job, triage, CodingJob, RelayPass, Target,
    Triaged, WorkflowError, APPROVAL_CHANNEL, TRIAGE_CHANNEL,
};
