# Rules scope — a rule raises a `needs:approval` item and an approval closes the loop

Status: SHIPPED (2026-07-04). Built per this ask; promoted to `public/rules/rules.md`. Session:
`sessions/rules/rules-approvals-session.md`. The Open questions below were resolved at build as
recommended (body-tag convention kept; a sibling `approval_release` reactor; effect-held-first
compound write; `defer` inert + `route` advisory in v1) — see the session doc.

A rule can already raise a plain inbox notice (`inbox.record`) and resolve one (`inbox.resolve`), but the
two ends do not yet form the **approval loop** the platform already uses for coding jobs: *raise an item a
human must sign off on → the human approves/rejects → an effect fires on approval*. We want a rule body to
**request an approval explicitly** — raise an item tagged `needs:approval`, addressed to a reviewer/team,
that stages a **gated effect** — and we want the existing resolution reactor to **fire that effect only
when the item is `Approved`** (and drop it on reject). This makes "a rule proposes, a human disposes" a
first-class, caller-gated pattern, reusing the exact `Item` + `Resolution` + reactor mechanism the coding
workflow already ships — no new approval primitive.

> Read with: `rules-messaging-scope.md` (the `inbox`/`outbox`/`channel` rhai handles this extends — the
> `inbox.record`/`inbox.resolve` verbs are already built), `rules-engine-scope.md` (the engine + the
> caller-gated seam model), `../inbox-outbox/` (the `Item` + the durable outbox), `../coding-workflow/`
> (the shipped `request_approval` → `resolve_approval` reactor this generalizes), `../jobs/` (the reactor
> driver), `../auth-caps/auth-caps-scope.md` (`caps::check`), README `§6.10` (inbox/outbox),
> `§3` (rules 3 state-vs-motion, 5 capability-first, 6 workspace wall, 7 MCP contract, 10 core-knows-no-ext).

---

## The confusion this resolves (read first)

Today three different things are easy to conflate — this scope names them and wires the missing bridge:

1. **`inbox.record`** raises a *plain notice* — a to-do that appears in the Inbox view. It is **not**
   "pending approval". Works today.
2. **An approval** is *not a separate verb*: it is an `Item` **tagged `needs:approval`** plus a sibling
   `Resolution` (`approved`/`rejected`/`deferred`) — the vision §5 finding, already how the coding workflow
   works (`crates/inbox/src/resolution.rs`, `host/src/workflow/request_approval.rs`). The tag currently
   rides as **text at the front of the item `body`** (e.g. `body = "needs:approval route:team:X …"`), read
   by the `resolve_approval` reactor.
3. **`outbox.enqueue`** stages a *must-deliver external effect* (a page/email/webhook the relay sends
   outside the system) — "pending **delivery**", not "pending **approval**". Works today.

The gap: **a rule cannot raise a true `needs:approval` item**, because the rule `inbox.record` handle takes
only `#{ channel, id, body }` and the `Item` shape has no structured tag/meta field — so a rule author would
have to hand-craft the magic `needs:approval …` body string, with nothing gating the *effect* on the
outcome. This scope closes that: a first-class `inbox.request_approval` rule verb that stages a **gated
effect**, and a generic resolution reactor that drains it on `Approved`.

## Goals

- A **rule verb `inbox.request_approval(#{ id, channel, body, route, on_approve })`** that raises a
  `needs:approval` item (author forced to the caller) AND durably stages the `on_approve` **effect** it
  should fire when approved — a normal outbox effect (`target`/`action`/`payload`), staged in a **held**
  state so the relay does not deliver it yet.
