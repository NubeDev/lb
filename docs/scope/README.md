# Scope docs

Pre-work briefs: the *ask* for each feature area, written before implementation (see
`../SCOPE-WRITTING.md`). One topic folder per area; one `<name>-scope.md` per ask within it.
A feature reads top-to-bottom across folders: `scope/<topic>/` → `sessions/<topic>/` →
`public/<topic>/`.

## Topics

- `agent/` — the central, workspace-scoped AI agent (S5).
- `ai-gateway/` — the swappable model-access sidecar (S5).
- `auth-caps/` — the capability grammar, token, and grant delegation.
- `bus/` — the Zenoh message bus (motion).
- `coding-workflow/` — the S6 worked example: issue → triage → approval → job → outbox.
- `core/`, `crate-layout/`, `extensions/`, `mcp/`, `node-roles/`, `registry/`, `secrets/`,
  `store/`, `tags/`, `tenancy/` — the spine and platform surfaces.
- `files/`, `skills/`, `document-store/` — shared workspace assets (S4).
- `inbox-outbox/` — the normalized inbox (S2) and the transactional must-deliver **outbox**
  (`outbox-scope.md`, the S6 driver).
- `jobs/` — the SurrealDB-native durable job queue / resumable session (S5).
- `sync/` — multi-node sync + authority (S3).
- `frontend/` — the React/Tauri UI shell.
- `testing/`, `debugging/` — the standards every session follows.

See `../STAGES.md` for which stage each area lands in and `../STATUS.md` for what has shipped.

