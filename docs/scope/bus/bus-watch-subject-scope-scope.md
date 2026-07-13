# Bus scope — subject-scoped `bus.watch` grants + revoke-terminates-stream

Status: scope (the ask). Promotes to `doc-site/content/public/bus/bus.md` once shipped.

`bus.watch` today authorizes a subscribe against only the **workspace-wide** cap
`mcp:bus.watch:call` — the subject string never enters the cap check, so within a
workspace any holder of that cap can watch **any** `ext/*` subject, including another
entity's per-subject feed. And the gate runs **once**, before the stream opens: revoking
a grant blocks the *next* subscribe but does not close a currently-open SSE stream. This
scope closes both gaps additively, converging the generic `bus.watch` path with the
channel service's existing `bus:chan/*:sub` subject-cap grammar onto **one** subject-scoped
cap model. Downstream: `NubeDev/cc-app` `care.feed.watch` (per-child feed on
`care.feed.<child>`) upgrades from reach-check-at-subscribe to platform stream isolation.

Tracks GitHub issue #49.

## Goals
- A subject-scoped bus grant `bus:<subject>:watch` that the `bus.watch` authorize path
  honors, so per-subject subscribe authz is platform-enforced (Gap 1).
- `grants.revoke` of such a grant closes the holder's matching **open** SSE streams within
  a bounded tick (Gap 2).
- Fully backward-compatible: **no** subject-scoped grant present for a caller ⇒ exactly
  today's behavior (coarse `mcp:bus.watch:call` gate only).
- One subject-scoped cap model shared by the channel `bus:chan/*:sub` grammar and the
  generic `bus.watch` path (both `Surface::Bus`, wildcard-capable resource).

## Non-goals
- No change to `bus.publish` authz (publish stays coarse-gated + subject-walled; a
  future `bus:<subject>:pub` mirror is trivial once this lands but is out of scope here).
- No WIT/ABI/SDK change — the fix is grant-grammar + authorize-path + stream-lifecycle,
  all host-side. Extensions mint the grant through the existing generic `grants.assign`
  MCP verb (the cap string is opaque data — rule 10).
- No new store table or migration. Grants ride the existing `grant` table.
- No per-message authz (motion is fire-and-forget, rule 3) — the gate is at subscribe +
  a periodic re-check, not per frame.

## Intent / approach

**Gap 1 — subject-scoped bus grants (the convergence).** The channel service already
proves the pattern: a `Surface::Bus` cap whose *resource is the thing* (`chan/{cid}`),
checked via `lb_caps::check` with `/`- and `.`-segment wildcards (`bus:chan/*:sub`). We
extend the same grammar to the generic subject: the scoped cap is `bus:<subject>:watch`
(new `Action::Watch`, additive to the caps grammar). The `bus.watch` authorize path keeps
`mcp:bus.watch:call` as the **coarse gate** (unchanged), then adds a *conditional*
subject gate:

- Resolve the caller's **live** bus-watch grants from the store (fresh, not the token —
  this is what makes revoke matter and is the same freshness `check_scoped` relies on).
- If the caller holds **no** `bus:*:watch` grant ⇒ **allow** (back-compat: today's
  behavior for every existing caller and every unscoped subject).
- If the caller holds **at least one** `bus:*:watch` grant ⇒ **require** one that matches
  this subject (`Request{ws, Surface::Bus, resource=subject, Action::Watch}` through the
  same `matches` grammar, so `bus:care.feed.*:watch` and `bus:care.feed.leo:watch` both
  authorize `care.feed.leo`). No match ⇒ opaque `Denied`.

This "present ⇒ required, absent ⇒ open" shape is the same additive-narrowing idiom the
entity-scoped `{table,ids}` grants use (`Scope::All` default = today), applied to the
subject-cap grammar instead of record rows — because a bus subject is a *string*, not a
`{table,id}` pair, so the channel subject-cap idiom is the right prior art, not the
`Scope::Ids` record selector. Rejected alternative: reusing `Scope::Ids{table:"bus", ids}`
— it can't express the `/`-segmented wildcard subject a per-entity feed needs
(`care.feed.*`), and it would overload a record-row selector with subject-string meaning.

**Gap 2 — revoke-terminates-stream.** The subscribe gate is one-shot. Add a bounded
**re-check tick** to the open-stream driver: on an interval, re-run the same subject
authorize; on failure, close that one stream (drop the subscription, end the SSE frames
for that subject). This holds for both the multiplexed hub (`hub.rs` driver task) and the
dedicated `GET /bus/{subject}/stream` route. Latency is bounded by the tick (default a
few seconds — a "bounded tick" per the ask), not instantaneous, which is the same
freshness posture the rest of authz has (revoke → deny on next check). Rejected
alternative: a push-on-revoke signal bus from `grants.revoke` into the hub — more moving
parts, needs a workspace-scoped fan-out channel and revoke-site coupling, for no better
guarantee than a short tick. A tick is symmetric-node-safe (no cross-node signal) and
local to the stream that must close.

