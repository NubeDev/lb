# Scope docs

Pre-work briefs: the *ask* for each feature area, written before implementation (see
`../SCOPE-WRITTING.md`). One topic folder per area; one `<name>-scope.md` per ask within it.
A feature reads top-to-bottom across folders: `scope/<topic>/` → `sessions/<topic>/` →
`public/<topic>/`.

## Topics

- `agent/` — the central, workspace-scoped AI agent (S5).
- `ai-gateway/` — the swappable model-access sidecar (S5).
- `auth-caps/` — the capability grammar, token, and grant delegation; plus `edge-trust-scope.md` (node
  enrollment/cert + mTLS + token-on-the-bus) and `authz-grants-scope.md` (durable roles/grants/teams —
  restricted user/team access).
- `bus/` — the Zenoh message bus (motion).
- `coding-workflow/` — the S6 worked example: issue → triage → approval → job → outbox.
- `core/`, `crate-layout/`, `extensions/`, `mcp/`, `node-roles/`, `registry/`, `secrets/`,
  `store/`, `tags/`, `tenancy/` — the spine and platform surfaces.
- `files/`, `skills/`, `document-store/` — shared workspace assets (S4).
- `inbox-outbox/` — the normalized inbox (S2) and the transactional must-deliver **outbox**
  (`outbox-scope.md`, the S6 driver).
- `ingest/` — a generic buffered read/write surface for high-volume external data; the cloud-side
  ingest buffer (the read-side analog of the outbox). Stays domain-free — IoT is one caller (S9).
- `jobs/` — the SurrealDB-native durable job queue / resumable session (S5).
- `prefs/` — per-(workspace,user) preferences + localization: language (en/es), timezone, date/number
  display style, and a backend unit-conversion layer (metric/imperial). Canonical data in, localized
  presentation out, exposed as `format.*`/`convert.*` MCP tools so thin clients don't re-implement it.
- `sync/` — multi-node sync + authority (S3).
- `frontend/` — the React/Tauri UI shell.
- `testing/`, `debugging/` — the standards every session follows.

See `../STAGES.md` for which stage each area lands in and `../STATUS.md` for what has shipped.

