# Push target — session

- Date: 2026-07-11
- Scope: `docs/scope/inbox-outbox/push-target-scope.md`
- Status: done

## Goal

Push notifications as an outbox `Target`: per-member device registrations, a generic notification
payload, and provider adapters (WebPush first) behind one trait. The core never knows *why* a
notification is sent (rule 10); callers hand it an opaque title/body/deep-link and an audience.

## What changed

### `lb-host` crate — `notify/` module (5 files)
- **`device.rs`** — `Device` record (id, sub, platform, token, app_id, last_seen, disabled).
  Platform: `Fcm` / `Apns` / `Webpush`. Raw verbs: `device_write`, `device_get_raw`,
  `device_list_raw`, `device_list_all_raw`, `device_disable_raw`.
- **`verbs.rs`** — `device_register` (self-only, gated `mcp:device.register:call`),
  `device_list` (self-only), `device_remove` (self-only — ownership checked),
  `notify_send` (gated `mcp:notify.send:call`; enqueues a push effect via `lb_outbox::enqueue`).
- **`push_target.rs`** — `PushTarget` (impl `Target`): resolves audience → live devices, checks
  quiet-hours prefs (`push_muted` axis), calls `PushProvider` per device, auto-disables on
  `TokenGone`, reports per-device outcomes. `PushProvider` trait (the one sanctioned external,
  one named file). `RecordingPushProvider` (test fake).
- **`tool.rs`** — MCP dispatch.
- **`error.rs`** — `NotifyError` → `ToolError`.

### `lb-prefs` crate
- **`push_muted: Option<bool>`** axis on `Prefs` (the `insight_notifications` pattern — whole-fold
  nullable, read at delivery time, `None` = inherit/default ON).

### Wiring
- `"device."` / `"notify."` added to `HOST_NATIVE_PREFIXES` + dispatch branch.
- `mcp:device.register:call` in the viewer set (every member registers their own devices).
- `mcp:notify.send:call` in the author set (a member sends push).
- Device/notify verbs in the system catalog.

## Decisions & alternatives

1. **`notify.send` is its own verb** (the scope's recommendation — "it's where the audience/prefs
   policy lives"). *Rejected:* a thin alias over `outbox.enqueue{target:"push"}` — loses the
   named policy seam.
2. **WebPush first (v1).** The scope's recommendation — PWA, no store approvals. FCM/APNs are
   later adapters behind the same `PushProvider` trait. The real WebPush impl (VAPID + RFC 8291
   encryption) is a named follow-up — the trait + recording fake are the seam.
3. **Quiet hours live in prefs v1** (the scope's recommendation — `push_muted` axis). Retrofitting
   DND after users are annoyed is the wrong order.
4. **Self-only device management.** A member registers/lists/removes their own devices only — the
   `sub` is forced to `principal.sub()`, and `device_remove` checks ownership. An admin sees
   counts (via a future admin route), never tokens. Token privacy: tokens are PII-adjacent, never
   in logs.

## Tests

Real store, real outbox — the provider is the one sanctioned fake. 9 tests:

- `register_and_list_device` — happy path
- `register_is_idempotent_upsert` — re-register updates `last_seen`, no duplicates
- `remove_own_device` — self-only removal
- `denies_register_without_cap` — **mandatory capability deny**
- `denies_notify_send_without_cap` — **mandatory capability deny**
- `device_not_visible_from_other_workspace` — **mandatory workspace isolation**
- `denies_removing_another_members_device` — self-only enforcement
- `notify_send_enqueues_effect` — the outbox write
- `recording_provider_records_sends` — the provider trait

## Follow-ups

- Real WebPush provider impl (VAPID + RFC 8291 encryption) — the trait is the seam.
- FCM v1 + APNs (token-based) adapters.
- Wire `spawn_relay_reactors` with the `PushTarget` at boot.
- Per-device outcome recording on the outbox effect.
- `collapse_key` → provider dedupe mapping.
