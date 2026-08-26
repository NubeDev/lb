# nav — `nav.get_default`: the workspace-default pointer becomes readable (session)

- Date: 2026-08-18
- Scope: ../../scope/nav/nav-builder-scope.md (the "Workspace-default" decision, extended)
- Related: ../../scope/nav/nav-no-lockout-scope.md (why an admin sees no effect on their own rail)
- Downstream issues: NubeIO/rubix-ai#165 (write-only default looks broken), NubeIO/rubix-ai#144
  (admin no-lockout restore — the second consumer of the same read)
- Status: done (unreleased — needs the next `node-v*` tag before rubix-ai can bump its pin)

## Goal

Give the one workspace-default nav pointer a **read**. It shipped write-only: `nav.set_default` /
`POST /nav/default` persisted correctly, and no caller — UI, agent or curl — could ask which nav the
workspace points at.

## Why it mattered (the two-sided symptom)

The downstream report was "the workspace default doesn't persist". It does. What was missing was any
way to *observe* it, and the two observation channels an admin naturally reaches for were both dead:

1. **The badge.** rubix-ai's `NavRoster` documents the hole in its own header comment — the "Default"
   badge marked the row only after it was set **in that browser session**, an honest local echo of the
   last write. Reload, or have a second admin look, and the badge is gone while the pointer is still
   set server-side.
2. **Their own sidebar.** Setting a default changes nothing on the *setting admin's* rail, by design:
   the no-lockout rule has an admin narrowed only by their OWN explicit pick, so resolve tiers 2 and 3
   are skipped for them (`nav/admin_lens.rs`). Correct, and indistinguishable from a no-op.

Write persisted + no confirmation + no personal effect = "not persisted". The fix is a read, not a
change to either behaviour.

## What changed

**Host (`rust/crates/host/src/nav/`)**

- `default.rs` — now owns both halves of the pointer: `nav_set_default` (unchanged) and the new
  `nav_get_default(store, principal, ws) -> Option<String>`. `None` for "never set" and for "cleared"
  alike — the same absence the resolver falls through on; there is nothing to distinguish. Reuses the
  existing `store::read_default` the resolver already calls.
- `mod.rs` / `lib.rs` — export + the module verb list.
- `tool.rs` — the `nav.get_default` MCP arm, returning `{"id": …}`.
- `tool_gate.rs` — `nav.get_default` joins the `nav.pref.*` / `nav.hidden.get` / `nav.ext_boards.get`
  arm that aliases to `nav.resolve`, so the dispatcher's outer gate and the `tools.catalog`
  visibility gate ask the same question the verb re-checks inside.
- `system/catalog/nav.rs` — the catalog row (advertise a tool exactly when the call would allow it).

**Gateway (`rust/role/gateway/`)**

- `routes/nav.rs` — `get_default_nav` → `{"id": "ops"}` / `{"id": null}`.
- `server.rs` — the same path carries both halves: `.route("/nav/default", post(set_default_nav).get(get_default_nav))`.

## The design call: the read is member-level, the write is not

`POST /nav/default` keeps `mcp:nav.save:call` (authoring). `GET /nav/default` gates on
`mcp:nav.resolve:call`, so every member — and a read-only viewer — can read it.

The pointer is already the **third tier of the caller's own `nav.resolve`**: for anyone it applies to,
the menu they are looking at *is* the default. Naming it discloses nothing new, while gating the read
on the authoring cap would make the pointer unreadable to exactly the population it shapes. This is
the same reasoning `nav.hidden.get` and `nav.ext_boards.get` already ride.

*Rejected:* folding the id into `nav.resolve`'s payload. `resolve` answers "what is MY menu", and for
an admin the default is precisely what does **not** apply — the answer would be absent exactly where
the builder needs it.

## Tests (real store + real gateway, no mocks)

- `crates/host/tests/nav_test.rs` — three new: `workspace_default_pointer_reads_back_and_clears`
  (unset → `None`, set → the id, LWW on a second write, `""` clears and the read reports the clear),
  `default_pointer_read_is_member_level_write_is_not`, `default_pointer_is_workspace_walled`. Plus a
  `nav_get_default` arm on the existing `each_verb_is_denied_without_its_cap` (deny-per-verb).
- `role/gateway/tests/nav_default_route_test.rs` (new) — the route over the real gateway + SurrealDB:
  round trip and clear, a plain member reads `200` and their `POST` is `403`, and a ws-B admin never
  sees ws-A's pointer.

```
$ cargo test -p lb-host --test nav_test
test result: ok. 49 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.91s

$ cargo test -p lb-role-gateway --test nav_default_route_test
running 3 tests
test the_read_is_member_level_and_the_write_stays_admin_ish ... ok
test the_default_pointer_is_workspace_walled ... ok
test default_pointer_round_trips_over_the_route ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.43s

$ cargo test -p lb-host --test tools_catalog_test --test catalog_mcp_test --test persona_menu_full_catalog_test
test result: ok. 8 passed; 0 failed  (tools_catalog_test)
test result: ok. 2 passed; 0 failed  (persona_menu_full_catalog_test)
test result: ok. 6 passed; 0 failed  (catalog_mcp_test)

$ cargo test -p lb-role-gateway --test nav_reach_test --test mcp_bridge_test --test catalog_routes_test
test result: ok. 7 passed; 0 failed
test result: ok. 8 passed; 0 failed
test result: ok. 2 passed; 0 failed

$ cargo fmt --all -- --check     # clean
```

## Docs moved

- `scope/nav/nav-builder-scope.md` — the "Workspace-default" decision now carries the read half and
  the rejected alternative.
- `skills/nav/SKILL.md` — the verb table row + the cap paragraph.
- `testing/nav/README.md` — the curl line, the viewer-posture paragraph, and a note on why the route
  is member-level.
- `STATUS.md` — the slice.

## Follow-up (downstream, not this repo)

rubix-ai consumes it: `nav_get_default` in `http.ts` → `getDefaultNav()` in `nav.api.ts` → `useNavs`
loads it on mount so `NavRoster`'s badge is a real read, plus the "applies to members, not admins"
hint next to the control. Needs a `node-v*` tag + pin bump (`WORKFLOW-LB.md` §4a); until then it runs
on the local `[patch]` already active on that box.
