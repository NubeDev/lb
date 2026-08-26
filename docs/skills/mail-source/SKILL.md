---
name: mail-source
description: >-
  Watch an IMAP mailbox and turn arriving email into workspace data — the raw message and every
  attachment stored as assets, matching files decoded into series samples, and an item in the lb
  inbox. Use when a task says "email your data in", "import a mailbox", "the monthly meter/invoice
  CSV arrives by email", "poll IMAP", "attachment to ingest", "NEM12", "set up a mail source",
  "the mailbox imports nothing", "why did that sender get rejected", or "the same email imported
  twice". Covers registering a source, the credential ceremony (app password vs OAuth2 refresh
  token for Gmail / Microsoft 365), the sender allowlist, the attachment/decoder policy and the
  format registry, verifying with mail.source.check, reading a poll pass, and diagnosing an import
  that produced no series.
---

# Watch a mailbox (email → assets + series + the inbox)

A **mail source** is a registered IMAP mailbox that the node polls on a cadence. Each new message
becomes, in this order:

1. the **raw RFC 822 message** as a workspace asset — stored *first*, before anything parses it;
2. every **attachment** as its own asset (byte-identical);
3. matching attachments **decoded into series samples** through the normal `ingest.write` path;
4. one **inbox item** on the source's channel, whose `meta` carries the from/subject/asset ids/series
   so a UI can render the arrival and link to everything it produced.

It is the receive half of [`email-transport`](../email-transport/SKILL.md), and it shares the same
credential posture: **the record stores a secret *path*, never a value.**

> **Everything here is admin-tier**, and deliberately so. A mail source is an *external ingress*:
> anyone who can email the address can put documents and series data in front of the workspace's
> agents. The sender allowlist is the containment; the admin gate is who gets to set it.

---

## 1. Seal the credential first

The source names a path; the poller resolves it at fetch time in the source's own workspace.

```bash
mcp secret.set '{
  "path": "mail/meter-mailbox",
  "value": "<the mailbox password or OAuth refresh token>",
  "visibility": "workspace"
}'
```

`visibility: "workspace"` is **required**: the poller runs as the node (`node:mail`), not as the
admin who sealed it, and a `Private` secret is unreadable to it.

The `secret:mail/*:write` grant is in the `workspace-admin` bundle. (It was not, briefly — see
[the debug entry](../../debugging/inbox-outbox/mail-source-admin-cannot-seal-its-own-credential.md);
if `secret.set` answers `denied` on a `mail/…` path, that grant is what is missing.)

Alternatively point the source at a node env var with `secretEnv` — the same
`sealed → env → unset` precedence the SMTP transport uses. Useful in dev, not for production.

### Gmail / Google Workspace, Microsoft 365

Neither accepts a plain account password over IMAP in the general case.

| situation | what to seal | `auth` |
| --- | --- | --- |
| Workspace/365 tenant with app passwords still enabled, 2FA on the account | the 16-character **app password** | `plain` |
| anything else, and the future | an OAuth2 **refresh token** | `xoauth2` |

The XOAUTH2 ceremony (obtained out-of-band; there is no browser consent flow in the product yet):

1. Create an OAuth client in the provider console (Google Cloud Console → *Credentials* → *OAuth
   client ID*, type **Desktop**). Note the client id and client secret.
2. Consent once for the scope `https://mail.google.com/` with `access_type=offline` and
   `prompt=consent` — the last two are what make the response contain a **refresh** token rather
   than only an access token, which is the step people miss.
3. Seal the refresh token at your `secretPath`, and the client secret at `oauth.clientSecretPath`.
4. Register with `auth: "xoauth2"` and the oauth block below.

The node exchanges the refresh token for an access token at fetch time and caches it (~1h, keyed by
a digest of the refresh token — so rotating the seal invalidates the cached bearer). A rejected
bearer is treated as **transient**: the next pass mints a new one rather than parking a healthy
mailbox.

---

## 2. Register the source

```bash
mcp mail.source.register '{
  "id": "meter-data",
  "name": "Meter data mailbox",
  "host": "imap.gmail.com",
  "port": 993,
  "tls": "implicit",
  "mailbox": "INBOX",
  "username": "alerts@nube-io.com",
  "auth": "plain",
  "secretPath": "mail/meter-mailbox",
  "channel": "mail",
  "pollSeconds": 60,
  "allowSenders": ["@example.com"],
  "attachments": {
    "storeBytes": true,
    "ingest": true,
    "format": "auto",
    "extensions": ["csv"],
    "seriesPrefix": "nem12.",
    "offsetMinutes": 600
  }
}'
```

