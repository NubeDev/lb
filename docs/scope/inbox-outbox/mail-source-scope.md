# Inbox/outbox scope — mail source (inbound email as a generic producer)

Status: **SHIPPED (v1) 2026-08-26** — branch `feat/mail-source-ingest`, unreleased (needs the next
`node-v*` tag). Session:
[`sessions/inbox-outbox/mail-source-session.md`](../../sessions/inbox-outbox/mail-source-session.md).
Public: `doc-site/content/public/inbox-outbox/mail-source.md`. Operator runbook:
[`skills/mail-source/SKILL.md`](../../skills/mail-source/SKILL.md).
**Read the "Shipped (v1)" section at the bottom before building on this** — it records what landed,
the answered open questions, and the gaps that are still owed.

> Read with: `inbox-outbox-scope.md` (the normalized-item posture this extends to an
> external source), `../files/media-scope.md` (attachments + raw-message storage),
> `../document-store/doc-extraction-scope.md` (attachment/body extraction),
> `../secrets/` (credential custody), `../auth-caps/api-keys-scope.md` (the machine
> principal the poller runs as), `../jobs/jobs-scope.md`, README §3 rule 10.

Email is the most common way documents actually arrive at a business — reports, invoices,
statements, attachments — and the platform has no inbound path for it: no mailbox
credentials custody, no poll cursor, no message→record normalization. Any product wanting
"email your docs in" would hand-roll IMAP, secrets handling, and dedup — the exact
machinery that should exist once. We want a **generic mail source in core**: register a
mailbox (credentials sealed in secrets), a durable poll job with a cursor, and each new
message normalized into the platform's existing surfaces — the **raw message stored as
media** (source of truth), the body as a markdown **doc**, attachments as media handed to
the extraction seam. Core knows RFC 5322, never products: what a message *means* (its
tags, its routing) is the caller's configuration, applied downstream by rules/extensions.

## Goals

- **`mail_source` record + verbs:** `mail.source.register / update / list / delete /
  pause` — host/protocol/folder + a **secrets path** for credentials (the record stores
  names, never values). v1 protocol: **IMAP**; the fetch side is one `MailFetch` trait so
  Gmail API / JMAP adapters slot in later (one contract, many providers — the gateway
  pattern).
- **A durable poll job with a cursor:** per source, UIDVALIDITY/UID-based; a restarted
  node resumes from the cursor, never re-imports. Poll cadence is source config.
- **Normalization, receive-only:** per new message — (1) raw RFC 822 bytes → **media**
  (the immutable original, checksum-deduped); (2) text/HTML body → markdown **doc** titled
  from the subject, with standard metadata (from, to, date, message-id) and the platform
  tag `email`; (3) each attachment → **media**, edge-linked to the message doc, optionally
  pushed through `docs.extract`. Dedup on Message-ID (fallback: content hash) via the
  source's ledger — a re-delivered message is a no-op.
- **A narrow machine principal:** each source's poll job runs as an api-key principal
  granted exactly {media put, doc put, extract call} in one workspace. Deny path: the
  poller can never read the corpus back.
- **Motion on arrival:** a ws-scoped bus event per imported message (the inbox posture:
  state persisted first, then the live echo) so UIs/rules/agents can react without polling.

## Non-goals (v1)

- **Sending email.** Outbound is the outbox's job (an SMTP/provider `Target` like
  `push-target-scope.md`) — a separate ask; this scope is strictly receive.
- **Threading/conversation model.** Messages import flat; `In-Reply-To` lands in metadata
  so a later view can thread without re-import.
- **Routing/tagging policy in core.** "Invoices from X get tag Y" is caller configuration —
  a rules-engine reaction to the arrival event or an extension verb, never a core schema
  (rule 10).
- **HTML fidelity.** Body conversion is best-effort markdown; the raw message media is the
  fidelity escape hatch, same posture as extraction.
