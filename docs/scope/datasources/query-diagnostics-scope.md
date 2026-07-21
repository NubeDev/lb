# Datasources scope — per-target query diagnostics on `viz.query` (why a panel is blank)

Status: **SHIPPED 2026-07-21** (`rust/crates/host/src/viz/query.rs` + `rust/crates/viz/src/frame.rs`;
tests in `rust/crates/host/tests/viz_query_test.rs`; session
`docs/sessions/datasources/query-diagnostics-session.md`). Promoted to
`public/datasources/datasources.mdx`. A rubix-ai (or any embedder) UI now bumps the `lb-node` tag and
consumes the per-frame `status` — see the sibling rubix-ai scope
`docs/scope/frontend/dashboard/dashboard-first-paint-scope.md`.

`viz.query` resolves a panel's targets by re-entering the dispatcher under the caller's authority and,
**on any failure of a target, collapses it to an empty frame and discards the reason**
(`dispatch_target`: `Err(_) => Vec::new()`, `query.rs:236-242`). So four very different outcomes —
a **capability deny**, an **unknown datasource/tool**, a **bad query** (the federation planner's rich
`Schema error: No field named nonexistent_col. Valid fields are …`), and **a query that genuinely
matched zero rows** — all reach the client as the **same** `{fields:[],length:0}` frame with HTTP 200.
The result: a blank panel that cannot say why it is blank, which is the single biggest "is this broken?"
papercut in dashboard building. We want `viz.query` to attach a **per-target status** to each frame that
distinguishes *ok / empty / denied / error(message)* — **surfacing a query author's own diagnostic while
keeping a capability deny opaque** — so the render and builder UIs can finally explain a dark tile.

## Goals

- Each frame `viz.query` returns carries a `status` — one of `ok`, `empty`, `denied`, `error` — plus, for
  `error`, the **downstream tool's own message** (the federation/planner text, a `bad input` reason).
- **Deny stays opaque.** A `ToolError::Denied` (or `NotFound`) target reports `status:"denied"` with **no
  message** — it must never reveal a gate, a tool's existence, or cross-workspace shape (mcp scope, the
  `Denied`-carries-no-detail rule the error enum documents).
- **Zero fabrication, unchanged rows.** `status` is additive metadata beside the existing `frames`/`rows`;
  a failed target still yields an empty frame (no invented rows), and every existing caller that ignores
  `status` renders exactly as today.
- **Same verb, same cap.** No new verb, no new capability — the diagnostic rides the existing
  `mcp:viz.query:call` result. The frames-in (compute-only) and stepwise-debug paths are unaffected.
- A gateway test proves the four outcomes are now distinguishable, **and** that a denied target's status
  is byte-for-byte identical whether or not the named source exists (no enumeration oracle).

## Non-goals

- **Not** changing `federation.query`/`store.query`/`series.read` themselves — they already return rich
  errors (federation surfaces the planner message today; §"What's already right"). This scope stops the
  **aggregator** (`viz.query`) from throwing that detail away.
- **Not** surfacing a per-*row* or per-*field* error, and not a partial-frame ("first 3 rows then failed")
  model — a target either produced rows or it did not.
- **Not** a UI change — the consuming status bar / empty-explainer / inspector is the rubix-ai scope. This
  scope's deliverable ends at the wire shape + the host that fills it.
- **Not** touching the deny-opacity contract anywhere else; this is one match arm in one file.

## Intent / approach

### The one edit that matters: match the error **variant**, don't discard it

`dispatch_target` currently returns `Vec<Value>` (the rows) and swallows the error. Change it to return a
small result that carries **both** the rows and a resolved status, by matching `ToolError` on its variant
rather than `Err(_)`:

```rust
enum TargetStatus { Ok, Empty, Denied, Error(String) }

// in dispatch_target, replacing `Err(_) => Vec::new()`:
match dispatched {
    Ok(out) => { let rows = ...; (rows, if rows.is_empty() { Empty } else { Ok }) }
    // Opaque by design — no message, indistinguishable from a missing tool (mcp scope).
    Err(ToolError::Denied) | Err(ToolError::NotFound) => (vec![], Denied),
    // The caller's OWN query/args being validated — safe (and vital) to surface.
    Err(ToolError::Extension(msg)) | Err(ToolError::BadInput(msg)) => (vec![], Error(msg)),
    // Operational (routing/reachability) — surface the message; these already carry no fleet secret
    // beyond what the caller named (see the error enum's per-variant notes).
    Err(e) => (vec![], Error(e.to_string())),
}
```

