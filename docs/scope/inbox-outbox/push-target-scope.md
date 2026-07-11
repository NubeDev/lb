# Inbox-outbox scope — push notifications as an outbox target (FCM/APNs/WebPush)

Status: scope (the ask) — v1 substrate shipped + review-fixed 2026-07-11 (see "Shipped (v1)"
below); WebPush(VAPID) adapter + admin device-count surface still open. Promotes to
`public/inbox-outbox/` once those close.

> Read with: `outbox-scope.md` (the must-deliver substrate + the generic `Target` trait this
> plugs into), `../auth-caps/api-keys-scope.md` (hashed-credential pattern),
> `../secrets/secrets-scope.md` (provider keys), README §3 rules 3/10.

A mobile-first product is dead without push — "Leo was checked in" must reach a locked
phone, not an open SSE tab. The platform has the right substrate (the transactional outbox:
durable, retried, at-least-once) and no target that ends at a phone. We want **push as one
more outbox `Target`**: per-member **device registrations**, a generic notification
payload, and provider adapters (FCM, APNs, WebPush for the PWA) behind one trait — the
same shape email delivery takes in `auth-caps/invites-scope.md`. The core never knows
*why* a notification is sent (rule 10): callers hand it an opaque title/body/deep-link and
an audience of member `sub`s.

## Goals

- **`device` record** (per workspace member): platform (`fcm|apns|webpush`), push token/
  subscription, app id, `last_seen`, disabled flag. Verbs: `device.register` (upsert by
  token, self-service — a member registers their own devices only), `device.list` (own;
  admin sees counts not tokens), `device.remove`.
- **`push` outbox target:** effect = `{to: [sub…], title, body, deep_link?, collapse_key?,
  priority}`. The target fans out to each recipient's live devices, maps provider errors
  (token-gone → auto-remove the device; throttled → backoff via the outbox's retry),
  reports per-device outcomes to the outbox record.
- **Provider adapters behind one trait** (`PushProvider`, one file each): FCM v1, APNs
  (token-based), WebPush (VAPID). Credentials via `secrets/` mediation. These are true
  externals — the sanctioned fake, one trait, one named test impl that records sends.
- **Preference gate:** a per-member mute/quiet-hours check applied at fan-out (an axis on
  `lb_prefs::Prefs`, whole-fold as usual), so "notify the guardians" callers don't each
  reimplement do-not-disturb.

## Non-goals

- **Not an in-app notification center** — that's inbox territory (a caller may write an
  inbox item *and* enqueue a push; this scope is only the phone-delivery leg).
- **Not templating/localization** — callers send final strings v1.
- **Not analytics** (open rates etc.) — the outbox's delivery record is the only ledger.

## Intent / approach

Everything hard about push (durability, retries, backoff, at-least-once) is already the
outbox's job — the entire design is "don't build a notification service, build a `Target`".
Device tokens are the only new state; auto-eviction on provider `410`/`UNREGISTERED` is the
one behavior that keeps the table healthy.

**Rejected — a standalone notifier service/reactor with its own queue:** duplicates the
outbox's semantics worse (its whole reason to exist), violates symmetric nodes with a
special role. **Rejected — SSE-only + PWA background hacks:** doesn't survive a locked
phone; WebPush exists for exactly this.

## How it fits the core

- **Tenancy / isolation:** devices are `(ws, sub)`-scoped rows; an effect's audience is
  resolved to members **of that workspace** only — a `sub` outside it is dropped (tested).
- **Capabilities:** `mcp:device.register/remove:call` (member-level, self-only —
  deny-tested for another member's device), `mcp:notify.send:call` for enqueueing (the
  caller-facing verb wrapping the outbox write). Deny = 403 before enqueue.
- **Placement:** either role; typically the hub holds provider credentials, so the target
  runs where secrets resolve — by config, not a code branch.
- **MCP surface (§6.1):** CRUD on devices + `notify.send` (fire the effect). Live feed and
  batch N/A (fan-out lives inside the target; the outbox is the batch machinery).
- **State vs motion:** device rows = state; the notification = a must-deliver outbox
  effect, **never** raw pub/sub (a dropped "your child had an incident" is not acceptable).
- **Secrets:** FCM service-account / APNs key / VAPID pair via `secrets/`, names-only on
  records.
- **SDK/WIT impact:** none — extensions call `notify.send` as a normal granted MCP tool.

## Example flow