The arguments **are** the source object — flat, not nested under a `source` key.

| field | notes |
| --- | --- |
| `tls` | `implicit` (993, what every hosted provider uses) or `none` (a trusted LAN server). **STARTTLS on 143 is not supported** and is refused rather than silently downgraded. |
| `auth` | `plain` \| `login` \| `xoauth2`. `none` is refused — a mailbox needs credentials. |
| `allowSenders` | **an array.** Exact addresses (`data@example.com`) or domains (`@example.com`, `example.com`, `*@example.com`). A domain rule is an *exact* domain match — `@example.com` does not admit `evil-example.com`. **Empty admits every sender**, which is right for a dedicated mailbox and wrong for a shared one. |
| `pollSeconds` | minimum 15. Polling faster gets a real mailbox rate-limited or locked; the floor is enforced at registration *and* at tick time. |
| `channel` | the inbox channel arriving mail lands on (default `mail`). |
| `oauth` | for `auth: "xoauth2"`: `{tokenEndpoint, clientId, clientSecretPath}` — e.g. `https://oauth2.googleapis.com/token`. |

**Re-registering the same id keeps its cursor, counters and owner** and replaces only the
configuration — so fixing a typo in the host name does not re-import the whole mailbox.

### The attachment policy

`storeBytes` and `ingest` are independent switches. A workspace that only wants the numbers turns
the first off; one that receives PDFs it cannot decode keeps them with the second off.

`format` is `auto` (identify from the bytes) or a named format id. Ask the node what it knows:

```bash
mcp mail.formats '{}'
# {"formats":[{"id":"nem12","description":"AEMO NEM12 interval meter data (100/200/300 records)"},
#             {"id":"csv-grid","description":"CSV with a leading timestamp column and one series per remaining column"}]}
```

`offsetMinutes` is how far ahead of UTC the file's wall-clock timestamps are — **600 for NEM12**
(NEM time is UTC+10, no DST, and the file says so nowhere). Get this wrong and a month of data
lands an hour or ten out; the check is in step 4.

`extensions` filters what is offered to a decoder (empty = try everything). It saves work; it is
not a security control.

---

## 3. Verify before you wait

```bash
mcp mail.source.check '{"id": "meter-data"}'
```

This opens a real IMAP session, fetches **one** message, and imports nothing — the cursor is
untouched, no asset, no ledger row.

```json
{"check": {
  "endpoint": "imap://imap.gmail.com:993/INBOX (implicit)",
  "uidValidity": 1787709288,
  "hasNew": true,
  "newest": {"uid": 1, "from": "data@example.com", "subject": "NEM12 interval data",
             "attachments": ["ZZZZ035361_nem12.csv"], "senderAllowed": true}
}}
```

`senderAllowed: false` here is **the most common reason a correctly-configured source imports
nothing.** Check it before you go looking at anything else.

Common failures:

| message | meaning |
| --- | --- |
| `no credential at secret path 'mail/…'` | nothing sealed there, or it was sealed `private` instead of `workspace` |
| `imap login: … AUTHENTICATIONFAILED` | wrong password, or the provider wants an app password / OAuth (see §1) |
| `imap: tls handshake …` | permanent, and never retried in the clear — check the host name and `tls` mode |
| `imap: tls mode 'starttls' is not supported` | use `implicit` on 993 |
| `mailbox '…' reports no UIDVALIDITY` | the server does not support UIDs; there is no durable place to resume from |

---

## 4. Watch it import

The reactor ticks every 15s and polls each source on its own cadence. An idle mailbox is silent;
anything that happened is logged:

```
mail source imported ws=acme source=meter-data imported=1 duplicates=0 rejected=0
                     failed=0 samples=21120 series=4 more=false
```

Or drive one pass by hand (this one **does** import and advance the cursor):

```bash
mcp mail.source.poll '{"id": "meter-data"}'
```

Reading the counters:

| counter | meaning |
| --- | --- |
| `imported` | new messages that became assets + (maybe) series + an inbox item |
| `duplicates` | already in the import ledger — a re-delivery, or a re-read after a UIDVALIDITY bump. **Expected and harmless.** |
| `rejected` | the sender was not on the allowlist. Nothing stored; the decision is ledgered so it is never re-made. |
| `failed` | the raw message IS stored, normalization did not finish. Read `notes` on the ledger row. |
| `more` | the mailbox held more than one batch past the cursor — the next tick continues |