The safety argument is the `ToolError` enum's own doctrine: `Denied` is the **only** variant defined to be
detail-free, precisely so an unauthorized caller learns nothing. `Extension`/`BadInput` are, by
construction, the *downstream tool answering the caller's own request* — the bad SQL the author typed, the
malformed arg they passed. Surfacing those to the same authorized caller leaks nothing they did not send;
withholding them is the actual bug.

### The wire shape (additive)

`viz.query` already returns `{ frames, rows }`. Add `status` **onto each frame** (the frame already
carries `refId`, so status is naturally per-target) and keep a flattened top-level convenience mirror is
**not** needed — a caller indexes by `refId`. A `Frame` grows one serde-defaulted field:

```jsonc
{ "refId": "A", "fields": [...], "length": 0,
  "status": { "state": "error",
              "message": "Schema error: No field named nonexistent_col. Valid fields are histories.point_uuid, histories.value, histories.timestamp." } }
```

- `state: "ok"` — `message` absent.
- `state: "empty"` — ran, 0 rows; `message` absent (the UI writes its own "0 rows for <range>").
- `state: "denied"` — `message` **always absent** (opaque).
- `state: "error"` — `message` present, the downstream tool's text.

`status` is `#[serde(default)]` (absent ⇒ treated as `ok`/legacy) so an old client and the frames-in path
need no change, and a `viz.query` responder stub that omits it still validates.

### Where it lives (one responsibility per file)

`TargetStatus` + the variant mapping is small enough to sit in `viz/error.rs` (which already owns
`VizError`) or a new `viz/status.rs` if `error.rs` would cross the size line — the session decides by line
count (FILE-LAYOUT). `query.rs`'s loop changes from pushing `Frame::from_rows(...)` to pushing a frame
**with** its status; `Frame` (in the pure `lb-viz` crate) gains the `status` field. Keep `lb-viz` pure —
`status` is plain data, no store/bus reach.

## How it fits the core

- **Capabilities & the deny path:** the whole point. The deny path is *preserved verbatim* (empty frame +
  opaque status); the change only stops non-deny errors from being silently laundered into that same
  shape. Reviewed against the mcp scope's "`Denied` reveals nothing" rule — `denied` status carries no
  message, no tool name, no existence signal.
- **Workspace isolation:** unchanged. Targets still dispatch under the token's workspace via
  `call_tool_at_depth`; a cross-ws read is still denied and now reports `denied` (no leak — same opaque
  status a same-ws deny gives). Add the isolation assertion to the test anyway.
- **MCP surface (the API shape):** one **additive** field on the `viz.query` result frame. No new verb,
  no get/list/watch/batch change — N/A. Rule 10 holds: `viz.query` stays the one generic verb; it treats
  every target tool opaquely, reading only the *variant* of the error the dispatcher already returns, not
  special-casing `federation` or any named source.
- **State vs motion / datastore / bus / secrets:** N/A — read-path metadata only. A `federation` error
  message could in principle echo a table/column name the caller referenced; that is the caller's own SQL,
  not a secret, and it never includes the DSN (the extension redacts connection detail in its own error).
  The session confirms the federation error text carries no credential before promoting.
- **SDK/WIT impact:** none — `viz.query` is a host verb, not an extension ABI. The `Frame` type is
  internal to `lb-viz` + the host; its JSON grows one optional field.
- **Skill/doc:** update the `viz.query` skill/reference with the `status` field and the deny-opacity note
  (a skill author must know `denied` is intentionally message-less).

## Example flow

1. A panel has two targets: `A` = `federation.query` with a typo'd column, `B` = `store.query` the caller
   isn't granted.
2. `viz.query` dispatches both under the token. `A` → the federation extension returns
   `ToolError::Extension("Schema error: No field named …")`; `B` → `ToolError::Denied`.
3. The result: `frames:[ {refId:"A", fields:[], length:0, status:{state:"error", message:"Schema error: No field named …"}}, {refId:"B", fields:[], length:0, status:{state:"denied"}} ]`.
4. The UI renders `A`'s message inline ("fix your SQL") and `B` as a plain "no access to this source" — the
   author instantly knows one is their bug and one is a permission, where today both were an identical
   silent blank.
5. Fixed SQL + a granted `B` → both frames `status:"ok"`; a correct-but-empty range → `status:"empty"`,
   and the UI writes "0 rows for <from>–<to>".

