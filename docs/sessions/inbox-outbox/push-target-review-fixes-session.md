# Session — push-target peer-review fixes (2026-07-11)

Scope: `../../scope/inbox-outbox/push-target-scope.md` (see its new "Shipped (v1) + review-fix
amendments" section). Branch `updates-to-core`. Fixing seven confirmed review findings on the
push-target slice; all changes confined to `host/src/notify/**`, the push test files, and one
strictly-required additive prefs-schema fix.

## What was fixed

1. **BLOCKER — hardcoded workspace (rule 6).** `push_target.rs::effect_workspace()` returned a
   literal `"acme"` for every effect. Fix = the email-target pattern: `verbs.rs::notify_send`
   embeds `"workspace": ws` in the effect payload (already authorized against that ws);
   `PushPayload` gained `workspace: Option<String>`; `deliver()` fails the effect if it is
   absent/empty — never guesses. The function and its comment wall are deleted. Entry:
   `debugging/inbox-outbox/push-target-hardcoded-workspace.md`.
2. **At-least-once double-send.** New `notify/delivered.rs`: ws-scoped per-device delivered
   markers keyed `(idempotency_key, device_id)`, checked before each provider send and written
   after success — an outbox retry re-sends only the failures. `collapse_key` is forwarded to
   the provider (provider-side collapse), not used as the dedup key (distinct effects sharing a
   collapse key must each deliver once).
3. **Effect-id collision.** `notify:{now}:{first_recipient}` collided within one second (the
   outbox idempotency dedup swallowed the second notification). Now `notify:{ulid}` via
   `lb_store::new_ulid()` (the flows/tool_call precedent).
4. **Deliver-path tests through the real relay** — `host/tests/push_deliver_test.rs`, 7 tests
   driving `relay_outbox` (the loop `spawn_relay_reactors` ticks) over real enqueued effects:
   fan-out per device (+ collapse_key pass-through), token-gone → auto-disable + never re-sent,
   quiet-hours suppression, **non-member audience sub excluded** (mandatory ws isolation; the
   audience is now `membership_is_member`-checked in `deliver()`), partial-failure retry does
   NOT re-send succeeded devices, same-second sends don't collide, missing-workspace effect
   fails instead of guessing.
5. **Dead-code / wiring contract.** `PushTarget` is registered by the product host with
   `spawn_relay_reactors` (the `EmailTarget` contract) — stated in the scope doc; the
   relay-driven suite is the in-repo proof.
6. **Honest WebPush status.** Scope doc now says: trait + adapter + recording fake shipped;
   WebPush(VAPID) HTTP adapter + secrets-mediated credentials deferred, with the why and the
   runbook item.
7. **Warnings + deferred notes.** Unused imports cleaned in `notify/tool.rs`, `notify/verbs.rs`,
   `notify/mod.rs` (mod-level `pub use`s trimmed to what `lib.rs` re-exports; `PushError` added
   to the `lb_host` public surface — a product host implementing `PushProvider` needs it); dead
   `device_list_all_raw` removed. Scope doc records: `device.remove` disables (rows accrete, no
   pruning) and the admin device-count surface is not built — deferred.

## Found-while-testing (second debugging entry)

The quiet-hours test failed on first run: `push_muted` had been added to `lb_prefs::Prefs` but
not to the SCHEMAFULL `DEFINE FIELD` list nor `PREFS_COLUMNS` in `prefs/store/schema.rs` — the
write was silently dropped. Fixed additively (both prefs tables, `IF NOT EXISTS`). Entry:
`debugging/inbox-outbox/push-muted-pref-silently-dropped.md`. This was the one edit outside the
push-scope file set — strictly required for quiet hours to exist at all.

Also extended `RecordingPushProvider` (still the ONE sanctioned fake, same file) with scripted
per-device failures (`mark_token_gone`, `fail_next`) and a blanket `PushProvider for Arc<P>` so
tests keep a handle after boxing.

## Green output

```
cargo test -p lb-host --test push_target_test --test push_deliver_test
test result: ok. 7 passed; 0 failed  (push_deliver_test)
test result: ok. 9 passed; 0 failed  (push_target_test)
```

Plus `prefs_deny_test` (5 ok) + `prefs_mcp_test` (3 ok) guarding the schema change.
Cap-deny + ws-isolation tests: green (push_target_test's existing four + the new non-member
exclusion case). Pre-existing unrelated failures (`agent_persona_catalog_test`,
`agent_routed_test`) not touched.
