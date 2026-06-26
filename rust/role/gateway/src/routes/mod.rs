//! The gateway's HTTP routes — one verb per file (FILE-LAYOUT §4 transport routes), each thin
//! glue over a `lb_host` verb. The route names mirror the host verbs and the UI api client.

mod history;
mod post;
mod stream;

pub use history::get_history;
pub use post::post_message;
pub use stream::channel_stream;
