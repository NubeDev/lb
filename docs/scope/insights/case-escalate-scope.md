# Insights scope — case escalate: hand a case to a colleague and email them

Status: **shipped** (2026-11-13), branch `feat/case-escalate`. Extends
[`case-plane-scope.md`](case-plane-scope.md) (the verb table and the wave-2 ask).

The Insights "Action" panel offers an email type *Internal escalation*. Until now every email on the
case plane went to a **party** — the external roster — through `case.request.send`: a tokenised reply
link, a nudge ladder, `waiting_on`. An internal escalation is a different act. It picks from the SAME
subjects as Assign (`case.assignees`: the caller, their teams, their teammates — `user:…` / `team:…`),
hands the case to that subject, and emails the person (or every member of the team) the typed subject
and message with a link that opens the case in the app. The recipient signs in, so there is nothing to
reply through.

## Goals

- One verb, `case.escalate`, that **assigns** the case and **emails** the subject's people.
- The recipients' addresses are resolved **server-side** from the identity table; no caller supplies
  or sees an address.
- Every refusal (unknown subject, no address, no grant, out-of-scope case) changes **nothing**.
- No new grant on customer nodes.

## Non-goals

- **No token, no nudge ladder, no `waiting_on`.** Internal users sign in; the ball is with us, not a
  contractor, and chasing a colleague is a comment, not a reminder rung.
- No new notification channel: the mail rides the existing `email` outbox target.
- No per-recipient locale yet: the mail renders in the catalog default (`en`), as the request link does.

## Contract

MCP tool `case.escalate`:

| arg | type | |
|---|---|---|
| `case_id` | string | required |
| `to` | string | required — `user:<sub>` or `team:<name>`, validated by `validate_assignee` |
| `subject` | string | optional — blank is absent |
| `message` | string | optional — blank is absent |
| `ts` | u64 | the caller's logical clock, as every case verb (normalised; host clock if absent) |

Reply: `{ "case": <the case after the assign>, "recipients": <number of mails staged> }`. Never an
address.

Behaviour, in order — every check happens before the first write:

1. `authorize_tool(…, "case.request.send")` and `authorize_tool(…, "case.workflow")`.
2. The case must exist and be visible (entity scope): out of scope reads exactly like missing.
3. `to` through `validate_assignee` (same opacity contract as `case.assign`).
4. Resolve addresses: `user:x` → its identity's login email, else the bare sub when it is itself an
   address; `team:x` → each `user:` member edge (both id spellings) that is a **live member of the
   workspace**, each resolved the same way. Lower-cased, de-duplicated; a member with no address is
   skipped (debug log). None at all ⇒ `bad input: no email address for <to>`, and the case is not
   reassigned.
5. Size-check the history row (`validate_event_size`).
6. Assign through `case_assign` — so the owner echo onto every member insight happens.
7. Stage one `email` effect per recipient, action `case_escalate`, through `lb_outbox::enqueue` with a
   justification row in `case_escalation` (the outbox has no effect-only write; the change row records
   `{case_id, to, by, ts}` — no address).
8. Append a `escalated` case event: actor = the caller, data
   `{ to, recipients: <count>, subject|null, message|null }`.

The words come from the catalog: `case_escalate.email.subject` / `.body` / `.body_html` (`en.mf`,
`es.mf`). Default subject: `Escalated to you: {title}{ at site}`; the typed subject wins. Body: the typed
message (else a short default naming the sender), then `Open the case:
{base_url}/#/t/{workspace}/insights?tab=cases&case={case_id}` (an `<a>` in HTML). The sender's words are
HTML-escaped in the HTML half (the `link_args` convention).

Effect id: `case-escalate:{case_id}:{ts}:{sha256(address)[..16]}` — deterministic per (escalation,
recipient), so a retry with the same `ts` re-stages the same effects and the email target's
per-recipient delivered ledger stops a second send. The address is hashed because an effect id is a
store key and appears in outbox listings.

## How it fits the core

- **Capabilities:** rides `mcp:case.request.send:call` through a `tool_gate.rs` alias — "may email
  people about a case" is exactly this authority, and a `mcp:case.escalate:call` would exist in no role
  bundle (Denied for everyone). The assign half also requires `case.workflow`, checked up front so a
  caller holding only the mail grant is refused before anything is written. Strictest wins.
- **Tenancy:** every read is workspace-scoped; a ws-B member is simply not a member here, so
  `validate_assignee` refuses them with the ordinary opaque message.
- **Privacy:** identities (and so login emails) are admin-only data (`identity.list` is admin). The
  reply carries a count, the history a count, the justification row no address; the address exists only
  inside the effect payload on its way into the mail.
- **Durability:** must-deliver mail goes through the outbox (rule: durability), never raw pub/sub.
- **Rule 10:** `email` is an opaque target string; no provider is named.

## Testing plan

`rust/crates/host/tests/case/escalate.rs` (in `case_suite`), real store, real relay, recording provider:

- a user escalation by a caller holding ONLY `case.request.send` + `case.workflow` (the positive gate
  proof) assigns, stages one mail to the identity's lower-cased email, and appends an `escalated` event
  with no `@` in it; the reply has no `@` either;
- a team escalation mails each live member once (both edge spellings dedup; a member with no address and
  a non-member edge are skipped);
- unknown subject, no address, missing either grant: refused, owner untouched, no event, nothing mailed;
- an out-of-scope case answers exactly like a missing one.

Unit: `tool_gate_tests::the_escalation_rides_the_ask_cap`; `email_content` renders the catalog copy and
the deep link; `escalate_recipients` normalises member edges.

## Risks

- **The deep link is the Rubix shell's route** (`#/t/<ws>/insights?tab=cases&case=`). It lives in the
  catalog, so a workspace can override the copy, but a shell that moves the route must move this line.
- **A retried call with a new `ts`** stages new effect ids and mails again — a retry must reuse its `ts`.

## Related

- [`case-plane-scope.md`](case-plane-scope.md) — the case verbs and `case.request.send`.
- [`insight-triage-scope.md`](insight-triage-scope.md) — `validate_assignee` and the opacity rule.
