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
mod agent_reactor;
mod apikey;
mod assets;
mod authz;
mod boot;
mod bus;
mod callback;
mod channel;
mod channel_registry;
mod dashboard;
mod dbview;
mod devkit;
mod ext;
mod federation;
mod flows;
mod host_tools;
mod identity;
mod inbox;
mod ingest;
mod install;
mod installed;
mod load;
mod members;
mod membership;
mod native;
mod outbox;
mod prefs;
mod query;
mod registry;
mod reload;
mod reminder;
mod remote;
mod render_templates;
mod role;
mod rules;
mod run_events;
mod serve;
mod store_query;
mod sync;
mod system;
mod tags;
mod teams;
mod telemetry;
mod tool_call;
mod tools;
mod ui_decl;
mod undo;
mod undo_capture;
mod users;
mod viz;
mod workflow;
mod workspaces;

pub use agent::{
    activate_skill, agent_call_key, agent_config_get, agent_config_set, call_agent_tool,
    cancel_run, decision_id, evaluate_policy, format_catalog, invoke, invoke_descriptor,
    invoke_remote, invoke_via_runtime, list_runtimes, load_decision, load_policy, rehydrate,
    render_catalog, resolve_effective_runtime, resume, run_session, save_policy, serve_agent,
    settle_decision, Activation, AgentConfig, AgentDecision, AgentError, AgentInvokeReply,
    AgentInvokeRequest, AgentRuntime, AgentServer, AllowedTool, ArgMatch, CallOutcome,
    DecisionState, Effect, ErasedModel, InHouseRuntime, Invocation, LoopState, ModelAccess,
    ModelEndpointPatch, Policy, ProposedCall, Rule, RunContext, RuntimeRegistry, SettleOutcome,
    Substrate, Turn, UnconfiguredModel, AGENT_CONFIG_TABLE, DECISION_APPROVAL_CHANNEL,
    DECISION_TABLE, DEFAULT_RUNTIME, DENIED_BY_POLICY, MAX_STEPS, POLICY_TABLE, SKILL_ACTIVATE,
};
/// The background driver for detached channel agent runs (run-lifecycle #5): `spawn_agent_reactors`
/// is the node-boot entry (beside `spawn_flow_reactors`); `drain_channel_agent_runs` is a synchronous,
/// deterministic drain a test (or a caller wanting an immediate flush) drives without the timer.
pub use agent_reactor::{
    drain_channel_agent_runs, drain_channel_agent_runs_with_ceiling, spawn_agent_reactors,
};
pub use apikey::{
    apikey_authenticate, apikey_create, apikey_get, apikey_list, apikey_revoke, apikey_rotate,
    ensure_builtin_roles, is_auth_failure, ApiKeyCache, ApiKeyError, ApiKeyFull, ApiKeyView,
    KINDS as APIKEY_KINDS, KIND_DISCRIM as APIKEY_KIND_DISCRIM, TABLE as APIKEY_TABLE,
    TOMBSTONE_STATUS as APIKEY_TOMBSTONE_STATUS,
};
pub use assets::{
    add_member, backlinks, call_asset_tool, delete_asset, delete_doc, get_asset, get_doc,
    grant_skill, link_doc, list_assets, list_docs, list_granted_skills, load_skill, put_asset,
    put_doc, put_skill, revoke_skill, share_doc, unshare_doc, AssetError, SkillCatalogEntry,
    MAX_ASSET_BYTES,
};
pub use authz::{
    authz_resolve, call_authz_tool, grants_assign, grants_list, grants_revoke, resolve_caps,
    revoke_subject, revoke_tokens, roles_define, roles_delete, roles_list, teams_create,
    teams_list, token_revoked, AuthzError, AuthzRole, CapSource, Grant, SourcedCap, Subject, Team,
};
pub use boot::{Node, NodeError};
pub use bus::{
    authorize_bus, bus_publish, bus_watch, call_bus_tool, wall_subject, BusError, BusSub,
};
pub use channel::{
    call_channel_chart_pref_tool, delete, edit, history, join, post, subscribe_channel, watch,
    watch_deletions, ChannelError, ChannelPresence, ChannelSub, DeletionFeed, PresenceFeed,
};
pub use channel_registry::{channel_create, channel_list, register_on_post, ChannelRecord};
pub use dashboard::{
    call_dashboard_tool, dashboard_delete, dashboard_get, dashboard_list, dashboard_save,
    dashboard_share, seed_iot_demo, Action, Cell, Dashboard, DashboardError, DashboardSummary,
    SeedReport, Source as CellSource, Target as CellTarget, Variable as DashboardVariable,
    Visibility as DashboardVisibility, MAX_OVERRIDES as DASHBOARD_MAX_OVERRIDES,
    MAX_TRANSFORMS as DASHBOARD_MAX_TRANSFORMS,
};
pub use dbview::{
    authorize_dbview, call_dbview_tool, store_graph_view, store_scan_view, store_tables_view,
    DbViewError, Graph, GraphEdge, GraphNode, Page, Row, TableCount,
};
pub use devkit::{
    authorize_devkit, call_devkit_tool, devkit_build, devkit_inspect, devkit_root, devkit_scaffold,
    devkit_templates, BuildStarted, DevkitError, DevkitRoot,
};
pub use ext::{
    call_ext_tool, ext_disable, ext_enable, ext_list, ext_publish, ext_uninstall, load_enabled,
    reconcile, ExtError, ExtRow, LoadedExt, ReconcileAction, ReconcilePlan,
};
pub use federation::{
    call_federation_tool, datasource_add, datasource_list, datasource_remove, datasource_test,
    federation_mirror, federation_query, resolve_datasource, Datasource, DatasourceSummary,
    FederationError,
};
pub use flows::error::FlowsError;
pub use flows::{
    arm_source, cron_is_valid, cron_run_id, disarm_source, placement_matches, react_to_flows_cron,
    reconcile_flows, source_series, spawn_flow_reactors, watch_flow_run, FlowReactorPass,
    FlowReconcilePass, FlowWatch,
};
pub use flows::{call_flows_tool, call_flows_tool_boxed};
/// Run-engine seams exposed for the runtime-control tests (deterministic mid-run cancel): seed a run,
/// set its durable status, and drive it — so a test can prove the drive halts on a pre-written
/// `cancelled` without a spawn race.
pub mod flow_engine {
    pub use crate::flows::coordinator::{drive, start};
    pub use crate::flows::run_store::set_run_status;
}
pub use host_tools::{
    call_host_tool, call_secret_tool, host_fs_list, host_fs_stat, host_net_info, host_net_reach,
    host_time_now, host_time_zones, HostFsEntry, HostFsList, HostFsStat, HostNetAddress,
    HostNetInfo, HostNetInterface, HostNetReach, HostTimeNow, HostTimeZones, HOST_FS_LIST_LIMIT,
    HOST_NET_REACH_DEFAULT_TIMEOUT_MS, HOST_NET_REACH_MAX_TIMEOUT_MS,
};
pub use identity::{
    call_identity_tool, identity_create, identity_get, identity_list, identity_workspaces,
    IdentityError, IdentityView, IdentityWorkspace,
};
pub use inbox::{list_inbox, record_inbox, resolve_inbox, InboxError};
pub use ingest::{
    authorize_ingest, call_ingest_tool, drain_workspace, ingest_write, publish_sample, series_find,
    series_latest_value, series_list, series_read_range, subscribe_series, DrainPass, IngestError,
    Qos, Sample, SeriesSub, COMMIT_BATCH, DEFAULT_STAGING_BOUND, MAX_SERIES_LIST,
};
pub use install::install_extension;
pub use installed::installed;
pub use load::{load_extension, LoadError, Loaded};
pub use members::{add_team_member, list_members, remove_member, MembersError};
pub use membership::{
    call_membership_tool, membership_add, membership_list, membership_login_resolve,
    membership_remove, MembershipError, MembershipView, WORKSPACE_ADMIN_ROLE_CAP,
};
pub use native::{
    authorize_native, build_spec, call_native_tool, call_sidecar, install_native, read_status,
    restart_native, status_native, stop_native, Lifecycle, NativeServiceError, NativeStatus,
    SidecarMap, Supervised,
};
pub use outbox::{enqueue_outbox, outbox_status, OutboxError, OutboxStatus};
pub use prefs::{
    authorize_prefs, call_catalog_tool, call_format_tool, call_prefs_catalog_tool, call_prefs_tool,
    catalog_changed_subject, message_render, message_set_catalog, prefs_catalog, prefs_get,
    prefs_resolve, prefs_set, prefs_set_default, CatalogView, PrefsSvcError,
};
pub use query::{
    call_query_tool, compile_descriptor, query_compile, query_delete, query_get, query_list,
    query_run, query_save, resolve_query, run_descriptor, save_descriptor, QueryError,
    QuerySummary, QueryTarget, RunSource, SavedQuery, TABLE as QUERY_TABLE,
};
pub use registry::{
    authorize_registry, cache_artifact, call_registry_tool, install_from_registry,
    install_native_from_registry, list_catalog, pull, read_cached, record_catalog,
    resolve as resolve_catalog, RegistryServiceError, Source,
};
pub use reload::reload_extension;
pub use reminder::{
    call_reminder_tool, fire_job_id, fire_reminder, react_to_reminders, reminder_create,
    reminder_delete, reminder_fire, reminder_get, reminder_list, reminder_update,
    Action as ReminderAction, ReactorPass as ReminderReactorPass, Reminder, ReminderError,
    ReminderPatch, ReminderStatus, FIRE_KIND as REMINDER_FIRE_KIND,
};
pub use remote::register_remote_extension;
pub use render_templates::{
    call_template_tool, template_delete, template_get, template_list, template_save, Engine,
    RenderTemplate, RenderTemplateError, RenderTemplateSummary, INLINE_MAX_BYTES,
    TEMPLATE_MAX_BYTES,
};
pub use role::Role;
pub use rules::{
    ai_limits, call_rules_tool, params_to_rhai, rule_limits, rules_delete, rules_get, rules_list,
    rules_run, rules_save, workspace_datasources, workspace_queries, HostAiSeam, HostDataSeam,
    RuleModel, RulesError, RunResult, SavedRule,
};
pub use run_events::{publish_run_event, run_subject, watch_run, RunEventSub, RunWatch};
pub use serve::{serve_ext, ToolServer};
pub use store_query::{
    authorize_store_query, call_store_query_tool, ensure_read_only, store_query_run,
    store_schema_read, QueryResult, Schema, SchemaColumn, SchemaTable, StoreQueryError,
    MAX_QUERY_ROWS, QUERY_TIMEOUT_SECS,
};
pub use sync::{replay_history, sync_channel, ChannelSync};
pub use system::{
    authorize_system, call_system_tool, system_acp, system_overview, system_subsystem,
    system_tools, system_topology, AcpInfo, Health, Metric, ServiceStatus, SubsystemDetail,
    SystemError, SystemOverview, SystemTools, SystemTopology, ToolInfo, TopoEdge, TopoNode,
};
pub use tags::{
    authorize_tags, call_tags_tool, tags_add, tags_find, tags_of, tags_remove, Applied, Facet,
    Provenance, Source as TagSource, Tag, TagsError,
};
pub use teams::{call_teams_tool, teams_delete, teams_rename, TeamsError};
pub use telemetry::{
    authorize_telemetry, call_telemetry_tool, read_or_admin_cap, telemetry_purge, telemetry_query,
    telemetry_seed, telemetry_tail, telemetry_trace, QueryFilter, QueryPage, TailSnapshot, TailSub,
    TelemetryRow, TelemetrySvcError, TELEMETRY_TABLE,
};
pub use tool_call::call_tool;
pub use tools::{call_tools_tool, tools_catalog, ToolsCatalog};
pub use undo::{history_compensations, history_list, redo, undo, UndoSvcError};
pub use users::{
    call_users_tool, user_create, user_delete, user_disable, user_enable, user_list,
    user_login_check, UserView, UsersError,
};
pub use viz::{call_viz_tool, viz_query, VizError};
pub use workflow::{
    call_workflow_tool, emit_effect, enabled_workspaces, ingest_issue, ingest_via_bridge, pr_spec,
    react_to_approvals, reactor_job_id, record_pr_spec, relay_outbox, request_approval,
    resolve_approval, start_coding_job, triage, CodingJob, EntryStatus, PrSpec, ReactorPass,
    RelayPass, Target, Triaged, WorkflowError, WorkspaceEntry, APPROVAL_CHANNEL, DIRECTORY_NS,
    TRIAGE_CHANNEL,
};
pub use workspaces::{
    call_workspaces_tool, workspace_create, workspace_delete, workspace_list, workspace_purge,
    workspace_rename, WorkspaceRecord, WorkspaceStatus, WorkspacesError,
};
// The workflow **directory** register/deregister verbs — prefixed at the crate boundary so the public
// API names the concept (a bare `register` would be ambiguous next to `register_remote_extension`).
pub use workflow::{deregister as deregister_workspace, register as register_workspace};