- **OAuth flows in the browser.** v1 credentials are app-passwords/tokens sealed in
  secrets; an interactive OAuth mint is a gateway/UI follow-up (the record already stores
  a secrets path, so the custody model doesn't change).

## Intent / approach

A host service (`crates/host/src/mail/` — source CRUD, the poll job, normalization) over a
pure `crates/mail` fetch/parse layer (`MailFetch` trait + IMAP impl + RFC 822 parsing,
fixture-testable offline).

*Rejected: product-side pollers* — N products × IMAP × credential handling × dedup, and
the platform's own agent/rules can never assume mail arrives uniformly.
*Rejected: mail → inbox `Item`s only* — the chat-shaped inbox item loses the attachments
and the durable document body; mail is document-shaped, so it lands on the docs/media
surfaces, with the bus event covering the "notify/triage" need (an inbox-item projection
can be added by a consumer if a triage UI wants it).
*Rejected: webhook-only ingestion* (reuse `POST /hooks`) — works for providers that push,
but the common case (plain IMAP mailbox) needs a poller with custody + cursor; the
webhook route stays available as a second front door for push providers.

## How it fits the core

- **Tenancy / isolation:** a `mail_source` belongs to one workspace; everything it imports
  lands in that workspace. Two workspaces polling the same mailbox are two sources, two
  cursors — wasteful but walled.
- **Capabilities:** `mcp:mail.source.*:call` admin-gated (it grants an external ingress
  and spends storage); the poll job runs under the source's dedicated api-key principal
  (narrow, revocable — instant kill switch per source).
- **Placement:** either — an edge node can poll a mailbox offline-tolerantly (cursor
  resumes after a gap); cloud placement is just config.
- **MCP surface (§6.1):** CRUD on sources; the import itself is a recurring **job** (long,
  network-bound, resumable) — no synchronous "poll now" verb beyond `mail.source.check`
  (one bounded fetch, for setup validation). No new read verbs: imported mail is read
  through the normal doc/media verbs.
- **Data (SurrealDB):** `mail_source` records (config + cursor + secrets *path*), a
  per-source import ledger `{message_id_hash, doc_id, ts}`; everything else is existing
  tables (docs, media, relations). State only.
- **Bus (Zenoh):** one fire-and-forget arrival event per message (motion; the ledger is
  the durable truth — a missed event is healed by listing docs). Nothing must-deliver, so
  no outbox involvement on the receive path.
- **Sync / authority:** the source record + cursor are **node-local by authority** (two
  nodes must not both poll one source); imported docs/media sync normally. The
  one-poller-per-source rule is the cursor record's node claim.
- **Secrets:** the load-bearing one — credentials sealed in `lb-secrets`, resolved only
  inside the poll job at fetch time; the source record, lists, and logs carry the path
  name only. This mirrors the agent model-key posture exactly.
- **No mocks:** parsing is fixture-tested on real `.eml` files; the IMAP server is the one
  sanctioned external fake — one `MailFetch` impl in one named file (`fetch/fixture.rs`)
  replaying fixture messages, per testing-scope §0.
- **SDK/WIT impact:** none.

## Example flow

1. An admin registers `mail.source.register { host, folder: INBOX, secrets_path:
   "mail/reports-mailbox", workspace }`; `mail.source.check` fetches one message to prove
   credentials; a dedicated api-key principal is minted for the source.
2. The poll job wakes on cadence, resolves credentials from secrets, fetches UIDs past the
   cursor: one new message — a monthly report, PDF attached.
3. It stores the raw `.eml` as media; writes the body as a doc (title = subject, tagged
   `email`, metadata from/date/message-id); stores the PDF as media edge-linked to the
   doc; calls `docs.extract` on it; advances the cursor; emits the arrival event.
4. A workspace rule reacts to the event and applies the caller's own tags (its business,
   not core's). The embeddings reactor picks up both new docs; the corpus grew without a
   human touching anything.
5. The provider re-delivers the same message a day later: Message-ID hits the ledger —
   no-op.

## Testing plan

Mandatory categories:

- **Workspace isolation:** two sources in two workspaces over the fixture server; imports
  never cross; ws B cannot list ws A's sources.
- **Capability deny:** non-admin denied `mail.source.register`; the source's api-key
  principal denied doc reads and every verb outside its grant.
- **Offline/sync:** kill the node mid-batch → restart resumes from the cursor with no
  duplicate docs (ledger + UID cursor together).

Key cases: `.eml` fixture matrix (plain, HTML-only, multipart, 3 attachments, missing
Message-ID → hash fallback, 8-bit subject encodings); re-delivery no-op; UIDVALIDITY
change → cursor reset without re-import (ledger catches); credential rotation via secrets
without touching the source record; `pause` actually stops the job.

## Risks & hard problems

- **Email is a swamp** (encodings, malformed MIME, HTML soup). Containment: the raw
  message is always stored first — normalization can fail per-message into a visible
  `failed` ledger state and be re-run after a parser fix, never losing mail.
- **Credential custody is the whole game.** A leaked mailbox password is worse than most
  platform bugs; the secrets-path-only posture must survive logs, error messages, and the
  source `list` verb (test it explicitly).
- **Duplicate pollers** after a node split-brain double-import; the ledger makes it
  idempotent, but the node-claim on the cursor needs a liveness story (bus liveliness
  token, same as extension health).
- **Mailbox as attack surface:** anyone who can email the address can inject documents
  into the corpus (and thence into agent context — exfil/poisoning). Mitigations are
  caller policy (sender allowlists as source config, quarantine-until-rule-approves via
  visibility) — but the scope must ship allowlist config in v1, not defer it.
- **Provider drift** (Gmail IMAP deprecations, OAuth-only mandates): the `MailFetch` trait
  is the hedge; the Gmail-API adapter is the likely first follow-up.

## Skill doc

Yes — `docs/skills/mail-source/SKILL.md`: registering a mailbox (secrets first),
validating with `check`, reading import results, pausing/re-running, the allowlist knob.

## Open questions

- IMAP crate choice (`async-imap` vs `imap` + executor bridge) and TLS posture.
- Body→markdown converter: reuse the extraction seam's HTML extractor (one converter,
  two callers) or a mail-specific pass for quoted-reply trimming?
