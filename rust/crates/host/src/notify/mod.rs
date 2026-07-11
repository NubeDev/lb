//! The **notify** service — push notifications as an outbox `Target` (push-target scope). Device
//! registrations, a generic notification payload, and provider adapters behind one trait
//! (`PushProvider`). The core never knows *why* a notification is sent (rule 10): callers hand it
//! an opaque title/body/deep-link and an audience of member `sub`s.
//!
//! Verbs: `device.register` / `device.list` / `device.remove` / `notify.send`. The push
//! `Target` adapter fans out to each recipient's live devices at delivery time.

mod delivered;
mod device;
mod error;
mod push_target;
mod tool;
mod verbs;

pub use device::{Device, Platform};
pub use error::NotifyError;
pub use push_target::{
    LoggingPushProvider, PushError, PushPayload, PushProvider, PushTarget, RecordedPush,
    RecordingPushProvider, PUSH_TARGET,
};
pub use tool::call_notify_tool;
pub use verbs::{device_list, device_register, device_remove, notify_send, NotifyCatalogRef};