Then check what landed:

```bash
mcp inbox.list '{"channel": "mail"}'      # the arrivals, with meta.series / meta.attachments
mcp series.stats '{"series": "nem12.ZZZZ035361.B1"}'
```

**Sanity-check the timezone here**, once, on real data. Read a local day bucketed hourly and look at
the shape:

```bash
mcp series.read '{"series":"nem12.ZZZZ035361.B1","mode":"buckets",
                  "from":<local midnight, epoch ms>,"to":<+24h>,"width_ms":3600000,"method":"sum"}'
```

A solar export channel should peak around midday and be flat overnight. If the peak is at 02:00, or
everything is one interval out, `offsetMinutes` is wrong — fix it and delete the ledger rows to
re-import (see §6).

---

## 5. What arrives, and where

| thing | where it goes |
| --- | --- |
| the raw message | asset `mail-{source}-{key}-raw`, mime `message/rfc822` |
| attachment *n* | asset `mail-{source}-{key}-att{n}`, byte-identical |
| decoded samples | series `{seriesPrefix}{decoder's name}`, producer `node:mail/{source}` |
| the notification | inbox item `mail-{source}-{key}` on `channel`, author `node:mail` |

`{key}` is a digest of the message's `Message-ID` (or of its bytes when it has none), which is what
makes every one of these idempotent: re-importing the same message upserts the same rows.

Samples carry the file's own dimensions (`nmi`, `uom`, `meterSerial`, `intervalMinutes`, `quality`
for NEM12) **and** provenance (`mailSource`, `mailFrom`, `mailMessage`, `mailAttachment`) as tags —
so "why is this series here?" is answerable months later without opening the mailbox. The file's own
dimensions win a name collision; a caller cannot relabel a meter's unit by misconfiguring a source.

### Two sources must not write the same series

Each source writes under its own producer (`node:mail/{source}`), and ingest's dedup identity is
`(series, producer, seq)` — so two sources importing the *same* meter with the *same* `seriesPrefix`
produce **two rows per instant**, and a `sum` over that series double-counts. That is the data
plane behaving correctly (two producers of one series are two independent streams); it is an
operator misconfiguration.

`series.stats` is how you spot it: more than one entry under `producers` for a series that should
have one source. Fix it by giving the sources distinct `seriesPrefix` values, or by deleting the one
that should not be there (`series.samples.delete` scoped to its producer).

---

## 6. Operating it

```bash
mcp mail.source.list '{}'                              # the roster: cursor, counters, lastError
mcp mail.source.pause '{"id": "meter-data"}'           # the kill switch — keeps everything, stops polling
mcp mail.source.pause '{"id": "meter-data", "paused": false}'
mcp mail.source.delete '{"id": "meter-data"}'          # deletes the SUBSCRIPTION only
```

**Delete does not cascade.** The import ledger, the assets, the items and the series survive — they
are the workspace's data, not the source's, and dropping the ledger would mean re-registering the
same mailbox re-imports everything it had already seen.

**Rotating a credential needs no restart** — reseal the same path; the next pass picks it up.

**To re-import a message after fixing a decoder or an `offsetMinutes`**: delete its row from the
`mail_import` table and rewind the source's cursor below its UID. The ledger is what says "already
handled"; removing the row is what makes the message eligible again.

---

## What this deliberately does not do

- **No `mail.source.import` verb** that takes raw bytes. The import path is reachable only by a
  message actually arriving in a registered mailbox, under a narrow node principal — a verb letting
  a caller hand the platform arbitrary bytes to "import as mail" would be a way to write assets,
  inbox items and series while holding none of those caps.
- **No routing or tagging policy.** "Invoices from X get tag Y" is your configuration — a rule
  reacting to the arrival, or an extension verb. Core knows RFC 5322; it does not know your business.
- **No threading.** Messages import flat; `In-Reply-To` is parsed and available for a later view.
- **No HTML fidelity.** The body is best-effort text; the raw message asset is the escape hatch.
- **One poller per source is convention, not a lease.** Two nodes polling one source both import;
  the ledger makes that idempotent, so the cost is wasted work, not duplicate data.

## Related

- [`email-transport`](../email-transport/SKILL.md) — the send half, same credential posture.
- [`ingest-series`](../ingest-series/SKILL.md) — what happens to the samples once they land.
- Scope: `docs/scope/inbox-outbox/mail-source-scope.md` · Session:
  `docs/sessions/inbox-outbox/mail-source-session.md`
