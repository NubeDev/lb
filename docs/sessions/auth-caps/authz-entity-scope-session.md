# Auth-caps — `authz.entity_scope`, the menu-aware reach question (session)

- Date: 2026-10-05
- Scope: ../../scope/auth-caps/entity-scoped-data-scope.md ("Extensions ask the same resolver")
- Status: done (unreleased — needs the next `node-v*` tag)

## Goal

Let an extension ask "which sites may this person read?" and get the answer lb itself enforces.

## Why

A downstream extension (ext-ros waste tracking) keeps per-site rows in an estate database and must
show a viewer only their sites. The only reach verbs an extension could call, `authz.check_scoped`
and `authz.scope_filter`, read **scoped grants only**. Sites are handed to people through **menus**
(`entity-scoped-data-scope.md`: "menus are access"; the rubix-ai ESR rollout marks entities on team
menus), and `entity_scope` — the resolver federation, insights and cases use — reads menus AND
grants. So the extension refused every viewer that lb's own reads would have allowed. Measured live
on a rubix-ai scratch node before this change: a member granted `data:site:read` is not even
reachable through `grants.assign` (the cap grammar has no `data` surface, so the no-widening rule
refuses it for every caller); menus are the working path, and no verb exposed them.

## What changed

- `host/src/authz/entity_reach.rs` — the verb. Calls `entity_scope` unchanged; with `subject` it
  requires `mcp:authz.delegate_reach:call` and rebuilds the user with live caps (the same rebuild
  `entity_scope` does for a derived principal's owner). `user:` subjects only; `sources` validated.
- `authz/tool.rs` dispatch arm; `authz/scoped.rs` shares `DELEGATE_REACH_CAP` (one definition).
- `system/catalog/authz.rs` catalog row; the delegate marker's description names the new verb.
- `authz/builtin_roles.rs` — `mcp:authz.entity_scope:call` in the VIEWER bundle beside
  `check_scoped`/`scope_filter` (self-only, informational).

## Decisions & alternatives

- **Expose the resolver, do not add a source to `scope_filter`.** `scope_filter` is a per-CAP grant
  question (`cap` argument); menus mark entities, not caps. Mixing them would change an existing
  verb's meaning for every caller.
- **Rejected: extensions read through `federation.query` under the viewer.** Only works for a host
  datasource, and moves the extension's logic into the browser.
- Reply shape mirrors `scope_filter` so a caller decodes both the same way.

## Tests

```
$ cargo test -p lb-host --test authz_entity_scope_test
test the_verb_is_denied_without_its_cap ... ok
test a_subject_without_the_delegation_cap_is_denied_not_answered_for_the_caller ... ok
test bad_arguments_are_refused ... ok
test a_subject_resolves_only_inside_the_callers_workspace ... ok
test a_member_reaches_their_menu_sites_and_an_admin_reaches_all ... ok
test a_delegating_caller_gets_the_subjects_scope_not_its_own ... ok
test a_membership_change_reaches_the_answer_at_once ... ok
test result: ok. 7 passed; 0 failed
$ cargo test -p lb-host --lib -- authz catalog
test result: ok. 27 passed; 0 failed
$ cargo test -p lb-host --lib grant_role_tiers
test result: ok. 3 passed; 0 failed   (a_bare_tool_name_… FAILS on the old line)
```

Capability-deny (no verb cap; `subject` without the delegation cap) and workspace isolation are
both covered.

## Debugging

- [A `[[tools]] role` tier on a bare tool name grants nothing](../../debugging/auth-caps/role-tier-on-bare-tool-name-grants-nothing.md)
  — found while driving the first consumer live; `grant_role_tiers` now qualifies the tool name the
  way the host gates it. Regression test fails before, passes after.

## Public / scope updates

`doc-site/content/public/auth-caps/auth-caps.md` — new section. Scope: the "Extensions ask the same
resolver" subsection of `entity-scoped-data-scope.md`.

## Follow-ups

- `data:<table>:read` cannot be assigned through `grants.assign` (no `data` surface in the cap
  grammar). The `grant` source therefore only ever holds grants written by a system path. Decide
  whether admins should be able to assign it, or whether menus are the only intended source.
