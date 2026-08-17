# Inbox-outbox scope — attachments, recipient fan-out, and payload-authored words on the email target

Status: **BUILT 2026-08-17**, branch `feat/report-email-delivery`, unreleased (needs the next
`node-v*` tag). Consumer: `NubeIO/rubix-ai` (scheduled report PDFs).

> Read with: [`email-transport-scope.md`](./email-transport-scope.md) (the transport this sits on
> top of — SMTP/Postmark behind `EmailProvider`, selected by name at the binary boundary),
> [`outbox-scope.md`](./outbox-scope.md) (the must-deliver substrate + the `Target` trait),
> `../reports/report-builder-scope.md` (whose "scheduled / emailed report runs" non-goal this
> closes), README §3 rules 3/6/10.

## The problem

The transport shipped and works: an effect addressed to `"email"` reaches a real SMTP relay. But the
**target** in front of it could only ever deliver one thing — an invite.

Concretely, `EmailTarget::deliver` read `payload.email` (exactly one address), rendered subject and
body from the `invite.email.*` catalog keys under a hardcoded `action == "send_invite"` arm, had no
notion of an attachment, and fell through to the literal subject `"Notification"` with an **empty
body** for every other action. `EmailMessage` had no `attachments` field at all, even though
`lb_mail::MailMessage` — one layer down — has had one since the transport landed.

So the platform could send exactly one kind of mail, and the deferred half of the reports scope had
nowhere to land. `rubix-ai` renders a report to PDF on a schedule, stores it as a workspace asset,
and enqueues `{target: "email", action: "report", payload: {assetId, recipients, subject, body}}` —
a row that the target could not read, could not address, and could not attach. It would have failed
permanently on the first field it looked for.

**This is not a report feature.** Attaching a file and mailing several people are things any producer
wants; putting either behind an `if action == "report"` would repeat the mistake being fixed.

## Goals

- **Fan out to every recipient the effect names.** `payload.recipients: []` alongside the legacy
  single `payload.email`, de-duplicated, **one message per address** — so each has its own delivery
  outcome, its own `Message-ID`, and its own row in the delivered ledger. A retry after a partial
  failure re-sends only to whoever missed out.
- **Attachments, by reference.** `payload.assetId` (or an `attachments: [{assetId, filename, mime}]`
  array) resolved at delivery time from the workspace asset store into
  `EmailAttachment { filename, mime, bytes }`, carried on `EmailMessage`, and passed through by
  **both** shipped providers — SMTP into `MailMessage.attachments`, Postmark into its base64
  `Attachments` array.
- **Payload-authored words, catalog as the fallback.** A producer that wrote a subject gets exactly
  that subject. A producer that wrote none falls back to `{action}.email.subject|body|body_html` in
  the effect's locale — so a new emailed action ships translated copy by adding **catalog keys, not
  a match arm**.
- **One responsibility per file.** The target file was already at 397 of its 400 allowed lines, so
  the payload, the words, the attachments, and the dev providers each moved to their own file.

## Non-goals

- **No `email.send` verb.** Still deliberately absent (it is a spam cannon). The only way to reach
  the transport remains staging a durable, capability-gated, retried outbox effect.
- **No inline images / `cid:` references.** An attachment is an attachment; inline HTML imagery is a
  separate ask with its own MIME shape.
- **No size policy of its own.** Attachment bytes are already bounded by `MAX_ASSET_BYTES` at the
  point the asset was written. A relay that refuses an oversized message reports it as a permanent
  delivery error, which is the honest answer.
- **No per-recipient personalisation.** Every recipient of one effect gets the same words. A mail
  merge is a different feature.

## Decisions worth stating

**1. The bytes travel by reference, not in the row.** An outbox effect is durable queue state. A
multi-megabyte PDF inlined into it would be re-written on every retry, dragged along by every
`outbox.status` read, and kept for ever in a dead letter. The effect names an asset; the target reads
it once per delivery, before any send.

**2. The asset read is workspace-walled and principal-free — the same posture as the credential
read.** The relay reactor is host machinery with no user principal to carry a
`store:asset/{id}:read` capability, so `email_attachment.rs` goes through the raw
`lb_assets::get_asset(store, ws, id)` with the workspace taken from the **effect payload and never
defaulted** (rule 6). This widens no user authority: a ws-B effect can only name a ws-B asset, and the
effect could only have been staged by a principal already holding `mcp:outbox.enqueue:call` there.
It is exactly how `provider_smtp.rs` resolves its SMTP password via `lb_secrets::get_workspace`.