- Sender allowlist semantics: reject at fetch, or import-but-quarantine (visibility
  `Private` to the source principal until released)? Quarantine is safer for audit.
- Does the arrival event carry the doc id only, or a summary payload (subject/from) for
  cheap triage UIs?
- Cadence bounds and per-source quotas (a runaway mailbox shouldn't eat a workspace's
  storage) — config defaults?
- One-poller node claim: bus liveliness vs a lease field on the cursor record?

## Related

- `inbox-outbox-scope.md`, `outbox-scope.md` (the send-side sibling this deliberately
  isn't), `push-target-scope.md` (the Target pattern an SMTP sender would follow).
- `../document-store/doc-extraction-scope.md` + `../embeddings/embeddings-scope.md` — the
  pipeline imported mail flows into (mail → doc → vector → search, zero product code).
- `../secrets/`, `../auth-caps/api-keys-scope.md`, `../jobs/jobs-scope.md`.
- README §3 rule 10, §6.5/§6.6, §6.10, §6.12.


---

## Shipped (v1) — 2026-08-26

What landed, in the order it matters:

- **`lb-mail`'s `fetch/` half** (`rust/crates/mail/src/fetch/`) over `async-imap` (feature
  `runtime-tokio`) on a `tokio-rustls` socket, with `mail-parser` promoted from a dev-dependency to
  a normal one — parsing a fetched message is production behaviour, not test scaffolding.
  `MailFetch` is the one contract; `ImapFetch` is the v1 impl; `MailboxCursor` carries
  `(uidValidity, lastUid)` together because a UID is only unique within its generation.