- The existing **`inbox.resolve(item_id, "approved"|"rejected"|"deferred")`** verb (already built,
  rules-messaging) is the *decision* half — a human (via the Inbox UI's approve/reject) or another rule.
- A **generic approval reactor** (extending the coding-workflow's `resolve_approval` into a
  domain-free driver, or a sibling): on a new `Resolution`, look up the held effect keyed by the item id
  and **release it to the outbox** (deliverable) on `Approved`, **discard** it on `Rejected`, **leave it
  held** on `Deferred`. Idempotent — replays never double-release.
- **Every action caller-gated** — `request_approval` re-runs `mcp:inbox.record:call` + the effect-stage
  cap; `resolve` re-runs `mcp:inbox.resolve:call`. A deny is opaque; a rule can request no approval and
  fire no effect its invoker couldn't (rules-messaging's caller-gated invariant).
- A **Playground example + skill snippet** so an author can copy "propose a change, gate an email on
  approval" and see it run end-to-end.

## Non-goals

- **A bespoke approval table or policy engine.** Approval stays `Item` tagged `needs:approval` + a
  `Resolution` (the shipped mechanism) — no new primitive, no rules-owned ACL. Multi-reviewer quorums,
  escalation timers, and delegation are out (a later policy scope, not v1).
- **A new authorization path.** The verbs reuse `inbox.record`/`inbox.resolve` + the outbox stage cap
  verbatim. If a rule can't do it via a direct MCP call, it can't do it via a verb.
