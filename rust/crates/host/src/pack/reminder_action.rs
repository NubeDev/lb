//! Convert a pack manifest's `reminders:` block into the `lb_reminders::Action` `reminder.create`
//! takes — and **the one place a shape drift between the mirror and the real action would surface**
//! (`lb_packs::manifest_reminder` explains why the mirror exists).
//!
//! `action_kind` arrives as a `String` and every per-kind field as an `Option`. Neither an unknown
//! kind nor a missing required field can reach here: `pack.validate` errors the apply out first
//! (`lb_packs::validate_reminder`), so a typo is a lint the author sees rather than a reminder that
//! applies with an empty tool name. The fallbacks below therefore never fire in practice; they keep
//! a hypothetical un-linted path at a shape `reminder.create`'s own best-effort check REJECTS —
//! deliberately, so such a path fails loudly at create rather than persisting a reminder that can
//! never fire.

use lb_packs::PackReminder;
use lb_reminders::Action;

/// Convert the manifest's mirror into the real [`Action`]. Field-for-field by design.
pub(super) fn to_action(r: &PackReminder) -> Action {
    match r.action_kind.as_str() {
        "channel-post" => Action::ChannelPost {
            channel: r.channel.clone().unwrap_or_default(),
            body: r.body.clone().unwrap_or_default(),
        },
        "outbox" => Action::Outbox {
            target: r.target.clone().unwrap_or_default(),
            action: r.action.clone().unwrap_or_default(),
            payload: r.payload.clone().unwrap_or_default(),
        },
        // `mcp-tool` is the fallback arm, not a named one: `validate_reminder` has already rejected
        // every kind outside the closed set, so anything reaching here IS this kind. An empty tool
        // name is refused by `reminder.create` itself.
        _ => Action::McpTool {
            tool: r.tool.clone().unwrap_or_default(),
            args: r.args.clone().unwrap_or(serde_json::Value::Null),
        },
    }
}
