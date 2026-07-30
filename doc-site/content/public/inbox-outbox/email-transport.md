# Email transport

Outgoing email is an **outbox effect**, not a verb: a caller (today `invite.create`) stages a durable
effect with `target: "email"`, and the relay reactor delivers it on its tick through the node's
configured mail transport. Durability, retry, backoff and at-least-once belong to the outbox; the
subject and body come from the i18n catalog in the recipient's locale; the transport's only job is
putting bytes on a wire.

Configure it and email works. Don't, and the node says so — loudly.

## Choosing a transport

The transport is selected **by name** in boot config, so a product host gets working email from
configuration alone rather than by implementing a trait.

| `kind` | what it does |
| --- | --- |
| `smtp` | Submits to a real relay: implicit TLS or STARTTLS, `PLAIN`/`LOGIN`/`XOAUTH2`/no auth. The portable floor — an internal LAN relay, a hosted relay, SES-over-SMTP, Gmail, Microsoft 365. |
| `postmark` | Posts to the Postmark transactional API. The deliverability path: the provider signs with your domain's key, keeps the sending reputation, and reports bounces. |
| `logging` | Logs the message and delivers nothing. Dev only, and only when you ask for it by name. |

A host with its own mailer still injects an `EmailProvider` directly; that escape hatch wins over the
config.

```bash
LB_MAIL_KIND=smtp
LB_MAIL_HOST=smtp.example.com
LB_MAIL_PORT=587
LB_MAIL_TLS=starttls              # implicit | starttls | none
LB_MAIL_AUTH=plain                # none | plain | login | xoauth2
LB_MAIL_USER=reports@acme.com
LB_MAIL_SECRET_PATH=mail/smtp-password
LB_MAIL_FROM='Acme Reports <reports@acme.com>'
```

## Credentials are paths, not values

Configuration carries a **secrets path** (or an env-var *name*); the value is resolved **at send time,
in the effect's own workspace**. Three things follow from that:

- a ws-A effect can never resolve ws-B's credential — the workspace comes from the effect payload and is
  never guessed;
- rotating a credential is one `secret.set` on the same path — no restart, no redeploy;
- nothing holds a credential between sends, and the config struct is safe to log.

Seal the secret at **workspace** visibility: the relay reactor is host machinery with no user principal
to carry a `secret:<path>:get` capability, and a `private` secret is deliberately not host-resolvable.

## Gmail and Microsoft 365 mean OAuth, not a password

Google has been switching off app passwords in increments for years; Microsoft removed basic auth from
Exchange Online outright. So `XOAUTH2` is first-class, including **token refresh** — an access token
lives about an hour, which means a transport without refresh would work for exactly one hour after
setup. You seal a refresh token (obtained once, out of band) and the node mints access tokens as needed,
caching each until shortly before it expires.

Rotating the sealed refresh token invalidates the cached access token automatically: the cache is keyed
by a digest of the refresh token, so a new grant can never keep using a bearer minted from the old one.

## The message

An HTML body is always sent as `multipart/alternative` with a plain-text part — a single-part HTML mail
reads badly in a text client and scores badly with spam filters. Both halves come from the catalog
(`invite.email.body` and `invite.email.body_html`) so a translator sees both; when a catalog has no HTML
key the mail goes out text-only, and when it has no text half one is generated from the HTML (keeping
link destinations inline, so a text-only reader can still accept an invite).

Each message carries a stable `Message-ID` derived from the effect's idempotency key — identical across
retries, so a receiving MTA can collapse a duplicate the sender could not know it had sent.

## Delivery outcomes are honest

The transport classifies every failure, and the outbox acts on it:

- **transient** — connection refused, timeout, `4xx`, throttle, a token-endpoint outage: the effect stays
  schedulable and the outbox's backoff owns the retry;
- **permanent** — `5xx`, a bad recipient, a revoked OAuth grant, a TLS verification failure, a
  misconfigured transport: the effect is parked immediately, with no retry storm.

Either way **the reason is recorded on the effect row**, so a stuck email is diagnosable instead of
"failed 5 times". A TLS verification failure is permanent on purpose and never downgraded to cleartext,
and a `starttls` transport that meets a server which does not advertise the upgrade aborts rather than
sending credentials in the clear.

Credential values never reach an error string or a log line: a rejecting relay will happily quote your
`AUTH` line back, so the secret and its base64 SASL encodings are redacted out of every message the
transport produces.

Retry dedup has two layers — a per-`(effect, recipient)` delivered marker so a partial failure re-sends
only what failed, and the stable `Message-ID` above. Neither makes it exactly-once: a crash between "the
relay accepted the message" and "the marker is committed" can still duplicate. That window is stated,
not papered over.

## Deliverability is not a code problem

Sending direct-to-MX from a node gets filed as spam regardless of how correct the client is — no
SPF/DKIM/DMARC alignment, no IP reputation, cloud and residential IP blocks. Send through a relay or a
provider API, with the sending domain's DNS set up. That, not the SMTP client, is what determines
whether mail arrives.

## Not included

Receiving mail (the IMAP half), bounce/complaint webhooks, a suppression list, DKIM signing in core,
browser OAuth consent, per-workspace sending identities, and a generic `email.send` verb — the last
deliberately: a granted capability to mail arbitrary addresses is a spam cannon, and it needs a rate
limit and probably a recipient allowlist first.

## Operator runbook

`docs/skills/email-transport/SKILL.md` — the credential ceremony (app password vs OAuth refresh token,
which paths to seal), the full config reference, how to verify with a real send, and a table mapping each
`last_error` to what to do about it.
