# Session — the mail source: inbound email as a generic producer, and an attachment as an ingest

- Scope: [`scope/inbox-outbox/mail-source-scope.md`](../../scope/inbox-outbox/mail-source-scope.md)
  (the ask), with [`email-transport-scope.md`](../../scope/inbox-outbox/email-transport-scope.md)
  and [`email-attachments-and-fanout-scope.md`](../../scope/inbox-outbox/email-attachments-and-fanout-scope.md)
  as the already-shipped send half this closes the loop on.
- Date: 2026-08-26
- Branch: `feat/mail-source-ingest`
- Debug entries opened: [`mail-source-register-rejected-by-its-own-descriptor`](../../debugging/inbox-outbox/mail-source-register-rejected-by-its-own-descriptor.md),
  [`mail-source-admin-cannot-seal-its-own-credential`](../../debugging/inbox-outbox/mail-source-admin-cannot-seal-its-own-credential.md)
- Operator runbook: [`skills/mail-source/SKILL.md`](../../skills/mail-source/SKILL.md)

## The ask

> "make an inbox/outbox example working for gmail/email … an email is sent [with]
> `ZZZZ035361_nem12#0045575584#TCAUSTM.csv` and it's forwarded as an ingest and stored, so an email
> sent by a 3rd party, lb will get the email, process the email and attachment, and we have a new
> service for converting an email attachment to an ingest, and show it in the lb inbox."

Three halves, of which the platform had one:

