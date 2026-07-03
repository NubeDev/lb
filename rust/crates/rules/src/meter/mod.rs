//! The per-run budget meters — one file per meter (FILE-LAYOUT). [`AiMeter`] bounds AI spend (calls +
//! tokens); [`WriteMeter`] bounds motion-producing writes across the messaging planes (the DoS bound
//! for `outbox.enqueue`/`channel.post`/… — rules-messaging-scope). Reads are uncharged by both.

mod ai;
mod write;

pub use ai::AiMeter;
pub use write::WriteMeter;
