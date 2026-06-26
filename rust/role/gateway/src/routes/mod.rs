//! The gateway's HTTP routes — one verb per file (FILE-LAYOUT §4 transport routes), each thin
//! glue over a `lb_host` verb. The route names mirror the host verbs and the UI api client. Every
//! guarded route authenticates the session token first (`session::authenticate`) — the workspace +
//! caps come from the token, never the request (the hard wall, §7).

mod channel_registry;
mod history;
mod inbox;
mod login;
mod members;
mod outbox;
mod post;
mod stream;
mod workspace;

pub use channel_registry::{create_channel, list_channels};
pub use history::get_history;
pub use inbox::{list_inbox, resolve_inbox};
pub use login::login;
pub use members::{add_team_member, list_team_members};
pub use outbox::get_outbox_status;
pub use post::post_message;
pub use stream::channel_stream;
pub use workspace::{create_workspace, list_workspaces};