| half | before this session |
|---|---|
| **send** an email, with an attachment, to several people | shipped (`EmailTarget` + SMTP/Postmark, #118 + the fan-out slice) |
| **receive** an email at all | **nothing.** `mail-source-scope.md` was a scope with no code; `lb-mail` had a `send/` folder and a comment promising a `fetch/` sibling |
| turn an attached **file** into series data | **nothing.** Every producer of a `Sample` did so by speaking JSON at `ingest.write`; a file had no path in |

## What shipped

### 1. `lb-mail`'s receive half (`rust/crates/mail/src/fetch/`)

`MailFetch` is the one contract (`fetch_since(cursor, limit) -> FetchBatch`); `ImapFetch` is the v1
implementation over `async-imap` on a `tokio-rustls` socket. `parse_message` turns RFC 822 octets
into a `ParsedMail` via `mail-parser`.

Three decisions worth keeping:

- **The mailbox is never mutated.** `EXAMINE` (read-only) *and* `BODY.PEEK[]`. Either alone would
  do; both are used because the failure — silently marking a human's mail read from under them — is
  invisible to us and very visible to them.
- **`UID FETCH n:*` is a trap** and the whole client is shaped around it. RFC 3501 guarantees the
  range matches *at least one* message, so an idle poll of a mailbox whose highest UID is 3, from a
  cursor at 3, gets **3** back. A poller that trusted the range would re-import the newest message
  on every tick, for ever. The range is a `UID SEARCH` hint and every returned UID is filtered
  against the cursor in code. `an_idle_poll_returns_nothing_even_though_the_server_matches_the_newest_message`
  is the test, and it fails the moment that filter is removed.
- **`parse_message` never fails.** The containment rule is that the raw message is stored first and
  normalization is allowed to be imperfect; a parser returning `Err` would tempt every caller into
  dropping the mail. A message that yields no text, no HTML and no attachment carries its own octets
  as its body.

**STARTTLS on 143 is refused, not half-built** — hosted IMAP is implicit TLS on 993, and
`async-imap` offers no safe seam to re-wrap its buffered stream mid-session. A stated gap with a
message naming the working mode beats an untested upgrade path whose failure mode is a mailbox
password on a cleartext socket.

### 2. A file-decoder registry in the data plane (`rust/crates/ingest/src/decode/`)

`decode(format_id, input, options) -> Decoded`. The format id is **opaque** — resolved through
`FORMATS`, never branched on by any caller — so a new format is a new file in that folder and one
row in the table, not a change to the mail source, the ingest verb, or the MCP surface. Two shipped:
`nem12` (AEMO interval metering) and `csv-grid` (a timestamp column, then one series per column).
`detect()` identifies from the bytes first, the extension second (a NEM12 file arrives named `.csv`
and is not a CSV in any useful sense).

**The `seq` decision is the load-bearing one.** `seq` is half of ingest's dedup identity
`(series, producer, seq)`. File-order (`0, 1, 2, …`) is wrong in a way that only appears in
production: the *same* file re-imported is fine, but a **second** file covering an overlapping
period — a corrected re-issue, a monthly export that repeats the last week — would reuse `seq 0..N`
for *different* instants and silently overwrite real data. Every decoder derives
**`seq = ts_ms / 1000`**, so the identity is a property of the instant: re-imports are exact
upserts, overlapping files converge, and `series.latest` (highest seq) still means newest.
Proven live below.

An error means *no* samples; a bad row means *fewer* samples and a warning. A month of interval data
with one unparseable cell imports the other 4,319 points.

### 3. The mail-source host service (`rust/crates/host/src/mail/`)

`mail.source.register / list / get / update / delete / pause / check / poll` + `mail.formats`, all
admin-tier, plus the reactor that makes any of it run.

- **The record holds names, never values** — `secretPath` / `secretEnv`. The credential is resolved
  per pass, in the source's own workspace, and dropped when the pass returns.
- **The importer is a deliberately narrow principal** (`node:mail`) holding exactly four things:
  `mcp:assets.put_asset:call` + `store:asset/*:write`, `mcp:inbox.record:call`,
  `mcp:ingest.write:call`. It was one line to reuse `flows::reactor_caps()` and it would have been
  wrong — that bundle carries `store:*:read`/`write` and `store.query`, i.e. an untrusted inbound
  path that could read the whole workspace. The property under test is what is *missing*:
  `the_importer_cannot_read_the_corpus_it_writes_into`.
- **Order is the containment strategy.** Raw message → asset FIRST, before anything parses it; the
  ledger row LAST, on every outcome including rejection and failure. A crash mid-import re-imports,
  which is harmless because every id (raw asset, attachment assets, inbox item, and each sample's
  `(series, seq)`) is derived from the message key.
- **The ledger, not the cursor, is the correctness guarantee.** The cursor is an optimization; a
  UIDVALIDITY bump, a crash before the cursor write, and a provider re-delivering at a new UID all
  defeat it, and the ledger catches all three because the key is the `Message-ID` (or a digest of the
  bytes when there is none), hashed so a hostile 4 KB header cannot become a 4 KB record key.
- **The sender allowlist ships in v1**, as the scope insisted — an empty list admits everyone
  (correct for a dedicated mailbox, and stated plainly rather than being an implicit deny that makes
  a fresh source look broken). A domain rule is an **exact** domain match, never a suffix: `@example.com`
  must not admit `evil-example.com`. A rejected message stores nothing but *is* ledgered, so the
  decision is auditable and never re-made when someone widens the list later.
- **The reactor is part of the slice, not a follow-up.** This platform has shipped the ingest drain,
  series retention, reminders, and flow cron triggers with the mechanism complete and the heartbeat
  missing. `spawn_mail_reactors` ticks every 15s and polls each source on its *own* cadence.

### 4. `lb_inbox::Item` finally has `meta`

The inbox-outbox scope left this open — "a `meta: Value` field on `Item`, or a typed per-source
extension record the item references? (Defer until a second source exists.)" Mail is that second
source. It is a field, because the alternative makes the *reader* — one inbox view rendering items
from every source — join against a table whose name it has to know, which is the per-source
knowledge the normalized `Item` exists to abolish. The rule that keeps it from becoming a dumping
ground: **nothing in the inbox ever reads inside it.** `Option` + `skip_serializing_if` left all 77
existing `Item::new` call sites byte-for-byte unchanged.

## The live drive — a real loop, on real sockets

The suite being green proved nothing about whether this works, so the whole loop was driven on a
running node. Every number below is from that run.

**The setup.** A real mail server (GreenMail: SMTP 3025, IMAP 3143) in Docker, and the lb node with
`LB_MAIL_KIND=smtp` pointed at it — so lb's own **outbox** is the "3rd party" that sends the mail
that lb's own **mail source** reads back. Real SMTP out, real IMAP in.

```
asset (163 KB NEM12 CSV) ─▶ outbox.enqueue {target:"email", assetId} ─▶ relay ─▶ SMTP:3025
                                                                                    │
                                       GreenMail mailbox alerts@nube-io.com ◀───────┘
                                                    │
                        mail reactor ─▶ IMAP:3143 ─▶ import ─▶ assets + series + inbox item
```

1. **The send.** `email sent to=alerts@nube-io.com host=127.0.0.1` — GreenMail logged the RCPT and
   auto-created the mailbox.
2. **`mail.source.check`** (imports nothing) reported the endpoint credential-free, the mailbox's
   `uidValidity`, and a peek at the message including `senderAllowed: true` — the single most common
   "it imports nothing" cause, visible before you commit to a poll.
3. **The import, with nobody calling anything:**
   ```
   mail source imported ws=acme source=meter-data imported=1 duplicates=0 rejected=0
                        failed=0 samples=21120 series=4 more=false
   ```
   21,120 samples = 220 `300` records × 96 fifteen-minute intervals, across
   `nem12.ZZZZ035361.{B1,E1,K1,Q1}`.
4. **The inbox item** carried the summary as its body and the payload in `meta`: subject, from,
   the raw-message asset id, the attachment (`163137` bytes, `assetId`, and
   `ingest: {format: "nem12", decoded: 21120, accepted: 21120, warnings: 0}`), and the four series.
5. **The data is real, and the timezone handling is right.** One local day of channel B1, bucketed
   hourly with `method: "sum"`:
   ```
   00:00  0        08:00  1.65      16:00  1.4
   01:00  0.025    09:00  5.5       17:00  0.05
   …                10:00  9.125    …
   07:00  0.15     12:00  14.825    23:00  0.025   → 74.1 kWh for the day
   ```
   That is a textbook solar-export curve — flat overnight, ramping from 08:00, peaking at midday,
   back to zero by 17:00. It is also an *independent* check on two decisions no unit test can
   really validate: if the NEM+10 offset or the period-ENDING convention were wrong, the peak would
   land at 02:00 or be shifted an interval. `series.latest` confirms the last sample at
   `2026-08-25T00:00+10:00` — interval 96 of 2026-08-24, period-ending, exactly as specified.
6. **An idle re-poll fetched 0** — the `n:*` trap defended on a real server.
7. **A re-issue converged instead of duplicating.** The same CSV was emailed again under a new
   subject and a new `Message-ID`. It imported as a genuinely new message (2 inbox items, correctly
   — they *are* two messages) and `series.stats` still reported **`raw_count: 5280`** for B1:
   55 days × 96 intervals, i.e. exactly one file's worth. The timestamp-derived `seq` decision,
   validated on live data.
8. **The allowlist held.** A message injected straight into GreenMail's SMTP from
   `stranger@spam.invalid` (with the same CSV attached, to make it worth stealing) came back
   `rejected: 1, imported: 0`. Inbox still 2 items; B1 still 5,280 samples; the source's roster
   showed `imported: 2, rejected: 1`. Nothing of it was stored, and it will never be re-evaluated.

**And the real thing.** The node was then booted against **AWS SES ap-southeast-2** on 587/STARTTLS
with the supplied credentials, and delivered a real message with the real attachment:

```
email sent to=ap@nube-io.com ws=acme host=email-smtp.ap-southeast-2.amazonaws.com
outbox relay delivered effects ws=acme delivered=1 failed=0 dead_lettered=0
```

SES needed `kind: "smtp"` and nothing else, as the transport scope's open question #1 predicted.

## What the live drive found that the suite could not

Both are written up as debug entries; both are the same shape — **a chokepoint no test crosses.**

1. **The verb and its own descriptor disagreed** (twice: the nested-vs-flat shape, and
   `allowSenders` declared `string` against a `Vec<String>` record). `tools::validate_args` runs in
   `call_tool_at_depth`, and every test calls the host fn or `call_mail_tool` directly. Fixed by
   making the descriptor the single contract.
2. **The admin who may register a mailbox could not seal its password.** `mcp:secret.set:call`
   clears the MCP gate; `lb_secrets::set` re-checks a per-path `secret:<path>:write`, the admin
   bundle named only `agent/*`, `federation/*`, `webhook/*`, and the wildcard is single-segment.
   Fixed by adding `secret:mail/*:write` to the admin bundle.

Neither is exotic. Both are the "shipped but unusable" family that `tool_gate.rs`'s alias table
already documents at length — one at the schema chokepoint, one at the inner resource gate.

## Tests

| suite | count | what it proves |
|---|---|---|
| `lb-mail` unit | 34 | cursor rebase/advance, the camelCase wire shape + snake_case alias, MIME parsing, address folding, the never-lose-the-mail floor |
| `lb-mail` `imap_fetch_test` | 10 | **against a real IMAP server on a real socket**: the `n:*` trap, `{len}` literal framing, EXAMINE + `BODY.PEEK[]` (read from the server's own command log), a UIDVALIDITY bump, a redacted permanent login failure, a silent server hitting the timeout, the exact XOAUTH2 SASL frame the server received |
| `lb-ingest` `decode_test` | 13 | **against the real 163 KB four-channel export**: detection, one series per channel, period-ending in NEM time, no seam at the day boundary, the meter's own dimensions, a re-issue landing on the same dedup keys, a broken row warning without losing the file, a `300` under a broken `200` never attributed to the previous meter |
| `lb-ingest` `decode` unit | 19 | calendar arithmetic, ISO/epoch-seconds/epoch-millis equivalence, an explicit zone beating the configured offset, the label-collision rule |
| `lb-host` `mail_import_test` | 11 | the full import against a real IMAP server, incl. the three mandatory categories |
| `lb-host` `mail::` unit | 12 | the narrow importer principal's deny surface, the allowlist's exact-domain rule, validation, the reactor's cadence floor |

**Revert-checked** (each fails against deliberately broken code, then restored):

1. `authorize_mail_source` gutted → the cap-deny test fails.
2. `already_imported` forced to `false` → all three dedup/rejection tests fail.
3. the cursor filter removed from `imap.rs` → the `n:*` test fails with `got [3]`.
4. `source.cursor = existing.cursor` removed from `register` → the re-register test fails.
5. `sender_allowed` forced to `true` → the allowlist test fails.

## Owed, and stated

- **STARTTLS on IMAP 143** is refused with a message naming the working modes. Implicit TLS (993)
  and plaintext are implemented.
- **No real-TLS IMAP test** — the in-test server is plaintext, matching the send half's own owed gap.
  Implicit TLS was exercised by hand against a real relay only on the send side.
- **XOAUTH2 is built and unit-tested** (the SASL frame is asserted from what the server received)
  but has not been driven against a live Google/Microsoft tenant — that needs a refresh token from
  the consent ceremony, which the skill doc documents and the browser flow does not yet automate.
- **One poller per source is by convention, not by lease.** Two nodes polling one source would both
  import; the ledger makes it idempotent, so the failure is wasted work rather than duplicate data.
  The node-claim/liveness story is the scope's open question and stays open.
- **No `mail.source.import` verb** taking raw bytes, deliberately: it would be a way to write
  assets, inbox items and series while holding none of those caps.
- **Nothing prunes the ledger.** It grows one small row per message, for ever — the same retention
  question the inbox-outbox scope already has open for channel history and delivered effects.
