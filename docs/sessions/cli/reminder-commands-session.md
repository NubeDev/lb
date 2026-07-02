# CLI — `lb reminder …` commands (session)

Date: 2026-07-01
Scope: `docs/scope/core/resource-verbs-scope.md` (the grammar), `docs/scope/reminders/reminders-scope.md`
(the shipped verbs). Skill: `docs/skills/lb-cli/SKILL.md` ("The common resource grammar").

## Goal

Ship the `lb reminder` family — the **reference** family of the common resource grammar — as first-class
typed clap subcommands over the already-shipped `reminder.*` MCP verbs, wired through the single dispatch
table (resource-verbs D5). Pure client sugar over `POST /mcp/call`: no new MCP verb, capability, or table.

Mapping:

| CLI | MCP verb |
|---|---|
| `lb reminder ls [--status enabled\|disabled] [--limit N]` | `reminder.list` |
| `lb reminder show <id>` | `reminder.get` |
| `lb reminder create --channel <c> --body <t> --cron <expr>` | `reminder.create` (prints the id, D4) |
| `lb reminder update <id> [--enabled] [--cron] [--max-runs]` | `reminder.update` |
| `lb reminder rm <id> [--hard]` | `reminder.delete` |

## What changed

**CLI (`rust/role/cli/`):**
- `src/cli.rs` — new `Reminder(ReminderCmd)` subcommand + the `ReminderCmd` enum (one variant per verb,
  friendly flags, real `--help`/completion).
- `src/commands/reminder/` — one file per verb (FILE-LAYOUT): `ls.rs`, `show.rs`, `create.rs`,
  `update.rs`, `rm.rs`, plus `mod.rs` holding the family's own two helpers — `now_ts()` (the wall-clock
  logical timestamp every write verb requires) and `derive_id()` (body→slug id for the no-`--id` UX).
- `src/dispatch.rs` — one `dispatch_reminder` table maps each subcommand to its handler (D5).
- `src/output.rs` — added `reminders` to the list-envelope unwrap keys, so `reminder.list`'s
  `{reminders:[...]}` tables its rows (same defensive unwrap as `inbox.list`'s `{items}`).
- `src/commands/mod.rs` — register the `reminder` module.

**Server (`rust/crates/host/`, `rust/crates/reminders/`):** extended the existing `reminder.list` verb
to accept the D3 shared-core filters `{status?, limit?}` — **not** a new verb/cap/table, just the
grammar's own list contract on the reference family:
- `crates/host/src/reminder/get.rs` — `reminder_list` now takes `Option<StatusFilter>` + `Option<usize>`;
  new `StatusFilter{Enabled,Disabled}` with a `parse` that returns `BadInput` on garbage (author
  feedback, not an opaque deny). Filtering is host-side over the ws-scoped read (the workspace is small,
  the wall is already enforced by the scan underneath). The reactor's due-scan (`scan::due`) is untouched.
- `crates/host/src/reminder/tool.rs` — the `list` arm parses `status`/`limit` from the JSON args.

## Decisions & assumptions (deviations from the raw prompt, grounded in the shipped verbs)

The prompt's assumed verb shapes differed from what's actually shipped; the CLI is pure sugar, so it
conforms to the real verbs. Decisions:

1. **`create` needs a client-supplied `id` (the verb does not server-generate one).** Per the user
   ("nice UX … make a body to enter"), the CLI derives the id from a kebab slug of the `--body` plus a
   short base-36 suffix of the timestamp (`standup-time-thhn28`), so the common case needs no `--id`.
   `--id` remains an explicit override. `create` still **prints the id** (D4), read back from the
   server's response (so it's the id the store actually holds), never the full record.