1. Sam's phone opens the PWA → `device.register{platform: webpush, subscription}` (idempotent).
2. Staff logs Leo's check-in; the care extension calls `notify.send{to: [sam, ana], title:
   "Leo checked in", deep_link: "care/feed/leo"}` — gated by its granted cap.
3. Outbox row written in the same breath as the domain write (transactional); the push
   target resolves 3 live devices (Sam×2, Ana×1), respects Ana's quiet hours, sends 2.
4. Sam's old tablet token returns `UNREGISTERED` → device auto-removed.
5. Provider outage → outbox retries with backoff; nothing is lost, nothing double-writes
   the domain.

## Testing plan

Mandatory: **capability-deny** (`notify.send` without the cap → 403; registering a device
for another `sub` → 403), **workspace isolation** (audience `sub` not a member → silently
excluded, cross-ws device never resolved). Plus: token-gone eviction, retry/backoff on
provider error (outbox semantics reused, asserted here), quiet-hours suppression, upsert
idempotency, per-device outcome recording. Providers exercised via the one recording fake;
everything else (store, outbox, caps) real.

## Risks & hard problems

- **At-least-once vs annoyance:** a retried effect must not double-buzz — `collapse_key`
  maps to provider dedupe; the target marks per-device success so retries only re-send
  failures.
- **Credential ceremony** (APNs keys, FCM service accounts) — a docs/skill problem as much
  as code; ship the runbook with the slice.
- **Token privacy:** push tokens are PII-adjacent — never in logs, admin sees counts.

## Open questions

- ✅ `notify.send` is its own named verb (the audience/prefs policy lives here). *Rejected:* a
  thin alias over `outbox.enqueue{target:"push"}` — loses the named policy seam.
- ✅ WebPush first (PWA, no store approvals), then FCM/APNs. v1 = WebPush.
- ✅ Quiet hours live in prefs v1 (`push_muted` axis — the `insight_notifications` pattern).
  Retrofitting DND after users are annoyed is the wrong order.

## Shipped (v1) + review-fix amendments (2026-07-11)

What is actually in-tree after the peer-review pass, and what is honestly deferred:

- **Wiring contract — SUPERSEDED (2026-07-11, release scope gap 1):** the `node` boot ritual now
  registers BOTH targets itself: `node/src/reactors.rs::spawn` builds a generic `RouterTarget`
  (dispatch on the effect's opaque `target` string) with `EmailTarget` + `PushTarget` and spawns
  `spawn_relay_reactors` (2s tick), gated by `BootConfig.reactors`. Providers come through the
  additive `BootConfig.outbox_providers` seam (`Option<Arc<dyn EmailProvider/PushProvider>>`);
  unset ⇒ logging no-op providers (log + ack — boot never crashes, effects never strand). Core
  still names no provider (rule 10). Boot-level proof: `node/tests/relay_boot_test.rs`.
- **i18n (2026-07-11, release scope gap c):** `notify.send` additionally accepts
  `title_key`/`body_key`/`args` (a catalog reference); `PushTarget` renders **per-recipient** in
  each recipient's `language` pref at deliver time. Literal title/body remain the compat path and
  are never translated. Proof: `host/tests/push_i18n_test.rs` (one notify → en+es payloads).
- **Workspace comes from the payload, never guessed:** `notify.send` embeds `"workspace": ws`
  in the effect payload at enqueue (it authorized against that ws); `deliver()` fails the
  effect if it is absent. The audience is membership-checked in that ws — a non-member `sub`
  is silently excluded (tested). This replaced a review-found hardcoded workspace
  (`debugging/inbox-outbox/push-target-hardcoded-workspace.md`).
- **At-least-once, per-device:** `notify/delivered.rs` keeps a ws-scoped delivered marker per
  `(idempotency_key, device_id)`; an outbox retry re-sends **only** the failures. `collapse_key`
  is NOT the dedup key — it is forwarded to the provider (WebPush `Topic` / FCM `collapse_key`)
  for provider-side collapse of stacked notifications; distinct effects sharing a collapse key
  are each still delivered once.
- **Effect ids are ULIDs** (`notify:{ulid}`) — the earlier `notify:{now}:{first_recipient}`
  collided within one second and the outbox idempotency dedup silently swallowed the second
  notification.
- **WebPush (VAPID) adapter: DEFERRED.** v1 shipped the `PushProvider` trait, the `Target`
  adapter, and the one sanctioned recording fake — no HTTP provider yet. Why: the adapter needs
  the VAPID keypair via `secrets/` mediation plus the credential runbook (Risks: "credential
  ceremony"), and no in-repo consumer exercises a real endpoint yet; shipping the trait-shaped
  seam first keeps the product host unblocked (it can wire any impl today). **Runbook item:**
  when the WebPush adapter lands, ship with it the VAPID key generation/rotation runbook and
  the secrets names it resolves.
- **`device.remove` disables, it does not delete** — rows accrete with no pruning; and the
  admin device-**count** surface (scope Goals: "admin sees counts not tokens") is not built.
  Both deferred to the next slice.

## Related

`outbox-scope.md` · `../secrets/secrets-scope.md` · `../prefs/` ·
`../auth-caps/invites-scope.md` (the email sibling) · first consumer: `cc-app`
`docs/scope/care/daily-feed-scope.md`.
