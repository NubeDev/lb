//! The gateway's HTTP routes — one verb per file (FILE-LAYOUT §4 transport routes), each thin
//! glue over a `lb_host` verb. The route names mirror the host verbs and the UI api client. Every
//! guarded route authenticates the session token first (`session::authenticate`) — the workspace +
//! caps come from the token, never the request (the hard wall, §7).

mod admin_grants;
mod admin_members;
mod admin_teams;
mod admin_users;
mod admin_workspaces;
mod channel_registry;
mod ext;
mod history;
mod inbox;
mod login;
mod members;
mod outbox;
mod post;
mod stream;
mod workspace;

pub use admin_grants::{assign_grant, list_grants, list_roles, revoke_grant};
pub use admin_members::remove_team_member;
pub use admin_teams::{create_team, delete_team, list_teams, rename_team};
pub use admin_users::{create_user, delete_user, disable_user, enable_user, list_users};
pub use admin_workspaces::{archive_workspace, purge_workspace, rename_workspace};
pub use channel_registry::{create_channel, list_channels};
pub use ext::{
    disable_extension, enable_extension, list_extensions, publish_extension, uninstall_extension,
};
pub use history::get_history;
pub use inbox::{list_inbox, resolve_inbox};
pub use login::login;
pub use members::{add_team_member, list_team_members};
pub use outbox::get_outbox_status;
pub use post::post_message;
pub use stream::channel_stream;
pub use workspace::{create_workspace, list_workspaces};
