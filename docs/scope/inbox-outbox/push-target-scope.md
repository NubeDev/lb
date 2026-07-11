# Inbox-outbox scope — push notifications as an outbox target (FCM/APNs/WebPush)

Status: scope (the ask). Promotes to `public/inbox-outbox/` once shipped.

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

- Is `notify.send` its own verb or a thin alias over a generic `outbox.enqueue{target:
  "push"}`? (Recommend the named verb — it's where the audience/prefs policy lives.)
- WebPush first (PWA, no store approvals) then FCM/APNs? (Recommend yes — v1 = WebPush.)
- Do quiet hours live in prefs v1 or ship later? (Recommend the prefs axis v1 — retrofitting
  DND after users are annoyed is the wrong order.)

## Related

`outbox-scope.md` · `../secrets/secrets-scope.md` · `../prefs/` ·
`../auth-caps/invites-scope.md` (the email sibling) · first consumer: `cc-app`
`docs/scope/care/daily-feed-scope.md`.