- **A file-decoder registry in `lb-ingest`** (`decode/`): `decode(format_id, input, options)` over an
  **opaque** format id resolved through `FORMATS`. Two formats shipped — `nem12` (the AEMO interval
  format the ask arrived in) and `csv-grid` (a timestamp column, one series per remaining column) —
  plus `detect()`, which reads the bytes before the extension. This is the "new service for
  converting an email attachment to an ingest" the ask named, built as a seam rather than a feature:
  a new format is a new file in that folder, not a change to any caller.
- **The `crates/host/src/mail/` service**: `mail.source.register / list / get / update / delete /
  pause / check / poll` + `mail.formats`, admin-tier, plus `spawn_mail_reactors` — the driver,
  shipped *with* the mechanism this time.
- **`lb_inbox::Item` gained `meta: Option<Value>`**, answering this scope's sibling open question
  (see below). All 77 existing `Item::new` call sites are unchanged.
- **`secret:mail/*:write`** added to the `workspace-admin` bundle, because an admin who may register
  a mailbox must be able to seal its password
  ([debug entry](../../debugging/inbox-outbox/mail-source-admin-cannot-seal-its-own-credential.md)).

### Decisions worth stating

**1. `seq` is derived from the sample's timestamp (`ts_ms / 1000`), never from file order.** The
obvious choice — 0, 1, 2 in file order — is wrong in a way that only appears in production: a
*second* file covering an overlapping period (a corrected re-issue, a monthly export repeating the
last week) would reuse `seq 0..N` for *different* instants and silently overwrite real data. Deriving
from the instant makes re-imports exact upserts and overlapping files converge. Verified live: the
same 21,120-sample export emailed twice produced two inbox items (two genuine messages) and left
`raw_count: 5280` on the channel under test — exactly one file's worth.

**2. The ledger, not the cursor, is the correctness guarantee.** The cursor is an optimization, and
three ordinary things defeat it: a UIDVALIDITY bump, a crash before the cursor write, and a provider
re-delivering at a new UID. All three are caught by a per-source ledger keyed on the `Message-ID`
(hashed; a content digest when absent) — which is also why the cursor may be reset freely.

