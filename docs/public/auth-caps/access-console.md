# The Access console

The access-first admin surface — the evolution of the shipped `/admin` console from a flat five-tab
directory into an **access-management tool**. An admin *sees* the access graph (resolved effective
caps per subject, with provenance), *changes* it through guided flows (a catalog-driven no-widening
picker), and *understands the consequence and timing* of every change (a live-token revoke lever on
top of the shipped grant-revoke). Closes the three backend gaps that blocked it: resolved effective
caps **with provenance**, a **live-token** revoke, and **`roles.delete`**.

Scope: [`../../scope/auth-caps/access-console-scope.md`](../../scope/auth-caps/access-console-scope.md)
· Session: [`../../sessions/auth-caps/access-console-session.md`](../../sessions/auth-caps/access-console-session.md).

## What shipped

**Three new verbs, wired store → cap → MCP → gateway → `http.ts` → UI:**

- **`authz.resolve`** (`mcp:authz.resolve:call`, admin-only) — a subject's resolved effective caps
  **WITH provenance**: each cap tagged with where it came from (`direct` / `role:<r>` / via
  `team:<t>`). It is the **provenance-tagging wrapper over the one shipped `resolve_caps`/
  `resolve_subject_caps` fold** — NOT a parallel resolver — so the displayed set and the enforced
  (token) set cannot drift (a cross-check test pins it: `resolve_caps_sourced` cap set ==
  `resolve_caps`). `GET /admin/authz/resolve?subject=…`.
- **`authz.revoke-tokens`** (`mcp:authz.revoke-tokens:call`, admin-only) — the **live-token revoke
  lever**. Writes a per-`(ws, subject)` **`token_revoke` tombstone** the verify chokepoint reads on
  every request, so the subject's *current* (cached) token is refused on the next request (single-node
  instant), AND **composes** with the shipped `revoke_subject` (tombstones grants → next re-mint) for a
  full immediate lockout. `POST /admin/authz/revoke-tokens`. Multi-node worst case bounded by TTL —
  stated honestly in the UI, never "instant global".
- **`roles.delete`** (`mcp:roles.manage:call`, admin-only) — cascade-un-assigns `role:<name>` from
  every subject AND deletes the role in **one store transaction** (a new bounded `write_batch`). Built-in
  roles (`super-admin`/`workspace-admin`/`member`) are immutable (clear `400`). Idempotent.
  `DELETE /admin/roles/{name}`.

**The verify-path check** (`role/gateway/src/session/authenticate.rs`): after the signature/expiry
check, `verify_token` reads the `token_revoke` marker for `(ws, subject)`; a marked subject's bearer is
refused as an opaque `401` (indistinguishable from a genuinely expired credential — no oracle). One
read per request, workspace+subject keyed.

**The UI (shadcn-first rebuild):**
- An **Access overview** landing (tiles: People/Teams/Roles/Keys counts, direct-grant subjects, keys
  expiring <7d, subjects holding an admin cap — honest counts; a tile whose source verb/cap is absent
  is hidden with a reason, never a fake 0).
- **Effective caps with provenance** in the People detail (each cap tagged direct / role:r / via team:t).
- A **guided capability picker** (catalog-driven over `tools.catalog`, already caller-cap-filtered →
  no-widening; emits canonical `mcp:<tool>:call`). Raw-string entry stays as an "Advanced" escape hatch.
- A **live-token revoke lever** ("Apply now — end active sessions") on revoking actions, with honest
  timing copy.
- New shadcn primitives: `tabs`, `table`, `select` (token-bound). `AdminView` + `AccessEditor` fully
  migrated off the legacy control layer.

## Decisions (the scope's five open questions, resolved)

1. **Live-token revoke** → per-`(ws, subject)` tombstone RECORD the verify path checks (not a nonce
   bump, not a global list); composes with `revoke_subject`.
2. **"Who can do X"** → client-side for v1 (no `who_has` verb).
3. **Overview tiles** → the security-posture set above; provenance/last-changed out.
4. **`roles.delete`** → cascade-un-assign in one tx; built-ins immutable.
5. **Provenance fields** → no `last_changed_by/at`; source comes from the resolver fold. Audit is
   `scope/audit/`.

## Tests (real infra, no mocks)

- `lb-authz` `access_console_test` (5): no-drift cross-check, provenance tags, key resolve,
  token_revoke round-trip, role_delete cascade idempotent.
- `lb-host` `authz_test` (+2): per-verb deny + ws-iso at the MCP bridge.
- `lb-role-gateway` `access_console_routes_test` (5): forged-call deny, resolve provenance, **the
  headline** (revoke_tokens refuses the prior token on the next verify), roles.delete cascade +
  built-in reject + idempotent, ws-iso.
- UI `AccessConsole.gateway.test.tsx` (6) + `RolesAdmin` delete (real gateway); `pnpm test` 168/168,
  `pnpm lint` 0 errors, `tsc` clean, `pnpm build` green.

## Follow-ups

- Full shadcn migration of the remaining admin view *bodies* (People/Roles/Teams/Workspaces/ApiKeys
  still use raw `<table>`/`<input>`; they carry the new features but stay in `LEGACY_VIEWS`).
- A pre-delete "affected count" preview for `roles.delete` (v1 reports it post-delete).
- Optional `bus.watch` "access changed" motion for multi-admin liveness (a named scope follow-up).