**3. A missing asset is a PERMANENT failure.** An id that does not resolve now will not resolve on
the fifth retry. More importantly: a report email that arrives with no report is worse than a visible
dead letter, because it looks like it worked. Attachments are resolved **before the first send**, so
an effect with a bad reference mails nobody rather than mailing the first recipient and then giving
up.

**4. Dedup moved from per-effect to per-(effect, recipient).** The delivered ledger was already keyed
`(target, dedup_key, recipient)` — it just had one recipient. Marking each address as it succeeds is
what makes a partial fan-out safe to retry. The irreducible at-least-once window (a crash between the
relay's accept and the marker's commit) is unchanged.

**5. The catalog prefix is derived from the action, with one named legacy exception.** `send_invite`
maps to the `invite.` prefix because those keys ship in every translated catalog already, and
renaming them would break every translation written against them. Everything else uses its own action
name. That single mapping is spelled out in `email_content.rs` rather than left implicit.

**6. The last-resort subject is the action name, not `"Notification"`.** An action with no catalog
copy and no authored words previously produced a constant subject and an empty body. Both are
useless to a recipient and to an operator reading a mailbox trying to work out which producer sent
it — and an empty body with an attachment is a well-known spam signal.

## Files

New, under `rust/crates/host/src/outbox/`:

| file | owns |
|---|---|
| `email_payload.rs` | the opaque payload string → a typed `EmailPayload`; the workspace refusal |
| `email_content.rs` | action + payload → subject / text / html (authored wins, then catalog) |
| `email_attachment.rs` | `EmailAttachment`; `assetId` → bytes, workspace-walled |
| `email_provider_dev.rs` | `LoggingEmailProvider` + `RecordingEmailProvider` (moved out) |

Changed: `email_target.rs` (the fan-out loop + per-recipient dedup; `EmailMessage.attachments`),
`provider_smtp.rs` (map attachments into `MailMessage`), `provider_postmark.rs` (Postmark
`Attachments`), `outbox/mod.rs` + `lib.rs` (re-exports, `EmailAttachment` added).

## Back-compatibility

The invite path is untouched behaviourally and its tests are unchanged: a `{email, workspace, token,
locale}` payload with `action: "send_invite"` still renders both catalog body halves in the effect's
locale, still carries the same `Message-ID`, and still refuses to guess a workspace. `EmailMessage`
gained a field; any embedder implementing `EmailProvider` compiles unchanged unless it constructed an
`EmailMessage` literally without `..Default::default()`.

## What the consumer had to change

The reports scope predicted "when the transport lands, **zero** rubix-ai code changes — the payload
shape is final now". That was nearly true, and the exception is worth recording: the payload carried
no `workspace`. The target refuses an effect without one (rule 6 — the workspace selects the SMTP
credential and the asset namespace, so guessing it would resolve another tenant's secret), so every
scheduled report would have dead-lettered with `payload missing workspace`. One field, added
downstream.

## Regression tests

`lb-host` unit (`cargo test -p lb-host --lib outbox::`, 27 green):

- invites still deliver both body halves with the stable `Message-ID`;
- a payload with no workspace fails **permanently** and sends nothing;
- a payload naming no recipient fails permanently;
- a repeated address is sent to once;
- a report effect fans out to every recipient **with the PDF attached**;
- a retry after a partial fan-out only sends to whoever missed out, and a third pass is a no-op;
- an effect naming a missing asset mails nobody;
- authored words win over the catalog; an action with no copy gets a traceable subject;
- an unknown mime gets no extension rather than a wrong one.

## Related

- `email-transport-scope.md` (the transport underneath) · `outbox-scope.md` ·
  `push-target-scope.md` (the sibling target) · `../auth-caps/invites-scope.md` ·
  `../reports/report-builder-scope.md` (the non-goal this closes).
- Consumer: `NubeIO/rubix-ai` `docs/scope/frontend/reports/report-server-pdf-schedule-scope.md`.
- Operator runbook: `docs/skills/email-transport/SKILL.md`.