## How it fits the core
- **Tenancy / isolation:** Gate 1 (workspace wall) is unchanged and still first. The new
  subject gate is *within* the workspace — it narrows, never widens. A cross-workspace
  subject is still refused by the wall before any grant is read.
- **Capabilities:** coarse `mcp:bus.watch:call` (unchanged) + the new conditional
  `bus:<subject>:watch` subject cap. Deny path: opaque `BusError::Denied` → `403` before
  the stream body (Gap 1) / stream close (Gap 2). Deny-test mandatory.
- **Placement:** either (symmetric). The re-check tick is node-local; no `if cloud`.
- **MCP surface:** no new verb. The grant is minted/revoked through the **existing**
  generic `grants.assign` / `grants.revoke` MCP verbs (the cap string is opaque data). The
  `bus.watch` stream stays the `GET /bus/{subject}/stream` + mux `bus:` subject.
- **Data (SurrealDB):** the existing `grant` table only — a `Grant{subject, cap:
  "bus:<subject>:watch", scope: All}` row. No new table, no migration (old rows unaffected).
- **Bus (Zenoh):** unchanged subjects; the subscription lifecycle gains a re-check +
  close. Motion stays fire-and-forget (rule 3).
- **Sync / authority:** grants tombstone-on-revoke as today (§6.8) — replays idempotently.
  The re-check reads the local store, so a synced revoke closes the stream on the next tick
  wherever the stream lives.

## Example flow (cc-app `care.feed.watch`)
1. cc-app's care extension, on linking guardian `ada` to child `leo`, calls
   `grants.assign(subject=user:ada, cap="bus:care.feed.leo:watch")` (generic MCP verb).
2. `ada` opens `GET /bus/stream?subject=care.feed.leo` (or the mux `bus:care.feed.leo`).
   Coarse gate `mcp:bus.watch:call` passes; the subject gate finds a `bus:*:watch` grant
   exists for `ada` and one matches `care.feed.leo` ⇒ allow. Stream opens.
3. `ada` opens `GET /bus/stream?subject=care.feed.mia` (another child). Coarse gate passes,
   but no `bus:*:watch` grant matches `care.feed.mia` ⇒ **`403`** (Gap 1 closed).
4. A caller with `mcp:bus.watch:call` and **no** `bus:*:watch` grant at all watches any
   `ext/*` subject exactly as today (back-compat).
5. cc-app unlinks `ada` from `leo` → `grants.revoke(user:ada, "bus:care.feed.leo:watch")`.
   Within one re-check tick, `ada`'s open `care.feed.leo` stream **closes** (Gap 2 closed).

## Testing plan
Mandatory categories (`scope/testing/testing-scope.md`): **capability-deny** and
**workspace-isolation**. Real infra — `mem://` store, real bus, real gateway; grants seeded
through the real write path (`grants.assign` / `grant_assign_scoped`), no mocks.

- **Gap 1 deny:** holder of `mcp:bus.watch:call` + `bus:care.feed.leo:watch` is DENIED
  `care.feed.mia`, ALLOWED `care.feed.leo`.
- **Back-compat:** holder of `mcp:bus.watch:call` with NO `bus:*:watch` grant watches any
  subject (unchanged) — the load-bearing assertion.
- **Wildcard:** `bus:care.feed.*:watch` authorizes `care.feed.leo` but not `other.feed.x`.
- **Workspace isolation:** a `bus:care.feed.leo:watch` grant in ws A does not authorize
  the same subject for a principal in ws B (wall first).
- **Gap 2 revoke-closes-stream:** open a stream to a scoped subject, `grants.revoke` the
  grant, assert the stream closes within a bounded number of ticks (real SSE via the hub).
- **Fresh grant read:** an assign *after* login (not in the token) authorizes on next
  subscribe — proves the store-read freshness.

## Open questions (resolved on ship)
- **Re-check tick interval.** RESOLVED: `RECHECK_INTERVAL = 3s` (a constant in
  `recheck.rs`; `WatchRecheck::with_interval` is the test seam that drives it in ms).
  Bounded latency is acceptable per the ask; not config-surfaced in v1 (no need yet).
- **Scoped-mode stickiness (found during build).** RESOLVED: a naive "require a match
  only while the caller holds any `bus:*:watch` grant" rule had an isolation hole —
  revoking the caller's *last* grant dropped them to open mode and re-opened the subject.
  Fixed by anchoring the stream re-check to the *grant itself* (`WatchMode::Scoped` +
  `still_scoped_authorized`): a scoped stream requires its matching grant to persist, so a
  last-grant revoke **denies**, never re-opens. Regression test
  `revoking_the_only_grant_denies_the_subject_it_does_not_reopen`.
- **Publish mirror (`bus:<subject>:pub`).** Deferred (non-goal). The `Action::Watch`
  grammar makes it a one-line follow-up if a per-subject publish gate is ever needed.
