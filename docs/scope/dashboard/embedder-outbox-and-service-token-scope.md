# Embedder seams: an outbox target registry, a service-session mint, and the reminder tick

Status: built (2026-07-29). Driven by `NubeIO/rubix-ai`
`docs/scope/frontend/reports/report-server-pdf-schedule-scope.md`, which named the service token as
"the one potential lb ask" and required the reminder path to actually fire.

One paragraph: a product host embedding lb through `boot_full` could not do three things it plainly
needs to. It could not register an outbox delivery adapter of its own — `BootConfig.outbox_providers`
was two hardcoded *slots* (`email`, `push`), so any other kind of effect retried four times and
dead-lettered with the reason discarded. It could not authenticate a **non-interactive worker**: the
only ways to get a bearer were an interactive password login (yielding a 12-hour token) or
hand-building `Claims`, which re-implements the cap fold and drifts. And its **reminders never
fired**, because `react_to_reminders` was spawned by nothing. Each of these failed *silently*, which
is what makes them one scope rather than three tickets.

## Goals

- **A generic outbox target registry.** `BootConfig.outbox_providers.targets` — `(name, Arc<dyn
  DynTarget>)` pairs folded into the boot `RouterTarget` **after** the built-ins, so an embedder can
  add a target *or replace one*. The core keeps routing on an opaque string and names nothing.
- **A short-lived, cap-scoped service session.** `mint_full_session_with_ttl` (the shipped
  `mint_full_session` delegating to it at the 12-hour constant) and the ergonomic
  `RunningNode::mint_service_session`. It **grants nothing**: caps are the named principal's live
  durable grants, so a revoked grant takes effect at the next mint and a worker sees exactly what
  that principal sees.
- **Spawn the reminder reactor.** `spawn_reminder_reactors` + its boot wiring, so an authored cron
  schedule fires.
- **A nameable seam.** `lb-node` re-exports `Principal`, `Store`, `Target`/`DynTarget`/`OutboxEffect`,
  `enqueue_outbox` and the asset verbs, so a host with only the `lb-node` dep can spell the types
  these seams require — the `SigningKey`/`Node`/`BrowserSessionConfig` precedent, for the same reason.

## Non-goals

- **No SMTP, no transport.** The email target's provider seam is unchanged.
- **No new authorization.** The mint performs no check of its own: holding the node's signing key IS
  the decision to mint, exactly as it is for every other issuance path.
- **No API-key rework.** The embedded node's pepper is still per-process (so an embedded node's API
  keys still die on restart). Named here because it was found while investigating the mint; it is a
  separate fix.

## Intent / approach

`RouterTarget` stores `Arc<dyn DynTarget>` instead of `Box`, gaining `route_dyn` for a caller that
cannot name the concrete type — which is the whole point of a config-supplied target.

`mint_full_session_with_ttl` is the existing function with the TTL lifted to a parameter; nothing
about issuance changed, which is deliberate — a second, subtly different mint path is exactly the
drift this avoids. `RunningNode::mint_service_session` returns `None` on a headless node: with no
gateway there is nothing for a token to authenticate against, and handing back a credential with no
door would be worse than refusing.

`spawn_reminder_reactors` copies the shape of `spawn_approval_reactors` verbatim. Cadence is 10s,
and that number is load-bearing: cron resolves to the minute and `advance()` does not backfill, so a
missed slot is *skipped*, not deferred. It feeds `react_to_reminders` a **seconds** clock, not the
millis several sibling reactors use — the reminder plane is a logical second clock, and millis would
put every `next_attempt_ts` ~55,000 years in the past and fire everything at once.

**Rejected alternatives.** (i) *Let the embedder call `spawn_relay_reactors` itself with its own
`RouterTarget`* — requires a direct `lb-host` dep pinned in lockstep, and duplicates the built-in
adapter wiring, which then drifts. (ii) *A `Principal → token` function* — `Principal::routed` caps
are unsigned in-process co-trust; minting from one would launder unverified authority into a bearer.
(iii) *Reuse `mint_run_token`* — it is `run_id`-scoped (so `verify_token` demands a live job record)
and lives behind an off-by-default feature. (iv) *Widen `OutboxProviders` with more named slots* —
that is the bug, one size larger.

## How it fits

- **Capabilities & the deny path.** Unchanged everywhere. A registered target delivers effects the
  enqueuer was already authorized to stage; the mint carries only resolved grants; the reminder tick
  fires under the reminder's stored principal with caps re-resolved live, so a revoked grant is a
  logged deny, never an escalation.
- **Rule 10 / rule 1.** Nothing added names a consumer or branches on a role. Whether a node runs the
  reminder tick is `BootConfig::reactors`, i.e. config.

## Testing plan

`node/tests/embedder_seams_test.rs`, against a real `boot_full` with reactors on: an
embedder-registered target receives its effects through the **boot-spawned** relay (nothing calls
`relay_outbox`); a due reminder fires from the **boot-spawned** tick (nothing calls
`react_to_reminders`); a minted session verifies now, is expired 300s later (proving it is not the
12h session), and carries no cap its principal lacks; a headless node mints nothing.

## Related

- Sibling released together: `./dashboard-kind-scope.md`.
- The precedent for re-exporting a role type for embedders: `node/src/lib.rs`'s
  `BrowserSessionConfig` block.
- Consumer: `NubeIO/rubix-ai` `src/report/target.rs`.
