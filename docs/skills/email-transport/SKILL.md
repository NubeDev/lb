---
name: email-transport
description: >-
  Configure and verify the node's outgoing mail transport (SMTP or a provider API) so invites and other
  outbox email actually get delivered. Use when a task says "the invite email never arrived", "set up
  SMTP", "support Gmail / Microsoft 365 sending", "seal the mail password", "rotate the mail
  credential", "why is email logged and dropped", "read a failed outbox row", or "email works in dev but
  not in production". Covers the boot transport config (kind smtp | postmark | logging), the credential
  ceremony (app password vs OAuth2 refresh token, which secrets paths to seal), TLS/auth modes, the
  4xx-retry vs 5xx-park outcome contract, how to verify with a real send, and how to diagnose a
  dead-lettered effect.
---

# Configure the mail transport (invites and outbox email)

Outgoing email is an **outbox effect**, not a verb. A caller (today `invite.create`) stages a durable
effect with `target: "email"`; the relay reactor picks it up on its tick, renders subject/body from the
i18n catalog, and hands it to the node's configured `EmailProvider`. Durability, retry, backoff and
at-least-once all belong to the outbox — the transport's only job is putting bytes on a wire.

**There is no `email.send` verb, deliberately.** A granted capability to mail arbitrary addresses is a
spam cannon; it needs a rate limit and probably a recipient allowlist first. Email is reached by
enqueuing an effect.

> **Before this shipped (issue #118), every email was logged and dropped.** The only provider was
> `LoggingEmailProvider`, which logged the send and *acked* it — so an admin invited a colleague, the
> outbox drained clean, and the colleague was never told. If you see
> `email DROPPED (no transport configured — logged only)` in the log, that is what is happening: no
> transport is configured.

## Pick a transport

| kind | when | what you need |
| --- | --- | --- |
| `smtp` | a relay you or your customer runs, an internal LAN relay, an on-prem/edge node, SES-via-SMTP, Gmail/M365 | host, port, TLS mode, auth mechanism, a sealed credential |
| `postmark` | you want good deliverability with no mail plumbing | a Postmark server token, a verified sender signature |
| `logging` | dev only — print the mail, deliver nothing | nothing (and no boot warning, because you asked for it) |

**Deliverability is not a code problem.** Sending direct-to-MX from a node gets filed as spam no matter
how correct the client is: no SPF/DKIM/DMARC alignment, no IP reputation, and cloud/residential IP
blocks. Send through a **relay or a provider API**, with the sending domain's DNS set up (SPF record,
DKIM key published by whoever signs, a DMARC policy). This is the single highest-value thing to get
right, and none of it lives in this repo.

## 1. Seal the credential (the ceremony)

Credentials never go in config. The config carries a **secrets path** (or an env-var *name*) and the
value is resolved **at send time, in the effect's workspace** — so a ws-A effect can never resolve
ws-B's secret, and rotating the secret needs no redeploy.

Seal it at **workspace visibility**, because the relay reactor is host machinery with no user principal
to carry a `secret:<path>:get` capability. A `private` secret is deliberately not host-resolvable.

```jsonc
// POST /mcp/call — as an admin holding secret:mail/smtp-password:write
{ "tool": "secret.set", "args": {
    "path": "mail/smtp-password",
    "value": "<the app password or relay password>",
    "visibility": "workspace"
} }
```

Which value to seal depends on the mechanism:

| auth | seal at `secret_path` | also needs |
| --- | --- | --- |
| `plain` / `login` | the SMTP password (or provider app password) | — |
| `xoauth2` | the OAuth2 **refresh token** | `token_endpoint`, `client_id`, and the client secret (its own sealed path) |
| `none` | nothing | only valid for a trusted relay that requires no auth |

### Gmail and Microsoft 365 need XOAUTH2, not a password

This is the part that bites. Google has been switching off app passwords / less-secure-app access in
increments for years, and Microsoft removed basic auth from Exchange Online outright. A password field
will demo fine against one test account and fail for real tenants. And an access token expires in about
an hour — which is why refresh is built in rather than deferred: **without refresh, "Gmail support" is a
config field that breaks an hour after setup.**

Getting a refresh token is an operator task done **out of band** (the browser consent flow is not in the
product yet):

1. In Google Cloud Console, create an OAuth 2.0 **Desktop app** client for the project; note the client
   id and client secret.