## Testing plan

Per `docs/scope/testing/testing-scope.md` — real gateway, real store, no fakes:

- **Gateway (`viz_query_test.rs`, extend):**
  - **bad query → `error` with message**: a `federation.query`/`store.query` target with an unknown column
    returns `status:"error"` and a non-empty `message` containing the planner text; the frame is still
    empty (no fabricated rows).
  - **deny → `denied`, no message (mandatory cap-deny)**: a target the token lacks the cap for →
    `status:"denied"`, `message` **absent**; assert the response is byte-identical whether the named source
    exists or not (no enumeration oracle).
  - **empty vs ok**: a valid query matching 0 rows → `status:"empty"`; matching ≥1 → `status:"ok"`.
  - **workspace isolation (mandatory)**: authed to `beta`, target a `demo-buildings` source seeded in
    `acme` → `denied` (not `error`, not rows) — the wall holds and the status doesn't leak that `acme`
    has the source.
  - **legacy/compat**: the frames-in (compute-only) path and the stepwise-debug path still return their
    existing shapes; a client that ignores `status` gets identical `frames`/`rows` to today (pin with the
    existing assertions before the change).
- **Unit (`lb-viz`)**: `Frame` serde round-trips with and without `status`; `status` absent deserializes as
  the legacy/`ok` default.
- **Negative control**: revert the match arm to `Err(_) => Vec::new()` and watch the four-outcome test go
  red — proof it pins the behavior, not the weather.

## Risks & hard problems

- **Deny-opacity is the one place to get wrong.** A careless `Err(e) => Error(e.to_string())` catch-all
  would turn `Denied`'s `Display` ("denied") into a *message* — harmless text, but the discipline is that
  `denied` status carries **no** message field at all. The match must list `Denied`/`NotFound` explicitly
  **before** any catch-all. The test's byte-identical assertion is the guard.
- **Error message content.** A downstream extension error could be verbose or, in theory, echo input. It is
  the caller's own input echoed back to the same caller — acceptable — but the session must confirm no
  federation error path includes the DSN/secret (grep the `federation` error constructors) before promote.
- **`query.rs` size.** The file is ~280 lines; the status mapping must land as a helper (new `status.rs` or
  into `error.rs`), not inline bloat pushing `query.rs` toward the 400 cap.
- **Frame type churn.** `Frame` lives in `lb-viz` and is serialized widely (cache keys, inspector). Adding
  a serde-default field is safe, but confirm no cache key hashes the whole `Frame` including `status` in a
  way that would thrash the editor's frames-in shape key (it hashes fields/values, not status — verify).

## Open questions — RESOLVED (shipped)

1. **Status on the frame vs a sibling `statuses[]` map?** → **On the frame.** `status: Option<FrameStatus>`
   sits beside `refId`; no second refId-keyed structure to join. No consumer reads frames without refId.
2. **`empty` vs `ok` for a 0-row success** → **Both kept.** `ok` = ≥1 row, `empty` = ran/0 rows, set by
   row count so the UI can write its own "0 rows for &lt;range&gt;" explainer.
3. **Routing variants (`Ambiguous`/`NodeUnreachable`/`NodeTooOld`)** → **Surface their message** via the
   catch-all `Err(e) => error(e.to_string())`; each carries only what the caller named. Not reachable on
   the common single-node embed.

## Related

- `rust/crates/host/src/viz/query.rs` — `dispatch_target` (`:205-243`), the swallow site; `viz_query`
  (`:40-110`) the loop that builds frames.
- `rust/crates/mcp/src/call/error.rs` — the `ToolError` enum whose variants make the deny/error split safe.
- `rust/crates/viz/src/frame.rs` — `Frame`, which grows the `status` field.
- `docs/scope/datasources/datasources-scope.md`, `federation-pushdown-scope.md` — the federation source
  whose rich errors this stops discarding.
- `docs/scope/viz/grafana-parity-backend-scope.md` — the `viz.query` backend this refines.
- **rubix-ai** `docs/scope/frontend/dashboard/dashboard-first-paint-scope.md` — the UI that consumes the
  new `status`, and the render-time honesty this unblocks.
- `docs/scope/frontend/dashboard/viz/` (rubix-ai) `data-studio-ux-scope.md` — the *builder* status bar that
  already wanted this; it surfaced only bridge-thrown errors, never the swallowed per-target ones.
