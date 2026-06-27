//! The gateway's HTTP routes — one verb per file (FILE-LAYOUT §4 transport routes), each thin
//! glue over a `lb_host` verb. The route names mirror the host verbs and the UI api client. Every
//! guarded route authenticates the session token first (`session::authenticate`) — the workspace +
//! caps come from the token, never the request (the hard wall, §7).

mod admin_grants;
mod admin_members;
mod admin_teams;
mod admin_users;
mod admin_workspaces;
mod assets;
mod channel_registry;
mod dashboard;
mod dbview;
mod ext;
mod ext_ui;
mod history;
mod inbox;
mod ingest;
mod login;
mod mcp;
mod members;
mod outbox;
mod post;
mod series_stream;
pub(crate) mod stream;
mod system;
mod workflow;
mod workspace;

pub use admin_grants::{assign_grant, define_role, list_grants, list_roles, revoke_grant};
pub use admin_members::remove_team_member;
pub use admin_teams::{create_team, delete_team, list_teams, rename_team};
pub use admin_users::{create_user, delete_user, disable_user, enable_user, list_users};
pub use admin_workspaces::{archive_workspace, purge_workspace, rename_workspace};
pub use assets::{
    get_doc, grant_skill, link_doc, list_docs, load_skill, put_doc, put_skill, share_doc,
};
pub use channel_registry::{create_channel, list_channels};
pub use dashboard::{
    delete_dashboard, get_dashboard, list_dashboards, save_dashboard, share_dashboard,
};
pub use dbview::{list_tables, read_graph, scan_table};
pub use ext::{
    disable_extension, enable_extension, list_extensions, publish_extension, uninstall_extension,
};
pub use ext_ui::serve_ext_ui;
pub use history::get_history;
pub use inbox::{list_inbox, resolve_inbox};
pub use ingest::{find_series, latest_sample, list_series, read_samples, write_samples};
pub use login::login;
pub use mcp::mcp_call;
pub use members::{add_team_member, list_team_members};
pub use outbox::get_outbox_status;
pub use post::post_message;
pub use series_stream::series_stream;
pub use stream::channel_stream;
pub use system::{system_overview, system_subsystem, system_topology};
pub use workflow::{request_approval, resolve_approval as resolve_workflow_approval, start_job};
pub use workspace::{create_workspace, list_workspaces};