2. **`--status`/`--limit` are real server filters, not CLI-side.** Per the user ("add it to the backend
   … better long term") and resource-verbs **D3** ("every `list` takes `{status?, limit, cursor}`"), the
   shipped `reminder.list` verb was extended to accept them. `status` maps to the reminder's `enabled`
   flag (`enabled`/`disabled`); `limit` truncates the sorted head. `cursor` is not added (the workspace
   read is unpaged today; a keyset cursor is a later addition per the scope, noted as N/A for v1).
3. **`ts` is stamped from the wall clock** by the CLI (`now_ts()`), since every write verb requires a
   logical timestamp and a client always means "now."
4. **`--hard` is accepted and forwarded as `hard:true` but is a no-op today** — the shipped
   `reminder.delete` always soft-tombstones. The flag exists so the surface matches every family's `rm`
   and becomes load-bearing when per-family undo lands (resource-verbs D2). Documented in `rm.rs`.
5. **`dev_claims` already authorizes the family** via the `mcp:*.{create,list,get,update,delete}:call`
   wildcards — so a real `lb login` (or `lb local`) operator can use `lb reminder` with no cap change.
   The deny test therefore mints a **narrow** token (explicit caps minus delete), not `dev_token`.

## Tests (rule 9 — real gateway on a real socket, seed via the real write path)

`rust/role/cli/tests/reminder_test.rs` (mirrors `remote_test.rs` + the inbox-list test), plus a new
`seed_reminder` helper in `tests/common/mod.rs` (seeds via the real `reminder_create` host verb):

- `create_ls_show_update_rm_round_trips_over_the_real_gateway` — the full round-trip; asserts create
  prints only the id (D4), ls/show reflect it, `--status disabled/enabled` track the pause, rm removes it.
- `rm_without_the_delete_cap_is_a_deny_and_never_a_fake_success` — **mandatory deny**: a token with every
  reminder cap *except* `reminder.delete` → `DENIED  mcp:reminder.delete:call`, exit 3, record untouched.
- `a_ws_a_token_lists_only_ws_a_even_when_targeting_b` — **mandatory isolation** (read): ws-A + ws-B
  seeded on one node; the A-token's list returns only A's reminder.
- `ws_b_cannot_rm_a_ws_a_reminder` — **mandatory isolation** (write): a ws-B token's rm cannot reach
  ws-A's namespace; ws-A's reminder survives.

Server-side, the D3 extension is covered by a new case in `crates/host/tests/reminders_mcp_test.rs`:
`list_status_and_limit_filter_over_the_ws_read` (status=enabled/disabled selects; limit truncates; a
garbage status is `BadInput`, not `Denied`).

### Green output

```
$ cargo test -p lb-cli
   Running tests/reminder_test.rs
running 4 tests
test a_ws_a_token_lists_only_ws_a_even_when_targeting_b ... ok
test ws_b_cannot_rm_a_ws_a_reminder ... ok
test rm_without_the_delete_cap_is_a_deny_and_never_a_fake_success ... ok
test create_ls_show_update_rm_round_trips_over_the_real_gateway ... ok
test result: ok. 4 passed; 0 failed; ...
   Running tests/remote_test.rs        ... 6 passed
   Running tests/sign_test.rs          ... 2 passed
   (lib) 24 passed  (local/config/ext tests) all green

$ cargo test -p lb-host --test reminders_mcp_test
running 6 tests ... test result: ok. 6 passed; 0 failed
  (includes list_status_and_limit_filter_over_the_ws_read)

$ cargo test -p lb-host --test reminders_reactor_test
running 10 tests ... test result: ok. 10 passed; 0 failed   (no regression)
```

### Live end-to-end run (grounds the skill)

```
$ lb local -w acme reminder create --channel team --body "standup time" --cron "0 9 * * 1"
ws: acme  user: user:ada  role: member  mode: local        # stderr
standup-time-thhn28                                         # stdout — just the id (D4)

$ lb local -w acme reminder create ... -o json              # raw envelope (scripting)
{ "id": "standup-time-thhn2l", "schedule": "0 9 * * 1", "maxRuns": null, "runs": 0,
  "enabled": true, "status": "active", "action": {...}, "nextAttemptTs": 1783328400, "ts": ... }

$ lb local -w acme reminder show nope
(no rows)                                                   # missing/tombstoned → legible, not "null"

$ lb local -w acme reminder ls --status paused; echo $?
unknown status filter 'paused' (expected enabled|disabled)
2                                                           # BadInput, honest — not a deny, not a crash
```

(A multi-command `lb local` round-trip does not persist between invocations — each `lb local` boots a
fresh in-process node — which is why the *persisted* round-trip is proven by the real-gateway integration
test above, where one node lives across the create→ls→show→update→rm sequence.)

## Debugging

Nothing broke; no `docs/debugging/cli/` entry needed this session.

## Public / scope updates

- `docs/skills/lb-cli/SKILL.md` — flipped the `lb reminder …` examples from ⏳ to ✅ in "The common
  resource grammar", grounded in the live run above; noted the `create`-prints-id and `--status/--limit`
  behaviours.
- `docs/scope/core/resource-verbs-scope.md` — reminders remain the reference family; the CLI half of the
  grammar is now real for this family (the section's promotion to `public/core/core.md` waits on ≥3
  families adopting, per the scope's own gate).

## Follow-ups

- `reminder.list` keyset `cursor` (D3's third field) — deferred; the ws read is unpaged today.
- `--hard` becomes load-bearing when reminder `undo`/soft-delete lands (D2).
- Adopt the grammar for the next families (`job`, `flow`, `ext`) to reach the ≥3-family promotion gate.
