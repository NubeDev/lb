# Auth-caps scope — invites (token onboarding for people who don't exist yet)

Status: scope (the ask). Promotes to `public/auth-caps/` once shipped.

> Read with: `global-identity-scope.md` (**shipped** — identities + `membership.add`, whose
> goals already name "an admin (or a self-join link) adds a global identity"; this scope
> builds the *link* half), `login-hardening-scope.md` (the credential seam invites must land
> behind, not before), `authz-grants-scope.md` (the role/team a joiner receives),
> `../../scope/inbox-outbox/outbox-scope.md` (delivery), `admin-crud-scope.md` (revoke).

`membership.add(ws, sub)` presumes the person already has a global identity and the admin
knows their `sub`. Real onboarding is the other way round: an admin knows an **email**, the
person has **no account**, and the join must carry **which role/team/grants** they receive
— a parent invited to a childcare workspace, a technician invited to a site, a teammate
invited to a project. We want a first-class **invite**: a durable, revocable, single-use
token record that an admin mints, an outbox target delivers, and a **pre-auth accept
surface** redeems into `identity` + `membership` + grants — atomically, with the caps live
on first login.

## Goals

- **`invite` record** (workspace-scoped): email, role and/or team to grant on join,
  optional opaque `payload` for the caller (e.g. an extension's guardian-record id — the
  core never interprets it; rule 10), single-use token hash, expiry, status
  (`pending|accepted|revoked|expired`), audit (who minted, when redeemed, from where).
- **Verbs:** `invite.create / list / revoke / resend` (admin, capability-gated) and the
  redemption path `invite.accept(token, credential)` — the one **pre-auth** verb.
- **Accept = atomic onboarding:** verify token (unexpired, unredeemed) → create-or-match
  the global identity (by verified email) → set the credential via the `CredentialCheck`
  seam → `membership.add` → apply the invite's role/team grants → mark redeemed → mint the
  session. Partial failure leaves the invite `pending` (idempotent retry), never a
  half-joined member.
- **Delivery via the outbox:** `invite.create` enqueues a must-deliver effect to a generic
  `email` target (one trait, one named file — the sanctioned external). No SMTP in core.
  **Wiring contract (explicit):** the core only *stages* the effect. Delivery happens
  **only if the product host registers `EmailTarget` (with its `EmailProvider` impl) with
  `spawn_relay_reactors` at boot** — a host that skips this silently accumulates pending
  `invite:*` effects and no mail ever leaves. This is by design (core names no provider,
  rule 10), but it is the deploying host's responsibility, not an implicit default.
