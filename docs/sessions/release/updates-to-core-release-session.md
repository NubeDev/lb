# Session — finish and tag `updates-to-core` (relay boot wiring + i18n + release)

- **Date:** 2026-07-11
- **Scope:** [../../scope/release/updates-to-core-release-scope.md](../../scope/release/updates-to-core-release-scope.md)
- **Status:** done
- **Outcome:** both gaps closed generically; branch merged to `master`; tags `node-v0.2.0`,
  `minimal-shell-v0.2.0`, `ui-v0.7.0` (lb-ext-ui-sdk).

## Gap 1 — relay boot wiring (the blocker)

`spawn_relay_reactors` had zero production call sites; invites/push enqueued effects nothing
drained on a running node. Shipped:

- **`RouterTarget`** (`host/src/outbox/router_target.rs`) — a composite `Target` dispatching on
  the effect's opaque `target` string (rule 10: no adapter is named, no id branched on). Includes
  the `DynTarget` object-safe twin (the `Target` trait's `impl Future` isn't dyn-usable). An
  unregistered target `Err`s → normal retry → dead-letter with a clear reason, never a silent drop.
- **`OutboxProviders`** on `BootConfig` (`node/src/config.rs`) — the additive provider-injection
  seam: `email: Option<Arc<dyn EmailProvider>>`, `push: Option<Arc<dyn PushProvider>>`. `None` ⇒
  the new `LoggingEmailProvider`/`LoggingPushProvider` (log + ack) so boot never crashes and the
  outbox never strands. Blanket `Arc<P>` provider impls widened to `?Sized` so `Arc<dyn …>` works.
- **`node/src/reactors.rs::spawn`** now builds the router (`EMAIL_TARGET`→`EmailTarget`,
  `PUSH_TARGET`→`PushTarget`) and spawns `spawn_relay_reactors` on a 2s tick, beside the other
  reactors, gated by the same `cfg.reactors`.
- **Boot-level proof** (`node/tests/relay_boot_test.rs`, 2 tests): a `boot_full` node with
  recording providers injected — `invite.create` lands on the recording **email** provider and
  `notify.send` on the recording **push** provider **via the spawned reactor** (no direct
  `relay_outbox` call); and a no-provider boot still boots and drains (logging ack).

Not redone (already shipped + tested pre-session): invite-accept rate limiting, relay loop body,
Email/Push target adapters.

## Gap 2 — i18n (en + es on every user-facing surface, via the ONE catalog engine)

### (a) Invite locale — `authz/src/invite.rs`, `host/src/invites/*`
Additive serde-default `locale: Option<String>` on `Invite`; `invite.create` takes `locale`
(validated against `lb_prefs::language_enabled` — unknown code is `BadInput` at mint, not a silent
fallback); carried in the email effect payload (create AND resend); **new pre-auth verb
`invite.verify`** (`invites/verify.rs`) + gateway route `GET /public/invite/verify` (same rate
limiter as accept) exposing `{email, locale, redeemable}` so the accept page renders pre-auth in
the invite's language; on accept the locale is **copied into the member's `language` pref**
(`set_user_prefs` merge-patch in `onboard`).

### (b) Invite email — `host/src/outbox/email_target.rs`
Subject/body now render through `lb_prefs::render_message` with new `invite.email.subject`/`.body`
keys in **both** `en.mf` and `es.mf` (client twin regenerated via `gen-prefs-catalog`; the `.mf`
key-parity test still gates drift). The old "no templating in core" non-goal is **overturned** —
reversal recorded in the invites scope doc.

### (c) Push — `host/src/notify/{verbs,push_target,tool}.rs`
`notify.send` accepts `title_key`/`body_key`/`args` (`NotifyCatalogRef`); literal title/body stay
the compat path (literals are never translated; neither-title-nor-key is `BadInput`). `PushTarget`
resolves **each recipient's** prefs at deliver time and renders per-recipient.

