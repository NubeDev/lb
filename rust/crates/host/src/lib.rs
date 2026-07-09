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
mod approval_reactor;
mod assets;
mod authz;
mod boot;
mod bus;
mod callback;
mod channel;
mod channel_registry;
mod credential;
mod dashboard;
mod dbview;
mod devkit;
mod directory;
mod ext;
mod federation;
mod flows;
mod host_tools;
mod identity;
mod inbox;
mod ingest;
mod insight;
mod install;
mod installed;
mod layout;
mod load;
mod members;
mod membership;
mod native;
mod nav;
mod outbox;
mod panel;
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
mod store_mutate;
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
mod weather;
mod webhook;
mod workspaces;

pub use agent::{
    activate_skill, agent_call_key, agent_config_get, agent_config_set, agent_def_create,
    agent_def_delete, agent_def_get, agent_def_list, agent_def_test, agent_def_update,
    agent_persona_create, agent_persona_delete, agent_persona_get, agent_persona_list,
    agent_persona_update, build_identity_fold, builtin_definitions, builtin_personas,
    call_agent_catalog_tool, call_agent_memory_tool, call_agent_persona_tool, call_agent_tool,
    cancel_run, check_runtime, clamp_to_preset, decision_id, evaluate_policy, fence_into_goal,
    format_catalog, get_agent_config, glob_matches, invoke, invoke_descriptor, invoke_remote,
    invoke_via_runtime, list_runtimes, load_decision, load_policy, memory_delete, memory_get,
    memory_index_for_injection, memory_list, memory_set, migrate_active_persona, narrow_tools,
    reachable_tools, rehydrate, render_catalog, render_catalog_filtered, render_index,
    resolve_active_definition, resolve_effective, resolve_effective_runtime,
    resolve_effective_runtime_id, resolve_endpoint_key, resolve_endpoint_key_host, resolve_persona,
    resolve_workspace_model, resume, run_session, save_policy, seed_agent_definitions,
    seed_personas, serve_agent, settle_decision, Activation, AgentConfig, AgentDecision,
    AgentDefinition, AgentError, AgentInvokeReply, AgentInvokeRequest, AgentRuntime, AgentServer,
    AllowedTool, ArgMatch, CallOutcome, DecisionState, DefinitionEndpoint, DefinitionPatch, Effect,
    EffectivePersona, ErasedModel, InHouseRuntime, Invocation, LoopState, Memory, MemoryKind,
    MemoryScope, ModelAccess, ModelBuilder, ModelEndpointPatch, Persona, PersonaListItem,
    PersonaPatch, Policy, PolicyPreset, ProposedCall, Rule, RunContext, RuntimeRegistry,
    SettleOutcome, Substrate, TestContext, TestResult, Turn, UnconfiguredModel, AGENT_CONFIG_TABLE,
    AGENT_DEFS_NS, AGENT_DEFS_TABLE, BUILTIN_PREFIX, DECISION_APPROVAL_CHANNEL, DECISION_TABLE,
    DEFAULT_RUNTIME, DENIED_BY_POLICY, INJECT_CAP, MAX_BODY, MAX_CONTEXT_BYTES, MAX_DESCRIPTION,
    MAX_STEPS, MEMORY_HEADER, PERSONA_NS, PERSONA_TABLE, POLICY_TABLE, SKILL_ACTIVATE,
    UNCONFIGURED_ANSWER,
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
/// The generic approval-release reactor (rules-approvals scope): `spawn_approval_reactors` is the
/// node-boot tick (beside the flow/agent reactors); `react_to_approval_releases` is the synchronous,
/// deterministic pass a test drives directly without the timer. `held_effect_id` is the shared id
/// derivation the rule verb and the reactor agree on.
pub use approval_reactor::{
    held_effect_id, react_to_approval_releases, spawn_approval_reactors, ApprovalReleasePass,
};
pub use assets::{
    add_member, backlinks, call_asset_tool, delete_asset, delete_doc, deprecate_skill, get_asset,
    get_doc, grant_skill, link_doc, list_assets, list_docs, list_granted_skills, load_skill,
    put_asset, put_doc, put_skill, revoke_skill, share_doc, unshare_doc, AssetError,
    SkillCatalogEntry, SkillTier, MAX_ASSET_BYTES,
};
// Core-skill boot seeder (core-skills scope): the node binary calls `seed_core_skills` at boot to
// write the embedded corpus into the reserved system namespace. Re-exported so the binary reaches it
// through `lb_host` without depending on `lb_assets` directly.
pub use authz::{
    admin_only_caps, author_caps, authz_resolve, call_authz_tool, ensure_builtin_authz_roles,
    grants_assign, grants_list, grants_revoke, member_role_caps, resolve_caps, revoke_subject,
    revoke_tokens, roles_define, roles_delete, roles_list, teams_create, teams_list, token_revoked,
    viewer_role_caps, workspace_admin_role_caps, AuthzError, AuthzRole, CapSource, Grant,
    SourcedCap, Subject, Team, ROLE_MEMBER, ROLE_VIEWER, ROLE_WORKSPACE_ADMIN,
};
pub use boot::{Node, NodeError};
pub use bus::{
    authorize_bus, bus_publish, bus_watch, call_bus_tool, wall_subject, BusError, BusSub,
};
pub use channel::{
    call_channel_chart_pref_tool, call_channel_tool, delete, edit, history, join, post,
    subscribe_channel, watch, watch_deletions, ChannelError, ChannelPresence, ChannelSub,
    DeletionFeed, PresenceFeed,
};
pub use channel_registry::{channel_create, channel_list, register_on_post, ChannelRecord};
pub use dashboard::{
    builtin_view_ids, call_dashboard_tool, catalog_descriptor, check_view_cells,
    dashboard_access_check, dashboard_catalog, dashboard_delete, dashboard_get, dashboard_list,
    dashboard_pin, dashboard_save, dashboard_save_meta, dashboard_share, mint_cell_from_envelope,
    pin_descriptor,
    seed_iot_demo, AccessReport, Action, Cell, Dashboard, DashboardError, DashboardSummary,
    DepKind, DepVerdict, ExtWidget as DashboardExtWidget, SeedReport, Source as CellSource,
    Target as CellTarget, Variable as DashboardVariable, Visibility as DashboardVisibility,
    WidgetCatalog, MAX_OVERRIDES as DASHBOARD_MAX_OVERRIDES,
    MAX_TRANSFORMS as DASHBOARD_MAX_TRANSFORMS,
};
pub use dbview::{
    authorize_dbview, call_dbview_tool, store_graph_view, store_scan_view, store_tables_view,
    DbViewError, Graph, GraphEdge, GraphNode, Page, Row, TableCount,
};
pub use devkit::{
    authorize_devkit, call_devkit_tool, container_enabled, devkit_build, devkit_inspect,
    devkit_root, devkit_scaffold, devkit_templates, select_toolchain, BuildStarted, DevkitError,
    DevkitRoot,
};
pub use ext::{
    call_ext_tool, ext_disable, ext_enable, ext_list, ext_publish, ext_uninstall, load_enabled,
    reconcile, ExtError, ExtRow, LoadedExt, ReconcileAction, ReconcilePlan,
};
pub use federation::{
    call_federation_tool, datasource_add, datasource_list, datasource_remove, datasource_test,
    enforce_endpoint, federation_mirror, federation_query, install_federation, put_datasource,
    resolve_datasource, Datasource, DatasourceSummary, FederationError,
    Installed as FederationInstalled, SeedSource,
};
pub use flows::error::FlowsError;
pub use flows::{
    arm_source, cron_is_valid, cron_run_id, disarm_source, flipflop_run_id, placement_matches,
    react_to_flow_approvals, react_to_flow_sources, react_to_flows_cron, react_to_flows_interval,
    reconcile_flows, source_run_id, source_series, spawn_flow_reactors, watch_flow_debug,
    watch_flow_run, FlowApprovalPass, FlowDebugWatch, FlowReactorPass, FlowReconcilePass,
    FlowWatch, SourceReactorPass,
};
pub use flows::{call_flows_tool, call_flows_tool_boxed};
pub use lb_assets::{seed_core_skills, CORE_SKILLS_NS};
/// Run-engine seams exposed for the runtime-control tests (deterministic mid-run cancel): seed a run,
/// set its durable status, and drive it — so a test can prove the drive halts on a pre-written
/// `cancelled` without a spawn race.
pub mod flow_engine {
    pub use crate::flows::coordinator::{drive, start};
    pub use crate::flows::record::{
        ClaimState, FlowRunRecord, FlowStepRecord, FLOW_RUN_TABLE, FLOW_STEP_TABLE,
    };
    pub use crate::flows::retain_runs::{retain_runs, DEFAULT_FINISHED_RUN_CAP};
    pub use crate::flows::run_store::set_run_status;
}
pub use credential::{
    call_credential_tool, credential_verify, identity_set_credential, CredentialCheck,
    CredentialError,
};
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
/// The **insights** service — the capability-gated surface over `lb_insights` (insights umbrella
/// scope + occurrences/subscriptions/notify sub-scopes). The MCP bridge `call_insight_tool` is
/// the one contract every host-native `insight.*` verb routes through; each verb re-checks its
/// own `mcp:insight.<verb>:call` gate inside.
pub use insight::{
    call_insight_tool, insight_ack, insight_get, insight_list, insight_occurrences,
    insight_policy_get, insight_policy_set, insight_raise, insight_resolve, insight_sub_create,
    insight_sub_delete, insight_sub_get, insight_sub_list, insight_sub_mute,
    react_to_insight_digests, spawn_insight_digest_reactors, subscribe_insight_events,
    InsightSvcError, InsightWatch,
};
pub use install::install_extension;
pub use installed::installed;
pub use layout::{
    call_layout_tool, layout_get, layout_set, LayoutError, UiLayout, MAX_LAYOUT_BYTES,
};
pub use load::{load_extension, LoadError, Loaded};
pub use members::{add_team_member, list_members, remove_member, MembersError};
pub use membership::{
    call_membership_tool, membership_add, membership_list, membership_login_resolve,
    membership_remove, MembershipError, MembershipView, WORKSPACE_ADMIN_ROLE_CAP,
};
pub use native::{
    authorize_native, build_spec, call_native_tool, call_sidecar, install_native, read_status,
    record_status, reset_native, restart_native, status_native, stop_native, Lifecycle,
    NativeServiceError, NativeStatus, SidecarMap, Supervised,
};
pub use nav::{
    call_nav_tool, nav_delete, nav_get, nav_hidden_get, nav_hidden_set, nav_list, nav_list_shares,
    nav_pref_get, nav_pref_set, nav_resolve, nav_save, nav_set_default, nav_share, nav_unshare,
    reach_caps, reach_check, Nav, NavError, NavFacet, NavHidden, NavItem, NavPref, NavSummary,
    ResolvedItem as NavResolvedItem, ResolvedNav as NavResolved,
    ResolvedSource as NavResolvedSource, Visibility as NavVisibility, MAX_HIDDEN as NAV_MAX_HIDDEN,
    MAX_ITEMS as NAV_MAX_ITEMS, MAX_PINNED as NAV_MAX_PINNED, MAX_TAG_GROUP as NAV_MAX_TAG_GROUP,
    REACH_ALL,
};
pub use panel::{
    call_panel_tool, hydrate_cells, panel_delete, panel_get, panel_list, panel_save, panel_share,
    panel_usage, validate_and_strip_refs, Panel, PanelError, PanelSpec, PanelSummary,
    PanelUsageRow, Visibility as PanelVisibility,
};
pub use weather::{call_weather_tool, weather_current, WeatherCurrent, OPEN_METEO_BASE_ENV};
// The production sidecar launcher, re-exported so a caller that drives `call_sidecar` (e.g. the
// gateway's `/native/call` bridge) gets the whole native-tier surface from `lb_host` without reaching
// into `lb_supervisor` internals.
/// A fresh sortable ULID (re-exported from `lb_store`) — the gateway's event-stream hub mints its
/// per-connection `sid` with it, without taking a direct `lb-store` dependency (it already depends on
/// `lb-host`). One id source for the whole node.
pub use lb_store::new_ulid;
pub use lb_supervisor::OsLauncher;
pub use outbox::{
    enqueue_held_outbox, enqueue_outbox, outbox_due, outbox_mark_delivered, outbox_mark_failed,
    outbox_status, relay_outbox, spawn_relay_reactors, OutboxError, OutboxStatus, RelayPass,
    Target,
};
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
    rules_run, rules_save, workspace_datasources, workspace_queries, AgentRuleModel, HostAiSeam,
    HostDataSeam, RuleModel, RulesError, RunResult, SavedRule,
};
pub use run_events::{
    pause_run, publish_run_event, resume_run, run_subject, stop_run, watch_run, RunEventSub,
    RunWatch, AGENT_CONTROL_TOOL,
};
pub use serve::{serve_ext, ToolServer};
pub use store_mutate::{
    authorize_store_mutate, call_store_mutate_tool, store_delete_run, store_write_run,
    StoreMutateError,
};
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
    authorize_tags, call_tags_tool, tags_add, tags_facet_values, tags_find, tags_of, tags_remove,
    Applied, Facet, Provenance, Source as TagSource, Tag, TagsError,
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
/// The **webhook** service — a first-class inbound-HTTP surface, keyed and mediated (webhooks
/// scope). A webhook is `API-key ⊕ ingest-producer ⊕ flow-source`, glued by the gateway route
/// `POST /hooks/{ws}/{id}`. `bearer` mode reuses the apikey credential verbatim; `signature` mode
/// uses an `lb-secrets` shared secret + constant-time HMAC over the raw body. Every accepted hit
/// becomes exactly one ingest `Sample` on `webhook:{ws}:{id}` — the webhook is a producer, not a
/// second store.
pub use webhook::{
    verify_signature, webhook_accept, webhook_create, webhook_get, webhook_list, webhook_resolve,
    webhook_revoke, webhook_rotate, AuthMode as WebhookAuthMode, CreateArgs as WebhookCreateArgs,
    CreatedWebhook, SignatureError as WebhookSignatureError, WebhookError, WebhookRecord,
    WebhookView, DEFAULT_HMAC_HEADER as WEBHOOK_DEFAULT_HMAC_HEADER,
    HMAC_SCHEME as WEBHOOK_HMAC_SCHEME, INGEST_CAP as WEBHOOK_INGEST_CAP,
    KIND_DISCRIM as WEBHOOK_KIND_DISCRIM, TABLE as WEBHOOK_TABLE,
    TOMBSTONE_STATUS as WEBHOOK_TOMBSTONE_STATUS,
};
pub use workspaces::{
    call_workspaces_tool, grant_default_core_skills, resolve_default_core_skills, workspace_create,
    workspace_delete, workspace_list, workspace_purge, workspace_rename, WorkspaceRecord,
    WorkspaceStatus, WorkspacesError, DEFAULT_CORE_SKILLS,
};
// The **reactor directory** — the durable workspace set the node's background reactors service
// (relocated from the retired workflow driver; rules-workflow-convergence scope). The register/
// deregister verbs are prefixed at the crate boundary so the public API names the concept (a bare
// `register` would be ambiguous next to `register_remote_extension`).
pub use directory::{
    deregister as deregister_workspace, enabled_workspaces, register as register_workspace,
    EntryStatus, WorkspaceEntry, DIRECTORY_NS,
};
