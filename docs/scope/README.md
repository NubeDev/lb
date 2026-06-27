# Scope docs

Pre-work briefs: the *ask* for each feature area, written before implementation (see
`../SCOPE-WRITTING.md`). One topic folder per area; one `<name>-scope.md` per ask within it.
A feature reads top-to-bottom across folders: `scope/<topic>/` → `sessions/<topic>/` →
`public/<topic>/`.

## Topics

- `agent/` — the central, workspace-scoped AI agent (S5).
- `ai-gateway/` — the swappable model-access sidecar (S5).
- `observability/`, `audit/`, `undo/` — the **three cross-cutting projections of the host dispatch
  chokepoint** (README §6.5/§6.6), scoped together as the S10 retrofit: `observability/` (structured
  logs + distributed traces + metrics, emitted everywhere with a `trace_id` that survives the routed
  hop), `audit/` (an immutable, hash-chained, workspace-walled ledger of every allow/deny — generalizes
  §6.14's model-call audit), and `undo/` (a reversible-command journal whose hard line is *reverse
  state, compensate motion*). See "The shared seam" in `observability/observability-scope.md`.
- `auth-caps/` — the capability grammar, token, and grant delegation; plus `edge-trust-scope.md` (node
  enrollment/cert + mTLS + token-on-the-bus), `authz-grants-scope.md` (durable roles/grants/teams —
  restricted user/team access), and `admin-crud-scope.md` (the destructive half — workspace/user/team/
  member delete·disable·remove·rename + dev-store user CRUD).
- `bus/` — the Zenoh message bus (motion).
- `coding-workflow/` — the S6 worked example: issue → triage → approval → job → outbox.
- `core/`, `crate-layout/`, `extensions/`, `mcp/`, `node-roles/`, `registry/`, `secrets/`,
  `store/`, `tags/`, `tenancy/` — the spine and platform surfaces. `extensions/` also holds
  `lifecycle-management-scope.md` (the full start·stop·enable·disable·upload·install·delete lifecycle
  exposed over the gateway, not Tauri-only) and `ui-federation-scope.md` (mount an extension's OWN
  pages inside the shell — module federation for trusted publishers, iframe/Web Component sandbox for
  untrusted, host-mediated MCP bridge; the deferred counterpart to the admin console), and
  `proof-panel-scope.md` (one self-contained **Tier-1 WASM** reference extension — a real MCP tool +
  a federated page reading real series through the bridge — proving the basics end-to-end with no
  placeholders; the wasm sibling of the native `fleet-monitor`).
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
- `frontend/` — the React/Tauri UI shell; `collaboration-scope.md` (the real multi-user app),
  `admin-console-scope.md` (the management UI for workspaces·teams·users·members·extensions), and
  `dashboard-scope.md` (the grid-of-widgets dashboard over real series — Phase 1 first-party/seeded,
  with the full asset-sharing authz model; Phase 3 the real edge fleet; the `vision/0003` IoT dashboard
  made buildable), and `dashboard-widgets-scope.md` (Phase 2 — widgets as installed extensions: how a
  widget accesses data through the host-mediated read-only bridge without ever holding the token or
  touching the DB, trust tiers, the `[widget]` manifest), and `data-console-scope.md` (the workspace
  data console: an admin-gated raw table browser + react-flow graph view, and an ingest/series explorer
  with manual write — the raw exploratory counterpart to the dashboard, for users who aren't good at SQL).
- `testing/`, `debugging/` — the standards every session follows.

See `../STAGES.md` for which stage each area lands in and `../STATUS.md` for what has shipped.

