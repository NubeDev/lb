# Skills — versioned, grant-gated workspace assets (shipped S4)

The trimmed truth of what shipped. Full design: `../../scope/skills/skills-scope.md`; session:
`../../sessions/files/shared-assets-session.md`.

A **skill** is a reusable instruction/recipe asset that an AI agent **loads only when the workspace
has granted it** (README §6.12). Same asset substrate as docs (`../files/files.md`), plus two
skill-specific notions: **versioning** and the **grant gate**.

## Versioning

A skill is addressed `skill:{id}@{version}` and is **immutable per version** — a change is a new
version (`put_skill` rejects republishing an existing `{id}@{version}`). Rollback (§6.4) is loading
a prior version, whose record never went away. `load_skill(id, None)` resolves the latest published
version; `load_skill(id, Some(v))` pins.

## The grant gate (load only when granted)

Beyond the workspace + `store:skill/{id}:read` capability, `load_skill` checks a
**`grant:skill/{id}`** relation: the workspace saying "our agents may use this skill." No grant →
**denied**, even with the read capability — the §6.12 "load only when granted" rule. Granting is a
revocable relation, not a capability minted into a token: `grant_skill`/`revoke_skill` write/delete
one record, and the next `load_skill` reflects it immediately.

## Verbs (host)

- `put_skill(store, principal, ws, id, version, description, body, ts)` — publish an immutable
  version. Requires `store:skill/{id}:write`.
- `grant_skill` / `revoke_skill(store, principal, ws, id)` — the workspace grant relation.
- `load_skill(store, principal, ws, id, version?)` — the grant-gated load (cap + grant).

Reachable over MCP as `assets.put_skill` / `assets.grant_skill` / `assets.load_skill` via
`call_asset_tool`, behind the same MCP gate as docs.

## Tested

Capability-deny (no cap, and **cap-but-no-grant**), workspace-isolation, grant→load→revoke,
latest + rollback — `host/assets_skill_test`, `host/assets_isolation_test`,
`assets/tests/skill_version_test`.

**Exit-gate clause met:** a skill loads only when granted.
