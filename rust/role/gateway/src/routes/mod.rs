//! The gateway's HTTP routes — one verb per file (FILE-LAYOUT §4 transport routes), each thin
//! glue over a `lb_host` verb. The route names mirror the host verbs and the UI api client. Every
//! guarded route authenticates the session token first (`session::authenticate`) — the workspace +
//! caps come from the token, never the request (the hard wall, §7).

mod admin_apikeys;
mod admin_grants;
mod admin_members;
mod admin_teams;
mod admin_users;
mod admin_workspaces;
mod assets;
mod bus;
mod chains;
mod channel_registry;
mod dashboard;
mod datasources;
mod dbview;
mod ext;
mod ext_ui;
mod flows;
mod history;
mod identity;
mod inbox;
mod ingest;
mod login;
mod mcp;
mod mcp_catalog;
mod members;
mod membership;
mod message;
mod outbox;
mod post;
mod prefs;
mod rules;
mod run_stream;
mod series_stream;
mod store_query;
pub(crate) mod stream;
mod system;
mod telemetry_stream;
mod workflow;
mod workspace;

pub use admin_apikeys::{create_apikey, get_apikey, list_apikeys, revoke_apikey, rotate_apikey};
pub use admin_grants::{
    assign_grant, define_role, delete_role, list_grants, list_roles, resolve_caps, revoke_grant,
    revoke_tokens_route,
};
pub use admin_members::remove_team_member;
pub use admin_teams::{create_team, delete_team, list_teams, rename_team};
pub use admin_users::{create_user, delete_user, disable_user, enable_user, list_users};
pub use admin_workspaces::{archive_workspace, purge_workspace, rename_workspace};
pub use assets::{
    get_doc, grant_skill, link_doc, list_docs, load_skill, put_doc, put_skill, share_doc,
};
pub use bus::{bus_stream, publish_message};
pub use chains::{delete_chain, get_chain, get_chain_run, list_chains, run_chain, save_chain};
pub use channel_registry::{create_channel, list_channels};
pub use dashboard::{
    delete_dashboard, get_dashboard, list_dashboards, save_dashboard, share_dashboard,
};
pub use datasources::{add_datasource, list_datasources, remove_datasource, test_datasource};
pub use dbview::{list_tables, read_graph, scan_table};
pub use ext::{
    disable_extension, enable_extension, list_extensions, publish_extension, uninstall_extension,
};
pub use ext_ui::serve_ext_ui;
pub use flows::{
    delete_flow, enable_flow, flow_node_state, flow_run_stream, get_flow, get_flow_node,
    get_flow_run, inject_flow, lifecycle_flow, list_flow_nodes, list_flow_runs, list_flows,
    patch_flow_run, run_flow, save_flow, update_flow_node,
};
pub use history::get_history;
pub use identity::{
    create_identity, get_identity, identity_workspaces as identity_workspaces_route,
    list_identities,
};
pub use inbox::{list_inbox, resolve_inbox};
pub use ingest::{find_series, latest_sample, list_series, read_samples, write_samples};
pub use login::login;
pub use mcp::mcp_call;
pub use mcp_catalog::mcp_catalog;
pub use members::{add_team_member, list_team_members};
pub use membership::{
    add_member_route as add_member, list_members_route as list_members,
    remove_member_route as remove_member,
};
pub use message::{delete_message, edit_message};
pub use outbox::get_outbox_status;
pub use post::post_message;
pub use prefs::{
    convert_unit, format_datetime, format_number, format_quantity, get_prefs, resolve_prefs,
    set_default_prefs, set_prefs,
};
pub use rules::{delete_rule, get_rule, list_rules, run_rule, save_rule};
pub use run_stream::run_stream;
pub use series_stream::series_stream;
pub use store_query::{read_schema, run_query};
pub use stream::channel_stream;
pub use system::{system_acp, system_overview, system_subsystem, system_tools, system_topology};
pub use telemetry_stream::telemetry_stream;
pub use workflow::{request_approval, resolve_approval as resolve_workflow_approval, start_job};
pub use workspace::{create_workspace, list_workspaces};
