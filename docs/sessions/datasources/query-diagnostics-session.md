# Session — per-target query diagnostics on `viz.query` (why a panel is blank)

Date: 2026-07-21 · Branch: `master` · Scope: `docs/scope/datasources/query-diagnostics-scope.md`

## The ask

`viz.query` resolved each panel target by re-entering the dispatcher under the caller's authority and,
**on any failure, collapsed the target to an empty frame and discarded the reason**
(`dispatch_target`: `Err(_) => Vec::new()`). Four different outcomes — a capability **deny**, a **bad
query** (the store/federation planner's rich message), an **operational error**, and a query that
genuinely **matched zero rows** — all reached the client as the same `{fields:[],length:0}` frame with
HTTP 200. A blank tile could not say *why* it was blank. The ask: attach a per-target `status`
(`ok`/`empty`/`denied`/`error`, with the tool's own message on `error`) while keeping a **capability
deny opaque**.

## What shipped

The one edit that mattered — match the `ToolError` **variant** instead of discarding it:

- **`rust/crates/viz/src/frame.rs`** — new plain-data types `FrameState` (`ok`/`empty`/`denied`/`error`)
  + `FrameStatus { state, message? }` with constructors `ok()/empty()/denied()/error(msg)`. `Frame`
  grows `status: Option<FrameStatus>`, `#[serde(default, skip_serializing_if = "Option::is_none")]` —
  absent ⇒ `None` (legacy/`ok`). Exported from `lb-viz` lib. `lb-viz` stays pure (no store/bus).
- **`rust/crates/host/src/viz/query.rs`** — `dispatch_target` now returns `(Vec<Value>, FrameStatus)`:
  - `Ok(out)` → parse rows; `empty` when 0 rows, else `ok`.
  - `Err(Denied) | Err(NotFound)` → `denied`, **no message** (listed FIRST so `Denied`'s `Display`
    can never leak in via a catch-all).
  - `Err(Extension(msg)) | Err(BadInput(msg))` → `error(msg)` — the caller's own query/args echoed
    back to the same authorized caller.
  - `Err(e)` (routing/reachability) → `error(e.to_string())` — carries only what the caller named.
  The resolver loop attaches the status onto each frame after `Frame::from_rows`.

Same verb, same `mcp:viz.query:call` cap — no new verb, no new capability. Additive field only.

## Testing (real node, no mocks — CLAUDE §9)

`rust/crates/host/tests/viz_query_test.rs` (extended) — all **17 green**:

- `status_ok_vs_empty` — ≥1 row → `{state:"ok"}`; a valid 0-row query → `{state:"empty"}`.
- `status_error_surfaces_message_frame_still_empty` — a bad SQL target → `error` with a non-empty
  `message`, frame still empty (no fabricated rows).
- `status_denied_is_opaque_with_no_enumeration_oracle` — a target without the cap → `{state:"denied"}`,
  message absent, **byte-identical** whether the underlying data exists or not (mandatory cap-deny + no
  enumeration oracle).
- `status_absent_on_frames_in_path` — the compute-only (frames-in) path resolves no target → no
  `status` field (legacy shape preserved).
- All pre-existing tests still pass unchanged (parity / legacy compat pinned).

`rust/crates/viz/src/frame.rs` unit tests — **2 green**: serde default (absent ⇒ `None`, not
re-serialized) and round-trip (`error` carries its message; `ok`/`empty`/`denied` do not).

**Negative control (scope-mandated):** temporarily reverted the match arm to `Err(_) => (vec![],
empty)` and confirmed `status_error_*` and `status_denied_*` both go **red** (`error`/`denied` collapse
to `empty`) — proof the tests pin the behavior, not the weather. Restored.

`cargo fmt` clean; `lb-viz` + `lb-host` build green.

## Safety notes confirmed before promote

- **Deny opacity** (the one place to get wrong): `denied` carries no `message` field at all, and the
  `Denied`/`NotFound` arm precedes any catch-all. The byte-identical enumeration-oracle test guards it.
- **No DSN/secret leak on `error`:** the federation DSN is written to `lb-secrets` and mediated into
  the sidecar (`federation/secret.rs`); the record stores only a ref and the sidecar's error text is
  the planner message (column/table names from the caller's own SQL), never the connection string. The
  `FederationError` constructors (`error.rs`) carry no DSN.
- **No frame cache-key thrash:** nothing under `crates/host/src/viz/` hashes a whole `Frame`; the
  editor's frames-in shape key hashes fields/values, not `status`.

## Open-question resolutions

1. **Status on the frame vs a sibling `statuses[]` map** → on the frame (co-located with its `refId`).
2. **`empty` vs `ok` for a 0-row success** → kept both (the UI writes a different explainer).
3. **Routing variants (`Ambiguous`/`NodeUnreachable`/`NodeTooOld`)** → surface their message via the
   catch-all (`error`); they carry only what the caller named. Not reachable on a single-node embed.

## Docs

- Promoted to `doc-site/content/public/datasources/datasources.mdx` — "Per-target query diagnostics
  (shipped 2026-07-21)".
- The consuming UI (empty-state/status-bar/inspector) is downstream: rubix-ai
  `docs/scope/frontend/dashboard/dashboard-first-paint-scope.md`.
