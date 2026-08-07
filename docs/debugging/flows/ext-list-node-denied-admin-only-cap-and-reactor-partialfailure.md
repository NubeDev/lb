# `ext-list` flow node `denied`, then `partialFailure` on the auto-fired run

- Area: flows / authz role bundles / flow reactor
- Found: ext-store-nodes session (2026-07-25), live on the rubix-ai dev node
- Symptom (two faults, one root theme): (1) a flow with an `ext-list` node returns `denied` at that
  node; (2) after fixing (1), a MANUAL run succeeds but the FLIP-FLOP / cron auto-fired run settles
  `partialFailure` — `ext-list` back to `denied` while the sibling `counter` branch is `ok`.

## Symptom

Flow `aaa` (`flipflop-1` → `counter-1` + `ext-list-1` → `debug-1`) on `http://localhost:5099/#/t/nube/flows/aaa`:

```
# manual run (author's gateway token) — after fault 1 fixed:
status: success   ext-list-1: ok (2 extensions)

# flip-flop auto-fired run (aaa-flip-flipflop-1-<ts>):
status: partialFailure
  counter-1  ok
  ext-list-1 err   error: denied
  debug-1    skipped   (its only upstream failed)
  flipflop-1 ok
```

`partialFailure` = `any_ok && any_failed` in one run (`run_store.rs::finalize_if_complete`) — a status
the flow author had never seen because no prior flow mixed a denied node with a passing one.

## Root cause

**Fault 1 — `mcp:ext.list:call` was admin-only.** The ext-store-nodes scope moved `store.tables` into
the member `AUTHOR_CAPS` bundle for the store-table picker but left `ext.list` in `ADMIN_ONLY_CAPS`
(`authz/builtin_roles.rs`). The `ext-list` node and the `lb:extension` picker both dispatch `ext.list`
under the flow author's own principal (`execute_node/ext_call.rs`), and the seeded dev user `test` is a
`member`, not an admin — so the outer gate `mcp:ext.list:call` denied it. (A stale dev store masked
this: an OLDER binary had seeded/compiled `member` WITH the broad `ext.*` caps, and the running node
was that old binary — a fresh build from current source denies, matching the user's report.
`resolve_caps_live` REPLACES a stale built-in role row with the live bundle, so the fix needs no store
purge — it lands on next login after rebuild.)

**Fault 2 — the flow reactor's system principal lacked the new nodes' caps.** A cron / flip-flop /
webhook flow does not run under the author's token; it runs under `reactor_caps()`
(`flows/reactor_loop.rs`), the `node:reactor` system principal. Its `mcp:*.call:call` wildcard matches
only `<x>.call` verbs (e.g. `native.call`) — never `ext.list` (verb `list`), nor `store.query`/
`store.write`/`store.delete`. So every ext-store-node the scope added was denied on a headless run,
even though the scope's own headline example is a *nightly-cron* flow using `ext-list` + `store-write`.

## Fix

- `authz/builtin_roles.rs`: move `mcp:ext.list:call` from `ADMIN_ONLY_CAPS` → `AUTHOR_CAPS`. Lifecycle
  mutators (`ext.disable`/`start`/`uninstall`/`publish`, `native.install`) stay admin-only.
- `flows/reactor_loop.rs`: add `mcp:ext.list:call`, `mcp:store.query:call`, `mcp:store.write:call`,
  `mcp:store.delete:call` to `reactor_caps()` so a headless flow drives every BUILT-IN platform node.
  NOT added: arbitrary `ext-call` to `<ext>.<tool>` (would need blanket `mcp:*.*:call`; the follow-up
  is run-as-owner — the flow has no `owner` field yet).
- Admin-console marker retarget (fallout of the tier move — `ext.list` was tripling as the admin
  marker): `nav/admin_lens.rs` `ADMIN_MARKER_CAPS`, `nav/surfaces.rs` `extensions` gate, and the
  rubix-ai/ui mirror (`admin-caps.ts` `ADMIN_SECTION_CAPS`, `routing/allowed.ts`, `App.test.tsx`) all
  move to the admin-only `mcp:ext.uninstall:call`. Individual extension **UI pages** stay on `ext.list`
  (using an extension is member-level; managing them is admin).

## Regression tests

- `builtin_roles::ext_list_is_an_author_cap_but_lifecycle_mutators_stay_admin`
- `reactor_loop::reactor_drives_the_builtin_platform_nodes_but_not_arbitrary_ext_call`
- rubix-ai/ui `admin-caps.lockstep.test.ts` (UI ⇄ lb marker set) + `App.test.tsx` (extensions surface
  gated on `ext.uninstall`, not `ext.list`).

## Verified live

Rebuilt + restarted the dev node; `test` (member) login now carries `mcp:ext.list:call` but NOT the ext
mutators (`resolve_caps_live` replaced the stale row). Flip-flop auto-runs flipped `partialFailure` →
`success` (`ext-list-1: ok`, 2 extensions). Store nodes e2e: `store-write` → `store-read` round-trips a
row; `store-write` to reserved table `flow` → `bad input: reserved table: flow` (wall holds through the
node), flow `aaa` untouched.