- **Fresh caps immediately:** accept mints the token *after* grants are applied — no
  "re-login to pick up caps" on first entry (the known publish/install friction must not
  apply to a person's first minute).

## Non-goals

- **Not SSO/OIDC** — the credential set at accept goes through `login-hardening`'s seam;
  when OIDC lands, accept binds the IdP subject instead. Design must not preclude it.
- **Not open self-signup.** Every join is minted by someone holding `invite:create`.
  A public "join this workspace" link is a *later* mode on the same record (`max_uses`).
- **Not the email service.** One `Deliver` trait; providers are config.

## Intent / approach

A small state machine over records + one pre-auth route. The **only** unauthenticated
surface is `POST /public/invite/accept` (+ a `GET` that shows workspace branding — the
deferred `GET /public/branding/{ws}` finally has its driver). Pre-auth routes are gated
hard: single-use, hashed token (store the hash, mail the secret — the api-keys pattern),
expiry, per-IP/per-record rate limit, constant-time compare.

**Rejected alternative — "admin creates user + password out-of-band":** it's what exists
implicitly today; it can't carry role/team intent, leaks credentials through side channels,
and dies the moment logins are hardened. **Rejected — bare signed URL, no record:** not
revocable, not auditable, can't be single-use.

## How it fits the core

- **Tenancy / isolation:** invites are rows in the target workspace; accept joins exactly
  that workspace. The token embeds nothing but entropy — ws/role live server-side.
- **Capabilities:** `mcp:invite.create/list/revoke/resend:call` gate the admin verbs
  (deny-tested); accept is pre-auth by design and gated by the token itself.
  **Resolved (no-escalation, review 2026-07-11):** role grants carried by an invite follow
  the **`grants.assign` precedent** — a `role:<name>` grant is *exempt* from a
  holds-cap check on the minter, because the role's caps were bounded at `roles.define`
  time and `mcp:invite.create:call` is the same authority tier as `mcp:grants.assign:call`
  (anyone who can create invites could equally `grants.assign` the role after a plain
  `membership.add`). *Rejected alternative:* "invite may only grant roles the minter
  holds" (this scope's original wording) — it is stricter than `grants.assign`, so it
  would not close any real escalation (the minter just assigns the role post-join) while
  breaking the common case of a delegated onboarding admin who isn't themselves a
  `workspace-admin`. If grants.assign ever gains a minter-bound check, invites must
  inherit it in the same change.
- **Placement:** identity is hub-authoritative (global-identity), so invite mint/accept are
  hub verbs; role decides mounting, no `if cloud`.
- **MCP surface (§6.1):** CRUD-ish (`create/revoke/resend`) + `list` (filter by status).
  Live feed N/A (roster refetch covers it). Batch: `invite.create_many` deferred until a
  real caller (an import job) exists.
- **Data / motion:** `invite` records in SurrealDB; delivery is a must-deliver outbox
  effect (never raw pub/sub); the email itself is the one external behind a trait.
- **Secrets:** provider API key via `secrets/` mediation; invite token = hash-at-rest.
- **SDK/WIT impact:** none required — extensions mint invites through the normal granted
  MCP verbs, passing their correlation in `payload`.

## Example flow

1. Admin (or the care extension on the admin's behalf) calls `invite.create{email:
   "sam@…", role: "member", team: "guardians", payload: "<guardian-record-id>"}`.
2. Outbox delivers the email; Sam taps the link on his phone.
3. `GET /public/invite/…` shows the workspace-branded accept screen; Sam sets a credential;
   `invite.accept` runs the atomic chain; token minted; he lands signed-in.
4. The inviting extension observes the acceptance (invite record now carries `sub`; a bus
   event `invite.accepted` fires) and binds its own records via `payload`.
5. Admin revokes a pending invite → the link is dead (`revoked`, 410 on accept).

## Testing plan

Mandatory: **capability-deny** (member without `invite:create` → 403; role grants follow
the `grants.assign` precedent — see the resolved no-escalation decision above, tested as
"admin with `invite.create` may invite with any defined role"), **workspace isolation**
(accept joins only the minting ws; invite of ws A invisible from B). Plus: expiry,
double-redeem (second → 410, and the loser must be rejected **before any credential
mutation** — the winner's password stays intact), revoke-then-accept, resend rotates the
token **and refreshes the expiry** (old token dead, new token works past the original
expiry), existing-identity takeover prevention (missing/wrong `current_secret` → 409 with
the original credential untouched; correct one binds without a duplicate identity),
accept-then-first-call makes a **real cap-gated call** with the minted token (no
re-login), rate-limit on the public route (429 past the per-IP window, other clients
unaffected), and the invite email driven through the **real outbox relay** to the
recording provider. All against a real booted node; the email trait's test impl records
sends.

## Risks & hard problems

- **A pre-auth route is attack surface** — this scope must land *with* login-hardening,
  and the public route ships rate-limited from day one.
- **Email-match account takeover** — matching an existing identity by unverified email is
  the classic hole; the invite email is treated as verified *for that address only*, and an
  existing identity must authenticate before the membership binds.
- **Atomicity of accept** — five steps, one outcome. **Resolved (review 2026-07-11):**
  the redemption is **claimed first** via a store-level conditional `CREATE` (an
  `invite_claim` row — first write binds, every racer gets `Conflict`, the
  `lb_store::create` first-settle primitive), and only the claim winner runs the
  credential/membership mutations; a post-claim failure releases the claim back to
  `pending` for idempotent retry. Previously the credential write ran *before* a plain
  read-modify-write mark-accepted, so two concurrent accepts could both pass and the
  loser overwrote the winner's password
  (`docs/debugging/auth-caps/invite-accept-credential-race.md`).

## Open questions

- ✅ `invite.accepted` rides polling for v1 (bus event deferred — the extension observes
  acceptance by checking `status == accepted` + `accepted_by` on the invite record).
- ✅ `max_uses > 1` deferred (strictly single-use first; a "join link" mode is a later additive
  field on the same record).
- ✅ Accept UI lives in the minimal shell (`frontend/minimal-shell-scope.md`) — a themed client
  route (`/accept?token=…`) calling `POST /public/invite/accept`.
- ❓ **Takeover check is workspace-local while identity is global.** The credential record
  is stored per-workspace (`(ws, sub)` — the login-hardening seam), so accept's
  `current_secret` check verifies against the *inviting workspace's* credential row. An
  identity that holds a credential only in another workspace looks `Absent` here and binds
  without proof — the cross-ws half of the email-match takeover hole. Do NOT restructure
  credentials inside this scope; resolve it with the credential-placement question in
  `login-hardening-scope.md` (global credential vs per-ws credential + a global check at
  the pre-auth surfaces). Until then, treat an invite as trusting its own workspace's
  credential state only.

## Related

`global-identity-scope.md` · `login-hardening-scope.md` · `authz-grants-scope.md` ·
`api-keys-scope.md` (hash-at-rest pattern) · `../inbox-outbox/outbox-scope.md` ·
`../frontend/minimal-shell-scope.md` · first consumer: `cc-app`
`docs/scope/care/enrollment-invites-scope.md`.