### (d) Shell + SDK — `packages/minimal-shell`, `../lb-ext-ui-sdk`
- `@nube/ext-ui-sdk` **0.7.0**: additive `src/i18n.ts` — `resolveLocale` (user pref →
  `navigator.language` base → `en`), `makeTranslator` (locale → en → key-literal chain, `{arg}`
  interpolation, never blank), and `catalogParity` (the TS twin of the `.mf` key-parity gate).
  Rich MF1 stays the host's job (documented in the module) — no second engine. `dist/` rebuilt
  and committed. 6 new tests.
- `@nube/minimal-shell` **0.2.0**: all user-facing strings through `src/i18n.tsx` catalogs
  (en + es, 10 keys); `I18nProvider` resolves the locale (post-login best-effort `prefs.resolve`
  → browser language → en). `src/i18n.test.tsx` is the CI completeness gate (`catalogParity` +
  no-empty-message + Spanish/English render through the real `App`).

## Test evidence (green)

```
lb-host  invite_i18n_test:  5 passed  (es email, en fallback, bad locale, pre-auth verify, pref copy)
lb-host  push_i18n_test:    3 passed  (per-recipient en/es render, literal compat, BadInput guard)
lb-node  relay_boot_test:   2 passed  (drain-at-boot email+push via spawned reactor; no-provider boot)
minimal-shell (vitest):     9 passed  (2 files — parity gate + locale chain + existing App tests)
lb-ext-ui-sdk (vitest):    20 passed  (4 files — incl. 6 new i18n tests)
cargo test --workspace (--no-fail-fast): 362 test binaries green. Red, all pre-existing /
environmental, none in this branch's areas:
  - lb-cli reminder_test (1) — the ONE allowed failure (logged in debugging/cli/…).
  - agent_persona_catalog_test (6) + agent_persona_coding_test (2) — the persona grounding red
    STATUS already records as pre-existing ("NOT this branch").
  - proof_panel_test (17) + devkit build_test (1) + devkit_e2e_test (1) — missing wasm ARTIFACTS:
    proof-panel's own build.sh fails on `DEP_LB_SDK_WIT: NotPresent` (the out-of-tree lb-sdk git
    dep doesn't export its WIT links metadata in this environment); the test harness itself says
    "Build it first". Environmental, and proof-panel is a retained-temporarily in-tree reference
    ext (see MIGRATION.md).
```

(Full suite output on the tag pasted in the release notes section of STATUS.)

## Debugging (entries filed)

- MF1 apostrophe/quoting silently degrades a message to its key literal →
  [debugging/prefs/mf1-apostrophe-quotes-break-catalog-message.md](../../debugging/prefs/mf1-apostrophe-quotes-break-catalog-message.md)
- minimal-shell's SDK `link:` path pointed inside the lb repo (types-only imports hid it) →
  [debugging/frontend/minimal-shell-sdk-link-path-wrong.md](../../debugging/frontend/minimal-shell-sdk-link-path-wrong.md)
- Pre-existing `lb-cli reminder_test` deny, logged not chased →
  [debugging/cli/reminder-create-denied-in-cli-round-trip-test.md](../../debugging/cli/reminder-create-denied-in-cli-round-trip-test.md)

Also fixed in passing (pre-existing, surfaced by running the shell's `tsc` build): `singletons.ts`
implicit-any `globalThis` writes; `federation.ts` `PageBridge.call` signature drifted from the
SDK's generic signature.

## Decisions (alternatives rejected)

- **RouterTarget in core vs a product-host composite** — core: every downstream would rewrite the
  same dispatch, and the seam stays opaque-string generic.
- **Logging no-op default vs failing effects when unconfigured** — logging ack: never crash boot,
  never dead-letter a workspace's invites because email isn't set up yet; the log line is the
  operator's signal.
- **Simple-interpolation TS seam vs shipping intl-messageformat in the SDK** — simple: shell/ext
  labels need `{arg}` only; server-generated rich MF1 already has the generated client twin.
  Documented in `i18n.ts` so nobody forks a plural engine later.
- **Version:** `node-v0.2.0` (minor bump — new verb surface: `invite.verify`, `invite.create
  locale`, `notify.send` keys, `BootConfig.outbox_providers`).

## Deferred (explicit, per scope)

Media HTTP Range, real WebPush VAPID provider, FCM/APNs adapters, SMTP provider, orphaned-upload
GC — all behind existing traits/seams; none block the tag.
