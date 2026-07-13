# Inbox/outbox scope — mail source (inbound email as a generic producer)

Status: scope (the ask). Promotes to `public/inbox-outbox/` once shipped.

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
