# Reminder firing denies for a token-only (dev-login) principal — re-resolve reads the durable grant store, not the token

Status: **open** (pre-existing; named follow-up — not introduced by the channel-rich-responses slice).

**Symptom (real-gateway control e2e):** over the real HTTP gateway, **every `reminder.fire` denies
(403 "denied")** — and, by the same path, the scheduled **reactor firing** never fires — for a
reminder created by a **dev-login** session, even though that session's token provably carries the
fired action's capability (e.g. `bus:chan/team:pub`, which `member_caps()` includes). The channel
`/reminders` "Run now" row control is therefore inert over the real gateway. A pure-HTTP repro:

```
POST /mcp/call  reminder.create (channel-post to "team")   -> ok, principalSub=user:ada  (dev-login)
POST /mcp/call  reminder.fire {id}                          -> 403 "denied"               ← BUG
```

## Root cause (traced, and it is PRE-EXISTING)

`reminder.fire` (and the reactor) re-check the fired action's own capability under the reminder's
**stored principal**, re-resolved **from the durable grant store at fire time**:

- `reminder/fire.rs` → `resolve_fire_principal` → `crate::authz::resolve_caps` →
  `lb_authz::resolve_caps` (`crates/authz/src/resolve.rs`) → `grant_list` — a read of the durable
  `grant` table (direct grants ∪ roles ∪ team-inherited).
- then `fire_channel_post` calls `authorize_channel(bus:chan/team:pub)`, which **denies because the
  re-resolved cap set is empty**.

This re-resolve is **deliberate** (`fire.rs` header: "a grant revoked after create turns the firing
into a logged deny … so a revoke takes effect at the next fire, not just the next token re-mint").
It assumes **caps live in the durable grant store**. That holds for a durably-granted member; it does
**not** hold for a **token-only principal**:

- `dev_claims` (`role/gateway/src/session/credentials.rs`) synthesizes `member_caps()` into the **JWT
  claims** and writes **nothing** to the grant store.
- `resolve_caps` is the *projection the token is minted FROM* — it reads the store, not the token. So
  for a dev-login user it returns only whatever role/team grants exist durably (here: none of
  `bus:chan/*:pub`), and the fire-time re-check finds the action cap absent → deny.

Why every prior test stayed green: the shipped `reminders_reactor_test.rs` / `reminder_fire_test.rs`
seed the action cap **durably** via `lb_authz::grant_assign` before firing, so the re-resolve sees it.
Only the **dev-login gateway path** (token-carried caps, nothing durable) exposes the gap — which is
the path the real-gateway control e2e drives.

This is **pre-existing** in the shipped reminder system (introduced with the reminder system itself,
commit `b78a0bd`, before the channel-rich-responses slice). The slice's run-now control and its
`ts`-default fix are correct; they merely surfaced this by driving a real dev-login fire over HTTP.

## Scope / impact

- Any reminder whose creator's caps are **token-only** (dev-login, and any principal whose caps aren't
  durably granted) **cannot fire** — neither run-now nor the scheduled reactor. A durably-granted
  member is unaffected.
- The channel `/reminders` **Run now** control (and the reactor for such users) is blocked end to end.
- **Unaffected:** `reminder.create`, `reminder.list` (read), `reminder.update` (pause), and
  `reminder.delete` all work over the real gateway — they run under the **token** principal directly
  (no fire-time re-resolve), so they see the token's caps.

## Fix (deferred — named follow-up, security-semantics-sensitive)

The fire-time re-resolve must account for token-carried caps without weakening the
revoke-takes-effect guarantee it exists to provide. Candidate directions, to be decided in a dedicated
slice (NOT this one):

- **Persist member caps durably on login/bootstrap** so `resolve_caps` sees them (aligns the token
  projection with its source; keeps revocation live) — likely the correct fix, since the token is
  meant to be a *cache* of durable grants.
- Or have fire re-resolve fold the stored principal's **captured token caps** as a floor (simpler, but
  a revoke then only applies after the token TTL — weakens the current guarantee).
- Or read through the same path `authz.resolve`/`grants.list` use if a divergence is found there.

Until then: reminders authored by durably-granted principals fire correctly; dev-login reminders do
not. The channel run-now control is correct and will work the instant the fire path is fixed — no
UI/contract change is needed.