2. Consent once as the sending mailbox, with the scope `https://mail.google.com/`, using
   `access_type=offline` — that is what makes the response include a `refresh_token`. Google's own
   `oauth2l`, or any one-off script, is enough; you need to do this exactly once per sending mailbox.
3. Seal both values:
   - the refresh token at `mail/gmail-refresh-token` (this is the `secret_path`),
   - the client secret at `mail/gmail-client-secret` (the `client_secret_path`).
4. Set `token_endpoint: "https://oauth2.googleapis.com/token"` and the client id in config.

Microsoft 365 is the same shape with a different endpoint
(`https://login.microsoftonline.com/<tenant>/oauth2/v2.0/token`) and the
`https://outlook.office.com/SMTP.Send` scope, against `smtp.office365.com:587` STARTTLS.

The node exchanges the refresh token for an access token at send time, caches it until shortly before
expiry, and re-mints as needed. A revoked or wrong refresh token comes back as `invalid_grant`, which is
treated as **permanent** — the effect is parked with that reason rather than retried, because only you
can fix it.

## 2. Configure the transport

An embedding product host fills `BootConfig::email_transport`; the standalone `node` binary reads
`LB_MAIL_*`. Both land in the same struct.

```bash
# A hosted relay with a password (the common case)
LB_MAIL_KIND=smtp
LB_MAIL_HOST=smtp.example.com
LB_MAIL_PORT=587
LB_MAIL_TLS=starttls              # implicit | starttls | none
LB_MAIL_AUTH=plain                # none | plain | login | xoauth2
LB_MAIL_USER=reports@acme.com
LB_MAIL_SECRET_PATH=mail/smtp-password
LB_MAIL_FROM='Acme Reports <reports@acme.com>'
LB_MAIL_TIMEOUT_SECS=30
```

```bash
# Gmail via XOAUTH2
LB_MAIL_KIND=smtp
LB_MAIL_HOST=smtp.gmail.com
LB_MAIL_PORT=587
LB_MAIL_TLS=starttls
LB_MAIL_AUTH=xoauth2
LB_MAIL_USER=reports@acme.com
LB_MAIL_SECRET_PATH=mail/gmail-refresh-token
LB_MAIL_OAUTH_TOKEN_ENDPOINT=https://oauth2.googleapis.com/token
LB_MAIL_OAUTH_CLIENT_ID=1234-abc.apps.googleusercontent.com
LB_MAIL_OAUTH_CLIENT_SECRET_PATH=mail/gmail-client-secret
LB_MAIL_FROM='Acme Reports <reports@acme.com>'
```

```bash
# Postmark
LB_MAIL_KIND=postmark
LB_MAIL_SECRET_PATH=mail/postmark-token
LB_MAIL_FROM='Acme Reports <reports@acme.com>'
LB_MAIL_STREAM=outbound
```

Env vars are read **only** at the binary boundary. An embedder constructs the config directly:

```rust
let mut cfg = BootConfig::default();
cfg.email_transport = Some(EmailTransport::Smtp(SmtpTransportConfig {
    host: "smtp.example.com".into(),
    port: 587,
    tls: TlsMode::Starttls,
    auth: MailAuthMechanism::Plain,
    username: "reports@acme.com".into(),
    secret_path: "mail/smtp-password".into(),
    from_name: "Acme Reports".into(),
    from_addr: "reports@acme.com".into(),
    ..Default::default()
}));
```

A host with its own mailer keeps using the escape hatch — `cfg.outbox_providers.email =
Some(Arc::new(MyProvider))` — which **wins** over this config.

### Notes on the knobs

- **TLS mode is config, never inferred from the port**, and `starttls` **requires** the upgrade: a
  server that does not advertise it fails the send rather than continuing in the clear (the next thing
  on that socket would be your AUTH line). A TLS verification failure is permanent and loud — never
  silently downgraded.
- **`plain` vs `login`** both mean "username + password"; the actual SASL mechanism is negotiated from
  the server's `EHLO`. Set whichever your provider's setup page names.
- **The timeout is mandatory** (default 30s). The send runs inside the relay tick, so an unbounded SMTP
  session would stall *every* outbox delivery behind it, push included.
- **A typo'd `tls`/`auth` value is reported and the safe default kept** — it never quietly picks
  something weaker.

## 3. Verify with a real send

Boot the node and read the log line — it states the transport, so a misconfiguration is visible before
any user is affected:

```
INFO email transport: smtp host=smtp.example.com port=587 tls=starttls auth=plain from=reports@acme.com
```

Anything else means email is not going out:

| log line | meaning |
| --- | --- |
| `email transport: NONE configured — every email will be logged and DROPPED` | no transport set (issue #118's state) |
| `email transport: MISCONFIGURED — falling back to logging` | the config cannot possibly send (no host, no `From`, `xoauth2` with no token endpoint) |
| `email transport: logging (explicit) — email is not delivered` | you chose `kind: logging` |

Then stage a real effect and watch it drain:

```jsonc
// POST /mcp/call — as an admin holding mcp:invite.create:call
{ "tool": "invite.create", "args": {
    "email": "you+lbtest@example.com", "role": "member", "locale": "en", "ts": 1
} }
```

Within a relay tick (~2s) you should see `INFO email sent to=you+lbtest@example.com ws=acme` and the
mail should arrive. Check the spam folder before concluding it did not — see the deliverability note.

## 4. Diagnose a failure

Read the outbox row. It now records **why**:

```jsonc
// POST /mcp/call — as an admin holding mcp:outbox.status:call
{ "tool": "outbox.status", "args": {} }
```

- `status: "failed"` with a `last_error` and a future `next_attempt_ts` — a **transient** failure
  (connection, timeout, `4xx`, throttle, a token-endpoint outage). The outbox is backing off and will
  retry; nothing is lost.
- `status: "dead-lettered"` with `attempts: 1` — a **permanent** failure the transport refused to retry.
  The `last_error` names it, and it needs you, not time.

| `last_error` | what to do |
| --- | --- |
| `no credential at secret path 'mail/…'` | the path is a typo, or the secret is `private` (seal it at `workspace` visibility) |
| `smtp auth failed (535): …` | wrong password, or the provider needs XOAUTH2 rather than a password |
| `mail: token refresh failed (400): invalid_grant` | the refresh token was revoked or is wrong — redo the consent ceremony |
| `smtp 550: …` / `postmark … (code 300 or 406)` | bad or inactive recipient address |
| `smtp: server does not advertise STARTTLS` | wrong port, or the relay wants `implicit` TLS (465) |
| `smtp tls: …` | certificate/hostname mismatch — fix the cert or the host name; do NOT reach for `allow_invalid_certs` outside a test |
| `email target: payload missing workspace` | an enqueue-side bug: the effect payload must carry its workspace (never guessed) |
| `smtp: session exceeded the 30s timeout` | the relay is hanging; raise `LB_MAIL_TIMEOUT_SECS` only if the relay is legitimately slow |

**Credential values never appear in these strings.** The transport redacts the secret and its base64
SASL encodings out of every error, because a rejecting relay will happily quote your `AUTH` line back at
you. If you ever see credential material in an outbox row or a log, that is a bug worth a debugging
entry.

## Rotation

Rotate by overwriting the sealed secret (`secret.set` on the same path, as its owner). No restart, no
redeploy: the value is read per send. Rotating an OAuth **refresh token** also invalidates the cached
access token automatically — the cache is keyed by a digest of the refresh token, so a new seal cannot
keep using a bearer minted from the old grant.

Changing the *relay* (host/port/kind) is boot config today, so it needs a restart. A stored,
admin-editable transport record is a named follow-up.

## What is deliberately not here

- **Receiving mail** — the IMAP/inbound half is `docs/scope/inbox-outbox/mail-source-scope.md`.
- **Bounce/complaint webhooks and a suppression list** — they arrive over `POST /hooks` and want their
  own table.
- **DKIM signing in core** — the relay or provider signs with the domain's key; signing in-process would
  mean custody of a domain private key for no gain.
- **Browser OAuth consent** — v1 takes a refresh token you obtained out of band (step 1).
- **Per-workspace sending identities** — the transport is node-level in v1.
- **An `email.send` verb** — see the top of this doc.

## Related

- Scope: `docs/scope/inbox-outbox/email-transport-scope.md` · `outbox-scope.md` ·
  `push-target-scope.md` (the sibling target) · `mail-source-scope.md` (the receive half)
- Session: `docs/sessions/inbox-outbox/email-transport-session.md`
- Code: `rust/crates/mail/` (the transport) · `rust/crates/host/src/outbox/provider_smtp.rs` ·
  `provider_postmark.rs` · `email_target.rs` · `rust/node/src/mail.rs` (the boot selector)
