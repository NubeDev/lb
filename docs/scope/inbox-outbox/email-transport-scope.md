# Inbox-outbox scope — the real email transport (SMTP + provider APIs behind `EmailProvider`)

Status: **SHIPPED (v1) 2026-07-30** — issue [#118](https://github.com/NubeDev/lb/issues/118), branch
`updates-for-reports`, unreleased (needs the next `node-v*` tag). Session:
[`sessions/inbox-outbox/email-transport-session.md`](../../sessions/inbox-outbox/email-transport-session.md).
Public: `doc-site/content/public/inbox-outbox/email-transport.md`. Operator runbook:
[`skills/email-transport/SKILL.md`](../../skills/email-transport/SKILL.md).
**Read the "Shipped (v1)" section at the bottom before building on this** — it records what landed, the
answered open questions, and the three gaps that are still owed.

> Read with: `outbox-scope.md` (the must-deliver substrate + the generic `Target` trait this
> plugs into), `push-target-scope.md` (the sibling that took this exact shape — read its
> "Shipped (v1)" section first; every lesson there applies here),
> `../auth-caps/invites-scope.md` (the `EmailProvider` trait's origin + its only consumer
> today), `mail-source-scope.md` (the receive-side sibling this deliberately isn't),
> `../secrets/secrets-scope.md` (credential custody), README §3 rules 3/10.

**Every email this platform sends today goes nowhere.** The seam is built and booted — a
durable outbox effect, an `EmailTarget` routed on the opaque string `"email"`, an
`EmailProvider` trait, catalog-rendered i18n subject/body — but the only non-test impl is
`LoggingEmailProvider`, which logs the send and acks it. A workspace admin invites a
colleague, the invite row is written, the outbox drains, the log says `email (no provider
configured — logged only)`, and the colleague is never told. No product host wires a
provider either (`rubix-ai` ships none), so this is the live state, not a hypothetical. We
want **the transport actually built**: a real SMTP provider impl (`mail-send`) with
`STARTTLS`/implicit-TLS, auth including **XOAUTH2** so Gmail/Microsoft 365 work at all, a
`MailBuilder` for proper MIME (HTML + plain-text alternative, attachments), credentials via
`secrets/` mediation, and boot config that selects a provider by name. Core still names no
provider (rule 10): it routes an opaque effect to a trait.

## Goals

- **`lb-mail` crate, send half** (`rust/crates/mail/`): a pure-ish transport lib over
  [`mail-send`](https://github.com/stalwartlabs/mail-send) +
  [`mail-builder`](https://github.com/stalwartlabs/mail-builder) — build a MIME message
  (HTML body + generated plain-text alternative + attachments + inline images), open a
  connection, authenticate, send. No store, no bus, no capability logic; the crate is
  fixture/integration-testable on its own. Shares the crate with the receive half when
  `mail-source-scope.md` lands (`send/` and `fetch/` folders — one crate, RFC 5322 in one
  place, per FILE-LAYOUT folder-of-verbs).
- **`SmtpEmailProvider`** — the first real `impl EmailProvider`, in `host/src/outbox/`
  beside `email_target.rs`. Config: host, port, TLS mode (`implicit | starttls | none`),
  auth (`plain | login | xoauth2 | none`), the `From` identity, an envelope-sender
  override, and a timeout. It resolves credentials from `secrets/` **at send time** and
  never holds them in the record or the log.
- **XOAUTH2 + token refresh** — the load-bearing one for "support Gmail and others". Gmail
  and Microsoft 365 no longer accept a password for SMTP/IMAP in the general case; an
  access token is required, and access tokens expire in ~an hour. So the provider needs a
  **`MailAuth` seam** that yields a *fresh* bearer at send time: a stored refresh token
  (sealed in secrets) exchanged at the provider's token endpoint, cached until near
  expiry. Without this, "Gmail support" is a config field that fails in production an hour
  after setup.
- **An API provider impl behind the same trait** — one of SES / Mailgun / Postmark
  (recommend **Postmark or SES**, decided at build time per the open question), because
  most products end up on a provider API for deliverability (SPF/DKIM/DMARC alignment,
  bounce webhooks, reputation) rather than raw SMTP. This proves the trait admits both
  shapes and gives hosts the path they'll actually want. One file per provider.
- **Boot config selects the provider by name** — `BootConfig` gains an email transport
  config (`kind: "smtp" | "postmark" | "logging"` + its settings) so a host gets a working
  transport from configuration alone, instead of writing Rust to implement the trait. The
  existing `outbox_providers.email: Option<Arc<dyn EmailProvider>>` seam stays as the
  escape hatch for a host with its own transport.
- **Delivery outcome is honest** — a permanent failure (5xx, bad recipient) fails the
  effect **without retry**; a transient one (4xx, connection, throttle) returns `Err` so
  the outbox's existing backoff/retry owns it. Today every send "succeeds", which is worse
  than failing: it strands nothing and delivers nothing.

## Non-goals

- **Receiving mail.** `mail-source-scope.md` owns the inbound half (IMAP poller, cursor,
  normalization). This scope is strictly send. The two share the `lb-mail` crate.
- **A templating engine.** Subject/body already render through the `lb_prefs` MF2 catalog
  (`invite.email.*`); this scope adds the **HTML** half of the same catalog-rendered
  message and nothing more. No new template language, no per-workspace template editor.
- **Bounce/complaint ingestion.** Provider webhooks (bounces, spam complaints,
  unsubscribes) are a real need and a separate ask — they arrive over `POST /hooks`
  (`../ingest/webhooks-scope.md`) and would want a suppression list. Named, deferred.
- **A suppression/unsubscribe list.** Follows bounce ingestion; transactional mail
  (invites, alerts) is the v1 traffic and doesn't need CAN-SPAM unsubscribe. Marketing
  mail is out of scope for the platform entirely.
- **DKIM signing in core.** `mail-send` can sign, but for SMTP-to-a-relay and for every
  API provider the *relay* signs with the domain's key — signing in-process means custody
  of a domain private key for no gain. Named the moment a host sends direct-to-MX, which
  we're not recommending (see Risks).
- **Interactive OAuth consent in the browser.** v1 takes a refresh token an operator
  obtained out-of-band and sealed in secrets. The mint-a-token-from-the-UI flow is a
  gateway/UI follow-up shared with `mail-source-scope.md` — the custody model
  (secrets-path-only) doesn't change when it lands.
- **A new caller-facing verb.** `notify.send`'s email twin is not in this scope: callers
  reach email by enqueuing an outbox effect with `target: "email"` (as `invite_create`
  does). A generic `email.send` verb is a plausible follow-up but needs an
  anti-abuse story first (a granted cap that lets an extension mail arbitrary addresses is
  a spam cannon — see Risks).

## Intent / approach

**Fill in a built seam; don't build a mail service.** Durability, retry, backoff,
at-least-once, and idempotency already belong to the outbox; i18n already belongs to
`lb_prefs`; routing already belongs to `RouterTarget`'s opaque string. What's missing is
exactly one thing: an `EmailProvider` that puts bytes on a wire. So the whole design is a
pure transport crate plus two trait impls plus a config struct — the same "it's a target,
not a service" move `push-target-scope.md` made, one layer further in.

**Crate choice: the Stalwart suite (`mail-send` + `mail-builder`), not `lettre`.** `lettre`
is more downloaded, but `mail-send` is natively async (no feature-flag maze, no
`spawn_blocking` bridge inside the relay reactor), and the suite shares an author with
`mail-parser`, which the receive half wants for its malformed-MIME tolerance — one
ecosystem across `lb-mail`'s two halves rather than two mail stacks in one workspace.
`mail-builder` also composes nested MIME (HTML + text alternative + attachments) more
directly than `lettre`'s builder.

*Rejected — `lettre`:* async is a flag matrix, no DKIM, and it would leave the receive half
on an unrelated stack.
*Rejected — hand-rolled SMTP:* SMTP is a swamp of TLS modes, SASL mechanisms, and 4xx/5xx
semantics; there is no upside.
*Rejected — API-provider-only (no SMTP impl):* on-prem and self-hosted deployments (a real
posture for this platform — symmetric nodes, edge placement) often have only an internal
relay and no internet egress to a SaaS API. SMTP is the portable floor.
*Rejected — leaving the logging provider as the default:* it is the current bug. Keep it as
an *explicit* `kind: "logging"` choice for dev, but an unset-but-expected transport should
be a **loud boot warning**, not a silent success.

## How it fits the core

- **Tenancy / isolation:** the effect payload already carries `workspace` (the
  hardcoded-workspace bug in `push_target` is the cautionary tale — see
  `debugging/inbox-outbox/push-target-hardcoded-workspace.md`); the provider receives it in
  `EmailMeta` and must never resolve credentials from a workspace other than the effect's.
  **Decision:** the transport config is **node-level** in v1 (one relay per node, like a
  system mailer), with the workspace carried for logging/auditing and for a per-workspace
  `From` override resolved from that workspace's secrets. Per-workspace *transports* (ws A
  sends through its own Gmail) is a named follow-up: it needs the credential-per-ws story
  and a config record, not a boot struct.
- **Capabilities:** N/A for the transport itself — the relay reactor is host machinery, not
  a caller surface, and nothing new is reachable. The gate lives on the verb that
  *enqueues* (`invite.create` today, gated `mcp:invite.create:call`). This is exactly why a
  generic `email.send` verb is a non-goal: it would need its own cap **and** a rate limit.
  The deny path to test is therefore the enqueue side, unchanged.
- **Placement:** either, by config. A node without egress runs `kind: "logging"` or points
  at a LAN relay; the hub typically holds provider credentials. No `if cloud` — the
  transport is a config-selected trait impl (rule 1).
- **MCP surface (§6.1):** **none.** No CRUD (config is boot config, not a record — v1), no
  get/list, no live feed, no batch. This is the honest answer for a transport: adding
  verbs to configure a mailer would be CRUD with no caller. The follow-ups that *would*
  add verbs are named above (per-ws transport config, `email.send`, suppression list).
- **Data (SurrealDB):** no new tables. The outbox record is the delivery ledger; the
  provider writes nothing. Secrets live in the existing secrets store, by path.
- **Bus (Zenoh):** none. Email is must-deliver, which is precisely why it goes through the
  outbox and not pub/sub (rule 3). The relay reactor's tick is the only motion.
- **Sync / authority:** N/A — the transport is stateless. The outbox row is
  node-authoritative and already handles at-least-once; a retried effect re-sends, so the
  provider must be safe to call twice (see Risks: double-send).
- **Secrets:** the load-bearing one. SMTP password / OAuth refresh token / API key resolve
  via `lb_secrets::get` at send time by **path**; the config struct holds path names only.
  The posture must survive logs, error strings, and `Debug` — an SMTP error that echoes the
  AUTH line leaks the password, so the provider maps provider errors to sanitized strings
  (test it explicitly, the same discipline as push tokens).
- **No mocks (rule 9):** an SMTP relay and a provider HTTP API are **true externals**
  behind the existing `EmailProvider` trait, so the sanctioned fake already exists
  (`RecordingEmailProvider`, one named file). But the fake must not be the only proof the
  transport works: the `lb-mail` send path is exercised against a **real SMTP server** in
  tests — either a container (`docker/`) or a tiny in-test listener that speaks enough SMTP
  to complete a session and hand back the received bytes. That listener is a *real server
  on a real socket*, not a mock of our own code, so it is squarely allowed — and it is the
  only way to prove TLS/auth/MIME rather than asserting our own recorder.
- **One responsibility per file:** `crates/mail/src/send/{message.rs, smtp.rs, tls.rs,
  auth/{plain.rs, xoauth2.rs}}`; `host/src/outbox/{provider_smtp.rs, provider_api.rs}`.
  Each ≤400 lines; no `mail/utils.rs`.
- **SDK/WIT impact:** none. Extensions never touch the transport.
- **Skill doc:** **yes** — `docs/skills/email-transport/SKILL.md`. Configuring a mailer is
  an operator task with a credential ceremony (app password vs OAuth refresh token, the
  Gmail/365 specifics, which secrets paths to seal, how to verify with a real send, how to
  read a failed outbox row). `push-target-scope.md` flagged the same "credential ceremony"
  risk and shipping the runbook with the slice is the mitigation. The implementing session
  owns writing it, grounded in a live run.

## Example flow

1. An operator seals credentials: `secrets.set { path: "mail/smtp-password", … }` (or
   `mail/gmail-refresh-token` for XOAUTH2) — values into secrets, never into config.
2. The node boots with `email: { kind: "smtp", host: "smtp.gmail.com", port: 587, tls:
   "starttls", auth: "xoauth2", user: "reports@acme.com", secret_path:
   "mail/gmail-refresh-token", token_endpoint: "https://oauth2.googleapis.com/token",
   from: "Acme <reports@acme.com>" }`. `reactors.rs` builds `SmtpEmailProvider` from it and
   routes `EMAIL_TARGET` to `EmailTarget::new(provider)` — the existing wiring, now with a
   real impl behind it.
3. An admin calls `invite.create` (gated). The invite row + the outbox effect are written
   in one transaction; the effect payload carries `email`, `workspace`, `token`, `locale`.
4. The relay reactor ticks, `RouterTarget` dispatches on `"email"`, `EmailTarget` renders
   subject + body from the `invite.email.*` catalog in the effect's locale, and calls the
   provider.
5. `SmtpEmailProvider` resolves the refresh token from secrets, exchanges it for an access
   token (cached, ~1h), builds a MIME message (HTML + plain-text alternative), opens
   STARTTLS to `smtp.gmail.com:587`, authenticates XOAUTH2, sends, reads `250 OK`, returns
   `Ok`. The outbox marks the effect delivered.
6. Gmail returns `421 4.7.0 too many auth attempts` instead → the provider returns `Err`,
   the outbox backs off and retries later; nothing is lost, the invite row is untouched.
7. A different recipient is a typo'd domain → `550 5.1.2` → permanent: the effect is failed
   (no retry storm) and the row records the reason an operator can read.

## Testing plan

Mandatory categories:

- **Capability deny:** the transport exposes no verb, so the deny under test is the
  **enqueue** side — `invite.create` without `mcp:invite.create:call` → 403 **before** any
  effect is written (assert no outbox row, not just the 403). Per
  `debugging`-memory discipline: also assert the property only the outer gate has, and
  revert-check the test so it fails on unfixed code.
- **Workspace isolation:** two workspaces enqueue email effects; each delivered message
  carries its own workspace's `From`/config resolution, and a ws-A effect can never resolve
  a ws-B secret path. Cross-check with the `push_target` hardcoded-workspace regression —
  assert the workspace comes from the payload, and that an effect **missing** `workspace`
  fails rather than defaulting.
- **Offline/sync:** kill the node mid-relay with an effect staged → restart re-drains and
  the message is sent exactly once per the outbox's idempotency (see double-send in Risks).

Key cases:

- **Against a real SMTP server** (the point of the slice): plain send; STARTTLS upgrade;
  implicit TLS; AUTH PLAIN; AUTH XOAUTH2 (token built correctly, including the SASL
  base64 framing); a multipart message asserted by **parsing the received bytes** (HTML
  part + text alternative + one attachment, headers correct, 8-bit subject encoded).
- **Error mapping:** 4xx → `Err` (retryable, outbox backs off); 5xx → permanent failure, no
  retry; connection refused / timeout → retryable; TLS verification failure → permanent and
  loud (never silently downgraded to plaintext).
- **Secret hygiene:** a failing AUTH must produce an error string containing **no**
  credential material — assert the password/token substring is absent from the error, the
  `Debug` of the config, and captured logs.
- **Token refresh:** an expired access token triggers exactly one refresh and one resend;
  a refresh failure is retryable, not a permanent fail.
- **Boot:** a node with no email config boots, warns loudly, and drains via the logging
  provider (extend `node/tests/relay_boot_test.rs`); a node with `kind: "smtp"` and an
  unreachable host boots (no crash) and its effects retry.
- **Regression green:** `invite_email_relay_test`, `invite_i18n_test`, `relay_boot_test`,
  `push_deliver_test` unchanged.

## Risks & hard problems

- **"Support Gmail" is an OAuth problem, not an SMTP problem.** This is the single most
  underestimated part. Google has been switching off app passwords / less-secure-app access
  for a decade of increments, and Microsoft removed basic auth from Exchange Online
  outright. A `password` config field will demo fine against a test account and fail for
  real tenants. XOAUTH2 + refresh must be in v1, and the *consent* flow (how an operator
  gets a refresh token at all) must at minimum be documented in the skill doc even though
  the browser flow is deferred.
- **Deliverability is not a code problem.** Direct-to-MX from a node will be filed as spam
  regardless of correct code (no SPF/DKIM/DMARC alignment, no IP reputation, residential/
  cloud IP blocks). The honest recommendation to hosts — which belongs *in the docs*, not
  buried — is: send through a relay or provider API with the domain's DNS set up. Building
  a perfect SMTP client and calling deliverability "done" is the trap here.
- **Double-send on retry.** The outbox is at-least-once; email has no collapse key and a
  duplicate invite email is a visible, embarrassing failure. Worse, a message can be
  *accepted* by the relay and the ack lost, so the retry cannot distinguish. `push_target`
  solved its version with a per-`(idempotency_key, device_id)` delivered marker; the email
  analogue is a per-`(idempotency_key, recipient)` marker written **before** the send is
  reported, accepting that a crash between accept and marker still duplicates. State the
  window; don't pretend it's exactly-once.
- **Secret leakage through error paths.** Mail libraries are chatty on failure; an
  unsanitized SMTP transcript in a log is a credential disclosure. This needs an explicit
  test, not care.
- **A granted `email.send` would be a spam cannon** — which is why it's a non-goal here.
  When it lands it needs a per-workspace rate limit and probably a recipient allowlist;
  flagging it now so a future session doesn't add "just a thin verb".
- **Blocking the relay reactor.** SMTP sessions can hang for minutes; an unbounded send
  inside the relay tick stalls *all* outbox delivery, including push. A per-send timeout is
  mandatory, not a nicety.
- **The receive half will want to share this crate.** Getting `lb-mail`'s folder split right
  now (`send/` vs `fetch/`, shared MIME/address types) avoids a painful reshuffle when
  `mail-source-scope.md` builds.

## Open questions

- **Which API provider first — SES, Postmark, or Mailgun?** Recommend **Postmark** for the
  simplest transactional API and best default deliverability, or **SES** if hosts are
  already AWS-resident. Decide at build time from the first real host's posture
  (`rubix-ai`).
- **Node-level vs per-workspace transport config in v1.** Scoped node-level above; confirm
  no near-term product needs per-workspace sending identities (a white-labelled
  multi-tenant product would).
- **Config record vs boot config.** Boot config is scoped (no verbs, no CRUD-with-no-caller).
  But a stored, admin-editable transport record is what a real operator wants (rotate a
  relay without a redeploy). Is the boot-only v1 acceptable, or does the first host need
  the record + `mail.transport.*` verbs immediately?
- **Where does the plain-text alternative come from?** The catalog holds one body string
  today. Options: catalog gains `invite.email.body_html`, or the HTML is generated from the
  markdown-ish body, or the text part is generated by stripping the HTML. Recommend
  **two catalog keys** (translators see both) with a generated fallback when the HTML key
  is absent.
- **The delivered-marker granularity** — reuse `notify/delivered.rs` generically (rename to
  an outbox-level `delivered` service keyed by `(idempotency_key, target, recipient)`), or
  a mail-specific ledger? Reuse is tempting; check it doesn't leak push assumptions.
- **Does the real-SMTP test use a container or an in-test listener?** A listener keeps
  `cargo test --workspace` self-contained (no docker in CI path); a container proves real
  TLS. Recommend: in-test listener for the message/auth/error matrix, plus one
  docker-compose-gated integration test for real TLS, skipped when absent.

## Shipped (v1) — 2026-07-30

What landed, in the order it matters:

- **`lb-mail`** (`rust/crates/mail/`, send half) over `mail-send` + `mail-builder`, features trimmed to
  drop DKIM / the MD5 SASL mechanisms / `aws-lc-rs` (`ring` is already the workspace's rustls provider).
  `send/{message,smtp,tls}.rs` + `send/auth/{plain,xoauth2,refresh}.rs` + `error.rs`. `fetch/` is not
  scaffolded — the folder split is documented and the receive half creates it.
- **`SmtpEmailProvider`** and **`PostmarkEmailProvider`** (`host/src/outbox/provider_*.rs`) behind the
  unchanged `EmailProvider` trait, credentials resolved **per send in the effect's workspace** by path
  through `lb_secrets::get_workspace` (the `agent/resolve_key.rs` precedence: sealed → env → unset).
  XOAUTH2 with refresh is in v1 as the scope insisted, and the token cache is keyed by a **digest** of the
  refresh token so rotating the seal invalidates the bearer minted from the old grant.
- **Boot selects by name**: `BootConfig::email_transport` (`Logging | Smtp | Postmark`), built in
  `node/src/mail.rs`, `LB_MAIL_*` at the binary boundary. Unset ⇒ still boots and drains through the
  logging provider, now with a **loud warning**; a config that cannot possibly send is reported at boot
  and falls back to logging rather than pretending.
- **The outbox learned to fail honestly** — the part the scope asked for that the substrate could not
  express. `Target::deliver` now returns `DeliveryError { reason, permanent }` (`From<String>` ⇒
  transient, so adopting it was a signature change, not a semantic one); `Effect` gained
  `last_error`; `mark_failed` records the reason and new `mark_dead_lettered` parks a permanent failure
  at `attempts: 1`. The relay used to **discard** the target's reason (`Err(_reason)`), so a parked effect
  could only say "failed 5 times". `outbox.mark_failed` gained optional `reason`/`permanent` for
  sidecar-driven relays; omitting both is the old behaviour.
- **Two catalog keys**: `invite.email.body_html` in `en`/`es`, paired with the text half as
  `multipart/alternative`; an absent HTML key renders as the key literal, which
  `email_target::catalog_html` turns back into "text-only".
- **Retry dedup**: a new outbox-level `outbox/delivered.rs` keyed by `(target, dedup_key, recipient)`,
  plus a stable `Message-ID` derived from the same key so the *receiving* side can collapse a duplicate.

### Open questions, answered

1. **API provider** → **Postmark**. Simplest transactional API (one `POST`, no request signing), best
   default deliverability, and decisively: **SES already speaks SMTP**, so an AWS-resident host is served
   by `kind: "smtp"` without dragging SigV4 or an AWS SDK into a core crate.
2. **Node-level vs per-workspace transport** → node-level, as scoped. Per-workspace sending identities
   stay a named follow-up.
3. **Config record vs boot config** → boot config, and the operator pressure the question anticipated is
   mostly relieved by resolving credentials per send: **rotating a credential needs no restart**. Changing
   the relay itself still does. A stored `mail.transport.*` record becomes CRUD-with-a-caller once a
   per-workspace transport exists, and belongs with that slice.
4. **Plain-text alternative** → two catalog keys, generated fallback when the HTML key is absent.
5. **Delivered-marker granularity** → a new outbox-level ledger (above). `notify/delivered.rs` keeps its
   own `push_delivered` table: migrating live push markers buys nothing but a one-time re-send.
   Consolidating the two is a follow-up.
6. **Container vs listener for the real-SMTP test** → in-test listener, so `cargo test --workspace` stays
   self-contained. See the gaps below for the container half.

### Gaps still owed (stated, not hidden)

- **No real-TLS test.** The docker-compose-gated integration test is not in this slice; TLS is covered by
  construction and by error mapping, and exercised by hand against a real relay.
- **Postmark's wire is untested** — its validation and error classification are, the HTTP call is not (it
  is a true external with no local equivalent).
- **The invite link is relative** (`/accept?token=…`), as it was before this slice: not clickable in a
  mail client. A host can already fix it per workspace via a catalog override; a node-level public base
  URL is a separate small ask against `invite_create`, not the transport.
- **Double-send keeps its irreducible window** — a crash between "the relay accepted" and "the marker is
  committed" duplicates. Two mitigations, neither exactly-once.
- **The push delivered ledger was not migrated** to the outbox-level one (above).

### Regression tests to keep green

`lb-mail`: `smtp_send_test` (7, against a real SMTP server on a real socket), `token_refresh_test` (6,
against a real HTTP token endpoint), 14 unit. `lb-host`: `email_transport_test` (6 — cap-deny with no
outbox row, ws isolation, missing-workspace parked, re-drain exactly once, permanent parked at attempt 1,
both body halves in the effect's locale), plus `invite_email_relay_test`, `invite_i18n_test`,
`push_deliver_test`, `approval_release_test`, `outbox_relay_ops_test`. `lb-node`: `relay_boot_test` (3,
including a node booted against an unreachable relay whose effect stays **owed**).

All four security/behaviour-critical tests were **revert-checked** (each fails against deliberately
broken code): the cap gate, the workspace wall, the permanent-park transition, and the delivered marker.

## Related

- `outbox-scope.md` (the substrate) · `push-target-scope.md` (the sibling target — read its
  "Shipped (v1)" amendments before building) · `mail-source-scope.md` (the receive half,
  shares `lb-mail`) · `../auth-caps/invites-scope.md` (the `EmailProvider` trait's origin
  and only consumer today) · `../secrets/secrets-scope.md` · `../ingest/webhooks-scope.md`
  (where bounce webhooks would land) · `../prefs/` (the MF2 catalog that renders the body).
- Code today: `rust/crates/host/src/outbox/email_target.rs` (the trait + target +
  logging/recording impls), `rust/node/src/reactors.rs` (the boot wiring),
  `rust/node/src/config.rs` (the provider seam), `rust/crates/host/tests/invite_email_relay_test.rs`.
- Skill doc (implementing session owns): `docs/skills/email-transport/SKILL.md`.
- README §3 rules 1/3/10, §6.10 (jobs/outbox), §6.12.
- `key-stack.md` — the new `lb-mail` row.
