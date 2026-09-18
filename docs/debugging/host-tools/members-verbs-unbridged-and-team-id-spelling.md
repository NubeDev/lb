# The assign picker was empty on every node: three independent bugs behind one symptom

- **Date:** 2026-09-18
- **Area:** host-tools (`members.*` MCP dispatch) + insights/case (the team-graph walk, the roster filter)
- **Status:** fixed
- **Found by:** a report from the rubix-ai surface — "the assign picker only offers *Assign to me*",
  reproduced against a real dev node rather than a test

## Symptom

`case.assignees` — the verb the assign picker reads — returned `{"me":"user:test","teams":[],
"users":[]}` on every workspace. The picker renders nothing when the roster is empty (deliberately:
an empty box captioned "no one to assign to" is worse than no box), so the control offered *Assign
to me* and the free-text field, which is exactly the state `case.assignees` had shipped to fix.

The UI was not at fault. The hook, the api client and the render path were all correct and tested;
the data genuinely was not there.

## Root cause

Three unrelated defects. The first two each produce the identical empty roster, and the first hides
the second, which is why fixing one and stopping would have looked like no progress at all. The
third only becomes reachable once the roster is non-empty — it turns the empty picker into a picker
that offers a row the assign then refuses, so it would have shipped as the "fix"'s own bug report.

### 1. `members.*` had caps, a service, and no dispatch arm

`members.add` / `members.list` / `members.remove` shipped with their authorization written, their
caps in the role bundles (`mcp:members.add:call`, `mcp:members.list:call`) and unit tests over the
Rust functions — and **no `call_*_tool` arm**, so every MCP call answered `no such tool`. A
capability and a catalog descriptor are not reachability: a verb is reachable only when some
dispatcher matches its name, and reachability is per-transport.

The catalog test only asserted one direction — every dispatched prefix has a catalog row. The
reverse (every catalogued row is dispatchable) was never asserted, so a family could be advertised
to `/system/tools`, the console and the agent menu while answering `no such tool` for every caller.

Note the gateway's own `POST /teams/{team}/members` REST route worked throughout, so the admin
console could add members. Only the MCP path was dead. That asymmetry is why this survived: the
surface a human clicked was fine, the surface a client or agent calls was not.

### 2. A team id has two spellings, and the edge lookup only knew one

`team_create` stores the team id **verbatim** — it normalises nothing — so `mechanical` and
`team:mechanical` are both real team records, and the product creates both: the admin console posts
a bare id, while `case.assign` and the pickers speak the prefixed form. The `member` edge is keyed
by that same raw string.

Both team walks — `insight::assignee::me_subjects` (the `Mine` lane) and `case::assignees`
(the picker) — looked up `list_related(.., MEMBER, &team.team)` under **one** spelling and
normalised to `team:` only *afterwards*, for the returned subject. A bare team record therefore
never matched its prefixed member edges: no error, just an empty membership, which reads downstream
as "this caller is on no team".

`validate_assignee` in the same file already accepted both spellings (`t.team == assignee ||
t.team == name`), so the codebase knew about the dual form; only the edge lookup forgot it.

This one also silently cost the **`Mine` lane** every team-owned insight, on any workspace whose
teams were made through the admin console. That damage was never reported — an empty lane looks
like no work assigned.

### 3. The picker offered subjects `case.assign` refuses

Found by clicking a row in the repaired UI: the roster offered `user:priya`, and assigning answered
`bad input: assignee is not a member of this workspace`.

The two sides apply different rules. `validate_assignee` requires a **live workspace membership**;
`case_assignees` collected **whoever the `member` edge names**. That edge is an unvalidated write —
`add_member` calls `relate` with no membership check — so a team can name a subject who never joined
the workspace, or who has since been removed. The control then contradicts itself in front of the
operator, which reads as a broken assign rather than a stale roster.

Teams do not have this problem: `validate_assignee` accepts any team that exists, and the team was
just read out of `team_list`.

## Fix

1. `crates/host/src/members/tool.rs` — the missing MCP bridge (`call_members_tool`), wired through
   `members/mod.rs`, `lib.rs`, the `members.` prefix in `tool_call.rs::HOST_NATIVE_PREFIXES` and a
   router arm. Kept out of the `teams.`/`authz.` arm on purpose: `teams.*` is the team RECORD,
   `members.*` is the `member` EDGE, different services and different caps.
2. `tool_gate.rs` — a `members.remove` → `teams.manage` alias. Its inner gate checks `teams.manage`
   and no `mcp:members.remove:call` exists in any bundle, so deriving the outer gate from the verb
   name would have denied it for every caller including admins — the shipped-but-unusable trap that
   table exists to prevent. `members.add`/`members.list` need no arm; their own caps are real.
3. `system/catalog/members.rs` — the three catalog rows, plus the reverse-direction test
   `every_catalogued_verb_is_dispatchable`, so the next family that forgets its bridge fails in CI
   instead of in a UI months later.
4. `case/assignees.rs` — the roster now keeps only `user:` subjects that pass
   `membership_is_member`, filtered after the dedup so each distinct subject costs one read. A
   failed read drops the row, matching the unreadable-team degrade posture; the typed box remains
   the door to anything omitted.
5. `insight/assignee.rs::team_member_edges` — one owner for "the member edges of this team",
   accepting both spellings (the second read happens only when the first finds nothing, so a
   correctly-keyed team still costs one read). `case::assignees` now shares it, so the picker and
   the `Mine` lane cannot disagree about what "my team" means.

## Tests

`crates/host/tests/members_mcp_test.rs`, over a real booted node and the real `call_tool` bridge:
the add→list round trip, idempotency, capability-deny, workspace isolation, the positive gate test
for the `members.remove` alias, the bare-vs-prefixed resolution, the offered-subject round trip
(every row the picker offers actually assigns — asserted as the round trip, not as the filter), and
the end-to-end one: with the edge written over MCP, `case.assignees` names the team and the
teammate.

All three fixes were verified to FAIL without their repair (prefix removed; dual lookup removed;
membership filter removed), not merely to pass with it.

## Lesson

Two lessons, and the second is the one that generalises.

**A cap plus a descriptor is not reachability.** Both are declarations; dispatch is the fact. Assert
the catalog and the dispatcher agree in *both* directions, or a verb can be advertised and dead at
once. And a working REST route says nothing about the MCP path — reachability is per-transport.

**Two surfaces over one dataset must agree on the rule, not just the data.** The roster read and the
assign write both answer "may this subject own this case?" and each had its own answer. Whenever a
read exists to feed a write, the property to test is the round trip — every offered option is
accepted — rather than either side alone. Neither side looked wrong in isolation.

**When a fixture and the product disagree about a key, tests prove nothing.** `seed_roster` wrote
`team:mechanical` for both the record and the edge, so every existing test agreed with itself and
passed, while the product wrote a bare record and a prefixed edge and failed. A fixture that
constructs state directly cannot catch a mismatch between two writers — only a test that writes
through the same verbs the product uses can. The bare/prefixed test above is that test.
