# Mail source — email in

A **mail source** is a registered IMAP mailbox the node polls. It is the receive half of the
[email transport](./email-transport.md), and it closes the loop: the platform could already *send*
mail with attachments; now mail that arrives becomes workspace data.

Each new message becomes, in this order:

1. the **raw RFC 822 message**, stored as a workspace asset — *first*, before anything parses it;
2. every **attachment**, stored byte-identical as its own asset;
3. matching attachments **decoded into series samples** through the ordinary `ingest.write` path;
4. one **inbox item**, whose `meta` carries the from/subject/asset ids/series so a UI can render the
   arrival and link to everything it produced.

```
a 3rd party sends mail  →  IMAP mailbox
                               │  poll (per-source cadence)
                               ▼
                       raw RFC 822 octets  ──▶  asset
                               │  parse
                               ▼
         allowlist ─▶ attachments ─▶ assets  ─▶  decode  ─▶  ingest.write ─▶ series
                               │
                               ▼
                       inbox item (+ meta)
```

## Register a mailbox

Everything about a mail source is admin-tier, deliberately: it is an *external ingress*, and anyone
who can email the address can put data in front of the workspace's agents.

The credential is sealed by **path**; the record never holds a value.

```bash
mcp secret.set '{"path":"mail/meter-mailbox","value":"<app password>","visibility":"workspace"}'

mcp mail.source.register '{
  "id": "meter-data",
  "host": "imap.gmail.com", "port": 993, "tls": "implicit",
  "mailbox": "INBOX", "username": "alerts@example.com",
  "auth": "plain", "secretPath": "mail/meter-mailbox",
  "channel": "mail", "pollSeconds": 60,
  "allowSenders": ["@example.com"],
  "attachments": {
    "storeBytes": true, "ingest": true, "format": "auto",
    "extensions": ["csv"], "seriesPrefix": "nem12.", "offsetMinutes": 600
  }
}'
```

`visibility: "workspace"` on the secret is required — the poller runs as the node, not as the admin
who sealed it.

**Gmail and Microsoft 365 do not accept an account password over IMAP** in the general case. Use an
app password (`auth: "plain"`) where your tenant still allows them, or an OAuth2 refresh token
(`auth: "xoauth2"` plus an `oauth` block); the node exchanges it for an access token at fetch time
and caches it. `tls: "implicit"` on 993 is the only TLS mode — STARTTLS on 143 is refused rather
than silently downgraded.

## Verify before you wait

```bash
mcp mail.source.check '{"id": "meter-data"}'
```

Opens a real IMAP session, fetches one message, imports nothing. It reports the endpoint
(credential-free), the mailbox's `uidValidity`, and a peek at the newest message — including
`senderAllowed`, which is the most common reason a correctly-configured source imports nothing.

## The allowlist

`allowSenders` is the containment for "anyone who can email this address can inject data". An empty
list admits every sender, which is right for a dedicated mailbox and stated plainly rather than
being an implicit deny that makes a fresh source look broken.

Entries are exact addresses or domains. **A domain rule is an exact match**, never a suffix:
`@example.com` does not admit `evil-example.com`.

A rejected message stores nothing — no asset, no item, no series — but the *decision* is recorded, so
it is auditable and is never re-made when someone widens the list later.

## Attachments become series

The decoder is chosen by an opaque format id. Ask the node what it knows:

```bash
mcp mail.formats '{}'
```

| id | what |
| --- | --- |
| `nem12` | AEMO NEM12 interval metering. One series per `(NMI, suffix)`; values are period-ending; the meter's own dimensions become tags. |
| `csv-grid` | A timestamp column, then one series per remaining column, named by its header. |
| `auto` | Identify from the bytes (falling back to the extension). |

`offsetMinutes` is how far ahead of UTC the file's wall-clock timestamps are — **600 for NEM12**,
whose times are NEM time by specification and say so nowhere in the file. Where a file *does* state
its zone, the file wins.

Each source writes under its own producer (`node:mail/{source}`), and ingest's dedup identity is
`(series, producer, seq)` — so pointing two sources at the same meter with the same `seriesPrefix`
gives you two rows per instant and a `sum` that double-counts. `series.stats` shows more than one
entry under `producers` when that has happened.

Samples carry the file's own dimensions **and** their provenance (`mailSource`, `mailFrom`,
`mailMessage`, `mailAttachment`) as tags, so "why is this series here?" is answerable months later
without opening the mailbox. The file's dimensions win a name collision: a source's configuration
cannot relabel a meter's unit.

## Importing is idempotent, twice over

- **A cursor** remembers how far into the mailbox the poller has read (`UIDVALIDITY` + UID, always
  together — a UID is only unique within its generation).
- **An import ledger** records every message the source has handled, keyed on its `Message-ID` (or a
  digest of its bytes when it has none).

The cursor is the optimization; **the ledger is the correctness guarantee**, because three ordinary
things defeat a cursor: a mailbox renumbering (`UIDVALIDITY` bumps and everything re-reads), a crash
between the import and the cursor write, and a provider re-delivering a message at a new UID. All
three are no-ops.

Beneath both, every id a message produces — the asset ids, the item id, and each sample's dedup key —
is derived from the message, so a re-import upserts exactly the same rows. And because a sample's
dedup key comes from its **timestamp**, a *second* file covering an overlapping period converges on
the same rows instead of colliding with them: a corrected re-issue of a monthly export adds nothing
but corrections.

## Operating it

```bash
mcp mail.source.list '{}'                              # the roster: cursor, counters, lastError
mcp mail.source.poll '{"id": "meter-data"}'            # one pass now (this one imports)
mcp mail.source.pause '{"id": "meter-data"}'           # the kill switch
mcp mail.source.delete '{"id": "meter-data"}'          # deletes the SUBSCRIPTION only
```

A poll pass reports `imported / duplicates / rejected / failed / samples / series / more`.
`duplicates` is expected and harmless. `more` means the mailbox held more than one batch past the
cursor — the next tick continues.

**Delete does not cascade.** The ledger, assets, items and series survive: they are the workspace's
data, not the source's, and dropping the ledger would mean re-registering the same mailbox re-imports
everything it had already seen.

**Rotating a credential needs no restart** — reseal the same path.

## What it deliberately does not do

- **No verb takes raw bytes to "import as mail".** The import path is reachable only by a message
  actually arriving in a registered mailbox, under a node principal holding exactly four capabilities
  (write an asset, record an inbox item, write samples) — and notably *unable to read the corpus it
  writes into.*
- **No routing or tagging policy.** "Invoices from X get tag Y" is your configuration — a rule
  reacting to the arrival. Core knows RFC 5322; it does not know your business.
- **No threading, no HTML fidelity.** Messages import flat; the body is best-effort text and the raw
  message asset is the escape hatch.

See also: [email transport](./email-transport.md) (the send half) ·
[inbox and outbox](./inbox-outbox.md).
