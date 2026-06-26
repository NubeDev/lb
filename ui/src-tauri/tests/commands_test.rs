//! The shell command layer, headless (no window): the same `channel_post` / `channel_history`
//! the desktop UI invokes, driven against a real in-process node. Proves the UI's IPC path
//! reaches the actual capability-checked channel service — "post a message, see it appear"
//! end to end on the Rust side, the mirror of the Vitest ChannelView test.
//!
//! Boots a Node → Zenoh peer, so the multi-thread flavor is required
//! (debugging/bus/zenoh-needs-multi-thread-runtime.md).

use lazybones_shell::{channel_history, channel_post, NodeHandle};
use lb_inbox::Item;

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn post_then_history_round_trips_through_the_command_layer() {
    let handle = NodeHandle::boot("shell-roundtrip").await.expect("boot");

    let stored = channel_post(
        &handle,
        "general",
        Item::new("m1", "general", "user:me", "hello from the shell", 1),
    )
    .await
    .expect("post via command");
    assert_eq!(stored.channel, "general");

    let view = channel_history(&handle, "general")
        .await
        .expect("history via command");
    assert_eq!(view.len(), 1);
    assert_eq!(view[0].body, "hello from the shell");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn history_is_empty_for_an_untouched_channel() {
    let handle = NodeHandle::boot("shell-empty").await.expect("boot");
    let view = channel_history(&handle, "general").await.expect("history");
    assert!(view.is_empty());
}