- **Auto-approval by a rule as the reviewer.** A rule *may* call `inbox.resolve` (it's caller-gated), but
  the intent is human sign-off; a rule approving its own request is possible only if the caller holds the
  resolve cap, and is documented as a foot-gun, not a feature.
- **Structured item tags/meta as a general facility.** We add exactly the `needs:approval` + `route`
  addressing this loop needs (kept as the existing body-tag convention, or a minimal typed facet — see
  Open questions), not a general item-metadata subsystem.
- **Draining/relaying effects itself.** A rule *stages* the gated effect; the **existing outbox relay**
  delivers it once released. A rule never races the relay (rules-messaging Resolved decision).

## Intent / approach

**Reuse the shipped approval mechanism; add only the rule-facing verb + the release step — this is the
load-bearing choice.** The coding workflow already proves the shape: `request_approval` writes a
`needs:approval` item + a sidecar record (its `PrSpec`), and `resolve_approval` reads the `Resolution` and
acts on `Approved`. We generalize the sidecar from "a PrSpec" to "**a held outbox effect**", and the
reactor from "start a coding job" to "**release the held effect to the outbox**". A rule's
`inbox.request_approval` is then just: `inbox.record` the tagged item + `outbox.enqueue` the effect in a
**held** status (a new terminal-until-released state, additive to the outbox status enum). Everything routes
through the one MCP contract and the caller's caps (rules-messaging), so isolation + the opaque deny come
for free (rules 5/6/7), and the effect id is opaque data the reactor keys on (rule 10).

**The effect is held in the outbox, not invented anew (state vs motion, §3 rule 3).** The must-deliver
effect is durable outbox state from the moment it's staged; approval flips it from `held` → `pending`
(deliverable), rejection flips it to `discarded`. This keeps one durability contract (the outbox's
never-lost/never-double-sent invariant already covers it), survives a restart mid-approval, and needs no
second queue. The alternative — stage the effect only *after* approval, inside the reactor — was rejected:
it splits "what will happen on approval" across two places (the rule body says one thing; the reactor
constructs another), loses the effect if the node restarts between approval and reactor tick, and hides the
proposed effect from the reviewer. Staging it **held + visible** means the Inbox UI can show *exactly* what
approving will do.

**Rejected — a new `approval:*` cap + table.** Considered giving approvals their own capability grammar and
store table. Rejected: it duplicates the `Item`/`Resolution`/outbox trio for no new authority (the
`inbox.*` + outbox caps already gate every step), and it would make a rule reach a plane the UI/agent don't
— breaking the "one contract" property (rule 7). The tag-on-item convention is enough and already wired.

## How it fits the core

- **Tenancy / isolation:** the item, the resolution, and the held effect are all workspace-scoped records
  written through `call_tool` (workspace pinned from the caller's token, never script-set). A ws-B rule
  cannot request an approval into ws-A, and the ws-B reactor never sees a ws-A resolution/effect. Mandatory
  isolation test: a ws-B `request_approval` + approve never touches a ws-A effect.
- **Capabilities:** `request_approval` re-checks `mcp:inbox.record:call` **and** the outbox stage cap
  (`mcp:outbox.enqueue:call`); `resolve` re-checks `mcp:inbox.resolve:call`; the reactor releases under a
  **host/system** authority (a driver, like the coding-workflow reactor — not a user cap). Every grant has
  a deny test; the deny is opaque (rules-messaging).
- **Placement:** `either` (symmetric). The verbs are in-process bridges to `call_tool`; the reactor is a
  tick in node boot (like the other resolution/relay reactors — `spawn_*_reactors`). No `if cloud`.
- **MCP surface (§6.1 — judged):**
  - **Consumed:** `inbox.record`, `inbox.resolve`, `outbox.enqueue`/`outbox.status` (all shipped).
  - **CRUD:** the new **`inbox.request_approval`** rule verb (a compound write: item + held effect) — its
    own responsibility, its own file (`rules/src/verbs/inbox.rs` grows one method; the host stage of a
    *held* effect is one host fn). `resolve` is the update. No delete verb (a resolution is append-only;
    a rejected effect is discarded by the reactor, not user-deleted).
  - **Get / list:** `inbox.list(channel)` already lists `needs:approval` items; `outbox.status()` already
    surfaces held/pending/discarded — extend the status shape with the `held` bucket (uncharged reads).
  - **Live feed:** N/A inside a rule (bounded run). The Inbox UI already watches the channel for the item;
    the reactor reacts to the resolution via the existing driver tick, not a rule watch.
  - **Batch:** N/A — one approval per `request_approval` call; the per-run write meter bounds a loop.
- **Data (SurrealDB):** no new table. The item is a normal inbox `Item`; the resolution is the existing
  `Resolution`; the gated effect is an existing outbox effect with a new **`held`** status value (additive
  to the status enum). The `needs:approval` tag + `route` ride the existing body-tag convention (or a
  minimal typed facet — Open questions). Ids are deterministic (`now` + counter) so a re-run upserts.
- **Bus (Zenoh):** approval **motion** (the item landing, the released effect delivering) rides the
  existing channel/outbox paths. The reactor is state-driven (reads the resolution record), not a raw
  pub/sub listener — durability holds (§3).
- **Sync / authority:** the item, resolution, and held effect are authoritative on the hosting node like
  any record; the reactor releases node-locally. A saved rule that requests approvals survives a restart
  (it's a record); a held effect approved just before a crash is released on the next reactor tick
  (idempotent — the release checks the effect isn't already `pending`/`delivered`).
- **Secrets:** none. The verb carries an opaque effect payload; the relay resolves any target secret at
  delivery, unchanged.
- **SDK/WIT impact:** none. New rule verb + a host fn + a reactor arm + one additive outbox status value.
  No wasm/native ABI change.

## Example flow

A facilities analyst writes a rule that proposes a refund and gates the email on a manager's approval.

1. The rule runs inside `rules.run`, workspace pinned to `acme`, the caller's principal on every seam:
   ```
   let breach = source("series").last("1h").col("value").max();   // a real read
   if breach > 5.0 {
       // Raise an approval item AND stage the email it will send IF approved.
       inbox.request_approval(#{
           id: `refund-${breach}`,
           channel: "ops",
           body: `Refund proposed — cooler breached at ${breach}°C`,
           route: "team:managers",                                 // who must sign off
           on_approve: #{ target: "email", action: "send",         // the HELD effect
                          payload: #{ to: "ops@acme.io", subject: "Refund approved" } },
       });
   }
   ```
2. The verb builds two writes through `call_tool` under `caller ∩ grant`: an `inbox.record` of a
   `needs:approval route:team:managers` item, and an `outbox.enqueue` of the `email.send` effect in status
   **`held`** (keyed by the item id `refund-…`). Both charge the per-run write meter.
3. The Inbox UI shows the item awaiting approval, **including the proposed effect** (email to `ops@acme.io`)
   — the reviewer sees exactly what approving does.
4. A manager clicks **Approve** → `inbox.resolve("refund-…", "approved")` writes the `Resolution`.
5. The **approval reactor** tick sees the new `Approved` resolution, looks up the held effect by the item
   id, and flips it `held → pending`. The **existing outbox relay** then delivers the email. (A **Reject**
   flips it `held → discarded`; the relay never sends it. **Defer** leaves it held.)
6. **Deny path:** the analyst lacks `mcp:outbox.enqueue:call`. `request_approval` is denied opaquely at the
   stage step, **before** the item is recorded (or after — the partial-write contract must be decided:
   stage-effect-first so a recorded item never dangles without its effect; see Open questions), and the
   analyst learns only "denied".

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`) — the gate. **No mocks**: real store, real bus,
real caps, real MCP host; the item/resolution/effect are **real records** through the real verbs. The only
sanctioned fake stays the model provider behind the AI seam (unused here unless the body calls `ai.*`).

- **Capability-deny (§2.1):** `request_approval` denied without `inbox.record` *or* without the outbox
  stage cap — opaquely, mid-run, with **no partial write** (neither the item nor the held effect lands on a
  denied step). `resolve` denied without `inbox.resolve`. The reactor's release requires the driver
  authority, not a user cap (assert a user token cannot force a release).
- **Workspace-isolation (§2.2):** a ws-B rule's `request_approval` + a ws-B approval never release a ws-A
  held effect; a ws-B `inbox.resolve` cannot resolve a ws-A item (the pin refuses before the cap check).
- **The gated release (the headline):** stage a held effect → assert it is **not delivered** while held
  (the relay skips `held`) → approve → assert the reactor flips it `pending` and the relay delivers it
  **exactly once**. Reject → assert it is `discarded` and **never** delivered. Defer → still held.
- **Idempotency / determinism:** the same run twice upserts one item + one held effect (ids are
  `now`+counter); the reactor releasing twice (a replay) delivers once (the release is a guarded
  transition, not an unconditional enqueue).
- **The rule verb is caller-gated (regression):** a rule whose caller lacks the outbox cap cannot stage a
  gated effect even though its `inbox.record` would succeed — the whole `request_approval` fails closed.
- **Offline / sync (§2.3):** a held effect approved immediately before a node restart is released on the
  next reactor tick after reboot (the resolution + held effect are records; the reactor re-reads them).
- **Integration (real gateway/UI):** a Playground `*.gateway.test.tsx` runs a `request_approval` rule
  end-to-end against a real spawned node, asserts the `needs:approval` item + the held effect over the real
  gateway, drives an **approve** through the real `inbox.resolve` path, then asserts the effect moved to
  `pending`/delivered. (Extends `RulesMessaging.gateway.test.tsx`.)

## Risks & hard problems

- **Partial-write on the compound verb.** `request_approval` does two writes (item + held effect). If the
  second is denied/faults, the first must not dangle. Decide the order + the contract: **stage the held
  effect first, then record the item** (so an item never exists without its gated effect), and document
  that a mid-verb fault leaves at most the effect staged (harmless — held, never delivered, GC-able). The
  deny test asserts no partial write on the denied step.
- **The `held` status is new outbox surface.** Adding a `held` value means the **relay must skip it** and
  `outbox.status` must bucket it. A relay that treats `held` as `pending` would deliver un-approved effects
  — a security bug. The relay change must be tiny and explicitly tested (a held effect is never picked up).
- **The reactor's authority.** The release runs under a driver/system authority (like the coding-workflow
  reactor), not the requester's caps — otherwise a released effect could exceed what the *approver* holds.
  Get this right: the **request** is caller-gated; the **release** is a system transition gated by the
  resolution existing, not by re-checking a user cap. Document the trust boundary loudly.
- **Reviewer sees the effect, not a lie.** The Inbox UI must render the *actual* held effect
  (target/action/payload, secrets redacted) so "Approve" is informed consent. A summary that drifts from
  the staged effect is worse than none.
- **Double-fire / replay.** The reactor must make release a **guarded transition** (`held → pending` only
  if currently `held`), so a replay or a concurrent tick delivers once. Mirror the outbox's existing
  never-double-sent guard.

## Open questions

- **Tag representation: keep the body-text convention, or add a minimal typed facet?** Today
  `needs:approval route:… ` is a **string prefix in the item `body`** (`workflow/request_approval.rs`). A
  rule-raised approval could keep that (zero schema change, consistent with the shipped path) **or** add a
  small typed `ItemFacet`/`meta` (structured `{needs_approval, route}`) so the UI/reactor parse a field,
  not a string. Recommendation: **keep the body-tag convention for v1** (no `Item` schema change, reuses
  the exact reactor parse), and note the typed facet as the clean follow-up if a second consumer appears.
- **Where does the reactor live — generalize `resolve_approval`, or a sibling `approval_release` reactor?**
  The coding-workflow's `resolve_approval` starts a job; ours releases an effect. Do we (a) generalize it to
  "on `Approved`, run the item's registered follow-through (job **or** effect)", or (b) add a sibling
  reactor keyed on held effects? Recommendation: **(b) a sibling `approval_release` reactor** keyed on
  `(resolution, held-effect)` — it keeps the coding path untouched and the new path domain-free (rule 10),
  and both are just resolution-driven driver ticks.
- **Compound-write order + partial-failure contract** — confirm "stage effect (held) first, then record
  item", and whether a dangling held effect (item write failed) is GC'd by the reactor or left for
  `outbox.status` visibility. (Risks names the recommended answer; confirm at build.)
- **Does `defer` re-notify?** A deferred item stays held — is there a re-surface/reminder, or is it inert
  until re-resolved? Recommendation: inert in v1 (a reminder is a `reminders/` concern), documented.
- **Reviewer addressing (`route`).** Is `route:"team:managers"` enforced (only that team may resolve) or
  advisory (anyone with `inbox.resolve` may)? Recommendation: **advisory in v1** — the cap is the gate;
  `route` is a routing hint the UI filters on. Enforced routing is a policy scope.

## Related

- `rules-messaging-scope.md` — the `inbox`/`outbox`/`channel` rhai handles this extends (the built
  `inbox.record`/`inbox.resolve`/`outbox.enqueue` verbs + the caller-gated seam + the per-run write meter).
- `rules-engine-scope.md` — the engine + the `DataSeam`/`AiSeam`/`MessagingSeam` model.
- `../coding-workflow/` — the shipped `request_approval` → `resolve_approval` reactor this generalizes
  (`needs:approval` item + `Resolution` + a resolution-driven driver).
- `../inbox-outbox/` — the `Item`, the `Resolution` facet (`crates/inbox/src/resolution.rs`), and the
  durable outbox whose status enum gains `held`.
- `../jobs/` — the reactor-driver pattern (a boot tick, not a scan; see `flows` reactor lineage).
- `../auth-caps/auth-caps-scope.md` — `caps::check` under `caller ∩ grant`, the chokepoint every verb runs.
- `skills/rules/SKILL.md` — the rules Playground how-to the implementing session extends with a
  "propose-and-approve" worked example (a drivable surface — the build owns writing it, grounded in a live
  run).
- README `§6.10` (inbox/outbox), `§3` (rules 3/5/6/7/10).
