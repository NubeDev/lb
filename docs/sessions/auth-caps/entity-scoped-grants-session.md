# Entity-scoped grants — session

- Date: 2026-07-11
- Scope: `docs/scope/auth-caps/entity-scoped-grants-scope.md`
- Status: done

## Goal

Row-level reach inside a workspace: a grant that carries a resource selector, resolved by the same
grant store, checkable at the same wall. The cc-app childcare product's defining invariant — "a
guardian may read only their own children's daily logs" — is one of N re-implementations of this
check across extensions; this scope makes it one platform seam.

## What changed

### `lb-authz` crate (the store + resolver layer)

- **New `Scope` type** (`scope.rs`): `All` (default, today's behaviour) | `Ids { table, ids }`.
  Opaque data to the core (rule 10). `#[serde(default)]` so old grant records (no `scope` field)
  deserialize to `All` with zero migration. Includes `ScopeFilter` (`All | Ids(Vec<String>)`) for
  the query-side filter API.
- **`Grant` struct** (`grant.rs`): additive `#[serde(default, skip_serializing_if)] scope: Scope`
  field. `grant_id` now takes `(subject, cap, scope)` — for `All` the key is unchanged (backward
  compat); for `Ids` it's `subject::cap::table:sorted_ids`.
- **Backward-compatible store verbs**: `grant_assign`/`grant_revoke` keep their old 4-arg
  signature (defaulting to `All`); new `grant_assign_scoped`/`grant_revoke_scoped` take `&Scope`.
  `grant_list_scoped` returns full `Vec<Grant>` (with scope); `grant_list` deduplicates caps.
- **`resolve_caps_scoped`** (`resolve_scoped.rs`): the scoped twin of `resolve_caps`. Same fold
  (direct ∪ roles ∪ team-inherited), but carries the scope union per cap. Any `All` grant wins;
  `Ids` for the same table merge id sets. Role-expanded caps are always `All` (a role defines
  *what you can do*, not *which records*).
- **`check_scoped` / `scope_filter`** (`check_scoped.rs`): thin reads over `resolve_caps_scoped`.
  `check_scoped(store, ws, user, cap, table, id) -> bool` — point check. `scope_filter(store, ws,
  user, cap, table) -> ScopeFilter` — query-side filter (`All` or `Ids`). Both have `_with`
  variants that accept an injected `BuiltinRoleCaps`.
- **`revoke_subject`** (`revoke.rs`): updated to use `grant_list_scoped` + `grant_revoke_scoped`
  so it revokes ALL grants including scoped ones.
- **`role_delete` cascade** (`role.rs`): `grant_id` call updated for the new signature.

### `lb-host` crate (the MCP bridge + capability gate)

- **`grants_assign`/`grants_revoke`** (`authz/grants.rs`): now take `&Scope` as the last
  parameter. The gateway's REST routes pass `&Scope::All`. New `grants_list_scoped` for the
  Access console.
- **`authz.check_scoped` / `authz.scope_filter`** (`authz/scoped.rs`): the MCP-bridge entry
  points. Gated by `mcp:authz.check_scoped:call` / `mcp:authz.scope_filter:call`. Resolve the
  CALLING principal's own reach (never accepts a `user` arg — no information leak). Strip
  `user:` prefix from `principal.sub()` to match the grant store's bare-name convention.
- **`call_authz_tool`** (`authz/tool.rs`): `grants.assign`/`revoke` now parse the optional
  `scope` field from the JSON input (absent → `All`). New verbs `authz.check_scoped`,
  `authz.scope_filter`, `grants.list_scoped` dispatched.
- **Tool dispatcher** (`tool_call.rs`): `"authz."` added to `HOST_NATIVE_PREFIXES` + dispatch
  branch routing to `call_authz_tool`. Extensions can now call `authz.check_scoped` /
  `authz.scope_filter` via `host.call-tool`.
- **Built-in role caps** (`authz/builtin_roles.rs`): `mcp:authz.check_scoped:call` and
  `mcp:authz.scope_filter:call` added to the **viewer** set — every member can ask "what can I
  reach?" (informational; enforcement still happens at the verb level).
- **System catalog** (`system/catalog.rs`): `authz.check_scoped`, `authz.scope_filter`,
  `authz.resolve`, `authz.revoke-tokens` added to the host inventory.

### Gateway

- `admin_grants.rs`: `assign_grant` / `revoke_grant` routes pass `&Scope::All` (the REST body
  could be extended to accept a `scope` field, but the MCP path is the primary surface for scoped
  grants — extensions call `grants.assign` via `host.call-tool`).

## Decisions & alternatives

1. **SDK host-callback via MCP, not a new WIT import.** The scope flagged "one additive
   host-callback pair in `lb-ext-sdk` (`authz.check_scoped` / `authz.scope_filter`)." Since the
   WIT lives out-of-tree (`lb-ext-sdk` repo) and the existing `host.call-tool` IS the host-callback
   ABI (extensions call any MCP tool through it), I implemented them as MCP tools dispatched
   through `call_authz_tool` + the `authz.` prefix in `tool_call.rs`. This is **more additive**
   than a WIT change (zero WIT bump, no `WORLD_MAJOR` risk) and fully functional — extensions call
   `host.call-tool("authz.scope_filter", {cap, table})`. *Rejected alternative:* modify the
   out-of-tree WIT — would require a cross-repo coordinated release and a `sdk-v0.3.0` tag, with
   no functional benefit over the existing `host.call-tool` seam.

2. **Selector forms v1: `ids` only.** The `tag` selector form is deferred — no real cohort caller
   exists yet (the scope's recommendation). The `Scope` enum is designed so `tag` is an additive
   variant later.

3. **`scope_filter` returns ids, not a WHERE fragment.** The core stays out of query-string
   business (the scope's recommendation). The verb returns `Ids(Vec<String>)`; the caller pushes
   them into its own indexed query.

4. **Watch verbs: filter-at-emit in the extension for v1.** No scoped subscription helper — an
   extension that needs scoped watch filters at the emit side itself (the scope's recommendation).

5. **`check_scoped`/`scope_filter` use the calling principal's own sub.** They never accept a
   `user` argument — a caller can only learn its OWN reach. This is the no-information-leak
   guarantee. *Rejected alternative:* accept a `user` arg gated by an admin cap — adds complexity
   for no real use case; the Access console's `authz.resolve` already shows a subject's full
   cap set with provenance.

6. **Scoped check caps in the viewer set.** Every member can ask "what can I reach?" — this is
   informational (the enforcement happens at the verb level). *Rejected alternative:* require a
   separate cap — would mean extensions need an extra grant just to ask what they can reach,
   which defeats the purpose.

7. **Backward-compatible store verbs.** `grant_assign(store, ws, subject, cap)` keeps its old
   4-arg signature (delegates to `grant_assign_scoped(…, &Scope::All)`). This means ~15 existing
   callers across the codebase needed zero changes. Only the host-layer `grants_assign` /
   `grants_revoke` wrappers (which needed the scope param for the MCP bridge) changed signature.

## Tests

Real store, real resolver, real capability gate — no mocks (rule 9).

### `lb-authz` unit tests (`tests/scoped_grants_test.rs` — 15 tests)

- `scoped_grant_narrows_check_scoped` — cap held, record outside scope → denied
- `all_scope_grant_allows_any_record` — All scope → any table/id
- `scope_filter_returns_ids_for_scoped_grant` — filter returns the id set
- `scope_filter_returns_all_for_all_grant`
- `scope_filter_returns_empty_for_different_table`
- `scope_filter_returns_empty_for_unheld_cap`
- `union_of_multiple_scoped_grants_merges_ids` — two scoped grants union
- `all_grant_wins_over_scoped_grants` — any All wins
- `revoke_scoped_grant_denies_after_revoke` — freshness: deny after revoke
- `revoking_one_scope_keeps_the_other`
- `empty_scope_lists_return_empty_not_error`
- `grant_list_scoped_returns_full_records`
- `old_grant_record_without_scope_field_deserializes_to_all` — zero migration
- `scoped_grants_never_cross_the_workspace_wall` — **mandatory workspace isolation**
- `role_expanded_caps_are_all_scope`

### `lb-host` integration tests (`tests/authz_scoped_test.rs` — 7 tests)

- `scoped_grant_assign_and_check_scoped_over_mcp` — full MCP round-trip
- `check_scoped_for_scoped_principal` — scoped principal's own reach
- `scope_filter_over_mcp_returns_ids`
- `denies_check_scoped_without_its_cap` — **mandatory capability deny**
- `denies_scope_filter_without_its_cap` — **mandatory capability deny**
- `denies_scoped_grant_assign_without_grants_cap` — **mandatory capability deny**
- `scoped_checks_never_cross_workspace_wall` — **mandatory workspace isolation**

### Existing tests (no regressions)

- `lb-authz` crate: 4 + 3 + 3 = 10 tests green (builtin_role_freshness, resolve_key,
  access_console)
- `lb-host` authz_test: 7 green (deny, isolation, union, no-widening, idempotent)
- `lb-host` admin_crud_test: green (revoke seam with scoped grants)
- `lb-host` dashboard_access_check_test: 2 green
- `lb-host` builtin_roles unit tests: 6 green (tier lattice holds)
- `lb-host` system catalog tests: 2 green (prefix coverage holds)

```
running 15 tests ... test result: ok. 15 passed; 0 failed
running 7 tests ... test result: ok. 7 passed; 0 failed
```

## Debugging

None — clean implementation on the first pass.

## Public / scope updates

Scope open questions resolved (see Decisions above):
- Selector forms v1: ids only (tag deferred)
- scope_filter returns ids (not WHERE fragment)
- Watch verbs: filter-at-emit in extension for v1

## Follow-ups

- `tag` selector form when a real cohort caller exists.
- Access console UI renders the scope selector (the `grants.list_scoped` verb is ready).
- The gateway REST body for `/admin/grants` could accept a `scope` field (the MCP path already
  does).