**3. A rejected sender is ledgered, and stores nothing else.** The scope asked whether to reject at
fetch or quarantine for audit. Neither, exactly: nothing of the message is stored (no asset, no item,
no series — a mailbox is spam-reachable and storage is the workspace's), but the *decision* is a
ledger row carrying from/subject/uid. That is the audit trail without the storage cost, and it is
what stops a later widening of the allowlist backfilling a surprise.

**4. The importer principal is NOT the flow reactor's.** `node:mail` holds exactly
`{assets.put_asset + store:asset/*:write, inbox.record, ingest.write}`. Reusing
`flows::reactor_caps()` would have been one line and would have handed an untrusted external ingress
`store:*:read`/`write` and `store.query`. The property under test is what is missing from the bundle.

**5. Order is the containment strategy.** Raw message → asset FIRST; ledger row LAST, on every
outcome. A crash mid-import re-imports, harmlessly, because every id is derived from the message key.
The opposite order (ledger first) would mark a message imported that is not.

**6. STARTTLS on IMAP 143 is refused, not half-built.** Hosted IMAP is implicit TLS on 993, and
`async-imap` offers no safe seam to re-wrap its buffered stream mid-session. A refusal naming the
working mode beats an untested upgrade whose failure mode is a mailbox password on a cleartext socket.

### Open questions, answered

1. **IMAP crate + TLS posture** → `async-imap` with `runtime-tokio` (no second executor), TLS by
   `tokio-rustls` with the workspace's existing `ring` provider and `webpki-roots` — no system CA
   bundle, no second crypto backend.
2. **Body→markdown converter** → **neither, for now.** The body is best-effort text (plain, else
   HTML) and rides on the inbox item as a preview; the raw message asset is the fidelity escape
   hatch. Mail arrived here **document-shaped in the scope's telling and data-shaped in practice** —
   the first real consumer wanted the *attachment*, not the prose. A markdown doc per message is a
   cheap addition on top of the stored raw asset when a consumer asks for one.
3. **Sender allowlist semantics** → reject-and-ledger (decision 3 above). Empty admits everyone,
   stated plainly.
4. **Does the arrival event carry the doc id only, or a summary?** → a summary. There is no separate
   bus event in v1; the **inbox item is the arrival notification**, and its `meta` carries subject,
   from, asset ids, series, and per-attachment decode results. The scope rejected "mail → inbox items
   only" because a chat-shaped item loses the attachments — correct, and the answer was not to skip
   the item but to give `Item` a `meta`. A ws-scoped bus event remains a small addition for a live UI.
5. **Cadence bounds** → `pollSeconds` ≥ 15, enforced at registration *and* at tick time (a record
   written before the floor existed cannot bypass it). Per-source storage quotas are not built;
   bounded work is (`MAIL_BATCH` messages/pass, `MAX_ATTACHMENTS`/message, `INGEST_CHUNK`
   samples/write, `DecodeOptions::max_samples`/file).
6. **One-poller node claim** → **still open.** Convention, not a lease. Two nodes polling one source
   both import; the ledger makes it idempotent, so the failure is wasted work rather than duplicate
   data. A lease field on the source record is the likely answer.

### Deltas from the scope as written

- **Docs/media, not docs-then-media.** The scope's normalization was "(1) raw → media, (2) body →
  markdown doc, (3) attachments → media + `docs.extract`". What shipped is (1) raw → **asset**,
  (2) attachments → **assets**, (3) attachments → **samples**, (4) the item. `lb_assets`' asset
  surface is the byte store this needed; `docs.extract` is not wired in, because the first consumer's
  attachment was a data file, not a document. Calling `docs.extract` on a stored attachment is a
  small, additive step and the asset is already there for it.
- **No dedicated api-key principal per source.** The scope wanted each poll job to run as a minted
  api-key principal. It runs as a per-workspace system principal (`node:mail`) with the same
  *narrowness*, which is the property that mattered; a per-source key would add a revocable kill
  switch per source, which `mail.source.pause` already provides.

### Regression tests to keep green

`lb-mail`: `imap_fetch_test` (10, against a real IMAP server on a real socket), 34 unit.
`lb-ingest`: `decode_test` (13, against the real 163 KB four-channel NEM12 export), 19 unit.
`lb-host`: `mail_import_test` (11 — the full import, cap-deny with no record written, workspace
isolation across two mailboxes, re-read/re-delivery dedup, the allowlist, a file no decoder handles,
`check` importing nothing, re-register keeping its cursor), 18 unit.

Five security/behaviour-critical tests were **revert-checked** (each fails against deliberately
broken code): the capability gate, the import ledger, the `UID n:*` cursor filter, the
re-register cursor preservation, and the sender allowlist.

### Gaps still owed (stated, not hidden)

- **No real-TLS IMAP test.** The in-test server is plaintext; implicit TLS is covered by construction
  and error mapping. The same gap the send half still carries.
- **XOAUTH2 is built and unit-tested but not driven against a live Google/Microsoft tenant.** The
  SASL frame is asserted from what a real server received; the refresh exchange has its own tests.
  The consent ceremony is documented in the skill doc; the browser flow is not automated.
- **The one-poller-per-source claim** (open question 6).
- **Nothing prunes the `mail_import` ledger.** One small row per message, for ever — the same
  retention question `inbox-outbox-scope.md` already has open for channel history and delivered
  effects.
- **No bus event on arrival** (open question 4) — a live UI polls `inbox.list` today.
- **`docs.extract` is not called** on stored attachments (see the deltas above).
