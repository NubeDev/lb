# Session — insight + occurrence delete (cascade)

## Ask

The Insights UI had no way to delete anything. Add:
- **Delete an insight** → cascades, deleting all its occurrences (transactions).
- **Delete a single occurrence** (transaction) on its own.

## Decisions

- **Occurrence delete leaves the parent's lifetime `count`/`first_ts`/`last_ts` untouched.**
  `count` is the monotone lifetime firing total (it can already exceed stored ring rows — occurrences
  scope), so deleting an evidence row just removes it from the ring; it does not rewrite history.
- **Two new member-grantable caps** (`mcp:insight.delete:call`, `mcp:insight.occurrence.delete:call`),
  added to `AUTHOR_CAPS` (not viewer): erasing shared content + evidence is an authoring reach a bare
  viewer must not have. Role-bundle unit tests pin viewer=none / member=all.

## Change

Two new verbs, each one-responsibility-per-file, funnelled through the existing gated seams.

**Crate `lb_insights`:**
- `delete.rs` — `delete(store, ws, id)`: bulk-`DELETE` the ring (`insight_occ WHERE insight_id`) FIRST
  (no orphan evidence), then `store::delete` the parent (`insight:<id>`). Idempotent.
- `occ_delete.rs` — `delete_occurrence(store, ws, insight_id, oseq)`: `DELETE ... WHERE insight_id AND
  oseq` (workspace+parent scoped so a sequence can't reach another insight's row). Idempotent.

**Host `lb_host::insight`:** `delete.rs` (`insight_delete`, gate `insight.delete`) + `occ_delete.rs`
(`insight_occurrence_delete`, gate `insight.occurrence.delete`). Wired into `mod.rs`, `tool.rs`
(dispatch arms), `system/catalog.rs` (catalog entries), `lib.rs` re-exports.

**Caps:** both added to `authz/builtin_roles.rs::AUTHOR_CAPS`.

**Gateway REST parity:** `DELETE /insights/{id}` (`delete_insight`) + `DELETE
/insights/{id}/occurrences/{oseq}` (`delete_occurrence`) in `routes/insight.rs`, mounted in `server.rs`.
(The UI drives the MCP bridge; the REST routes are parity + tested.)

**UI:**
- `insights.api.ts` — `deleteInsight(id)`, `deleteOccurrence(insightId, oseq)` over the generic
  `mcp_call` IPC (no per-tool http.ts mapping needed — `mcp_call` is a generic passthrough).
- `InsightActions.tsx` — a destructive "Delete" button (confirm dialog: "…and all N occurrences"),
  `onDeleted` prop.
- `InsightDetail.tsx` — per-occurrence trash button (hover-reveal, spinner, optimistic drop + refetch);
  threads `onDeleted`.
- `InsightsPage.tsx` — `onDeleted` closes the pane + refreshes the list.

## Tests (green)

- `host/tests/insights_test.rs` (+5): `delete_denied_without_the_cap`,
  `occurrence_delete_denied_without_the_cap` (not implied by the occurrences READ cap),
  `delete_removes_the_insight_and_cascades_its_occurrence_ring` (+ idempotent),
  `occurrence_delete_removes_one_row_and_leaves_count_untouched`,
  `delete_in_one_workspace_cannot_reach_another_workspaces_insight` (hard wall §7). 20/20 pass.
- `role/gateway/tests/insight_routes_test.rs` (+2): `delete_route_removes_the_insight_and_its_occurrences`,
  `occurrence_delete_route_removes_one_row`. 6/6 pass.
- `authz::builtin_roles` unit tests: viewer holds neither delete cap, member holds both. 6/6 pass.
- `ui` `tsc --noEmit` clean; `InsightsPage.gateway.test.tsx` 4/4 against a real spawned gateway.
- `cargo build --workspace`, `cargo fmt`, `cargo clippy -p lb-insights -p lb-host` (no new warnings).
