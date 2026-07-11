//! The gateway's HTTP routes — one verb per file (FILE-LAYOUT §4 transport routes), each thin
//! glue over a `lb_host` verb. The route names mirror the host verbs and the UI api client. Every
//! guarded route authenticates the session token first (`session::authenticate`) — the workspace +
//! caps come from the token, never the request (the hard wall, §7).

mod admin_apikeys;
mod admin_grants;
mod admin_members;
mod admin_teams;
mod admin_users;
mod admin_webhooks;
mod admin_workspaces;
mod agent_config;
mod agent_defs;
mod agent_invoke;
mod assets;
mod assets_bin;
mod brand;
mod bus;
mod catalog;
mod channel_registry;
mod dashboard;
mod datasources;
mod dbview;
mod events;
mod ext;
mod ext_ui;
mod flows;
mod history;
mod identity;
mod inbox;
mod ingest;
mod insight;
mod invite_accept;
mod layout;
mod login;
mod mcp;
mod mcp_catalog;
mod members;
mod membership;
mod message;
mod native;
mod nav;
mod outbox;
mod panel;
mod post;
mod prefs;
mod report;
mod rules;
mod run_control;
mod run_stream;
mod run_token_refresh;
mod series_stream;
mod store_query;
pub(crate) mod stream;
mod surface;
mod system;
mod telemetry_stream;
mod webhook;
mod workspace;

pub use admin_apikeys::{create_apikey, get_apikey, list_apikeys, revoke_apikey, rotate_apikey};
pub use admin_grants::{
    assign_grant, define_role, delete_role, list_grants, list_roles, resolve_caps, revoke_grant,
    revoke_tokens_route,
};
pub use admin_members::remove_team_member;
pub use admin_teams::{create_team, delete_team, list_teams, rename_team};
pub use admin_users::{create_user, delete_user, disable_user, enable_user, list_users};
pub use admin_webhooks::{
    create_webhook, get_webhook, list_webhooks, revoke_webhook, rotate_webhook,
};
pub use admin_workspaces::{archive_workspace, purge_workspace, rename_workspace};
pub use agent_config::{
    get_agent_config as get_agent_config_route, set_agent_config as set_agent_config_route,
};
pub use agent_defs::{
    create_def, delete_def, get_def, list_defs, test_active_def, test_def, update_def,
};
pub use agent_invoke::agent_invoke;
pub use assets::{
    get_doc, grant_skill, link_doc, list_docs, load_skill, put_doc, put_skill, share_doc,
};
pub use assets_bin::{get_asset_bin, put_asset as put_asset_bin};
pub use brand::{delete_brand, get_brand, list_brands, save_brand};
pub use bus::{bus_stream, publish_message};
pub use catalog::{get_catalog, render_message as render_catalog_message, set_catalog};
pub use channel_registry::{create_channel, list_channels};
pub use dashboard::{
    delete_dashboard, get_dashboard, list_dashboards, pin_dashboards, save_dashboard,
    share_dashboard,
};
pub use datasources::{add_datasource, list_datasources, remove_datasource, test_datasource};
pub use dbview::{list_tables, read_graph, scan_table};
pub use events::{events_stream, events_subscribe, events_unsubscribe};
pub use ext::{
    disable_extension, enable_extension, list_extensions, publish_extension, reset_extension,
    uninstall_extension,
};
pub use ext_ui::serve_ext_ui;
pub use flows::{
    delete_flow, enable_flow, flow_debug_stream, flow_node_state, flow_run_stream, get_flow,
    get_flow_node, get_flow_run, inject_flow, lifecycle_flow, list_flow_nodes, list_flow_runs,
    list_flows, patch_flow_run, run_flow, save_flow, update_flow_node,
};
pub use history::get_history;
pub use identity::{
    create_identity, get_identity, identity_workspaces as identity_workspaces_route,
    list_identities,
};
pub use inbox::{list_inbox, resolve_inbox};
pub use ingest::{find_series, latest_sample, list_series, read_samples, write_samples};
pub use insight::{
    ack_insight, delete_insight, delete_occurrence, get_insight, insight_events, list_insights,
    list_occurrences, resolve_insight,
};
pub use invite_accept::accept_invite;
pub use layout::{get_layout, set_layout};
pub use login::login;
pub use mcp::mcp_call;
pub use mcp_catalog::mcp_catalog;
pub use members::{add_team_member, list_team_members};
pub use membership::{
    add_member_route as add_member, list_members_route as list_members,
    remove_member_route as remove_member,
};
pub use message::{delete_message, edit_message};
pub use native::native_call;
pub use nav::{
    delete_nav, get_nav, get_nav_hidden, get_nav_pref, list_navs, list_shares_nav, resolve_nav,
    save_nav, set_default_nav, set_nav_hidden, set_nav_pref, share_nav, unshare_nav,
};
pub use outbox::get_outbox_status;
pub use panel::{delete_panel, get_panel, list_panels, panel_usage, save_panel, share_panel};
pub use post::post_message;
pub use prefs::{
    convert_unit, format_datetime, format_number, format_quantity, get_prefs, resolve_prefs,
    set_default_prefs, set_prefs,
};
pub use report::{
    delete_report, export_report, get_report, list_reports, save_report, share_report,
};
pub use rules::{delete_rule, get_rule, list_rules, run_rule, save_rule};
pub use run_control::run_control;
pub use run_stream::run_stream;
pub use run_token_refresh::refresh_run_token;
pub use series_stream::series_stream;
pub use store_query::{read_schema, run_query};
pub use stream::channel_stream;
pub use surface::surface_reach;
pub use system::{system_acp, system_overview, system_subsystem, system_tools, system_topology};
pub use telemetry_stream::telemetry_stream;
pub use webhook::post_webhook;
pub use workspace::{create_workspace, list_workspaces};
