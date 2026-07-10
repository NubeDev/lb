# Reports — finish the feature (Task C durable role fix + A/B/D/E cleanup)

**Status:** done · **Date:** 2026-07-10 · **Scope:** `docs/scope/reports/report-builder-scope.md`
(its "Post-scope demo pass") + `docs/scope/auth-caps/builtin-role-freshness-scope.md` (new) ·
**Parent session:** the reports build (`HANDOVER-reports.md`).

The reports feature (report builder + branded PDF exporter) was built, compiled, and tests-green per
`HANDOVER-reports.md`. A demo pass added two deliberate changes (the TipTap editor was replaced with
the lazybones textarea editor; `PanelPicker` got starter widgets from the setup wizard's cell
builders). This session finished the four open items the demo pass left, per the user's task list A–E.

## A — Editor port: verify + drop the unused TipTap deps

The ported editor (`ui/src/components/markdown-editor/MarkdownEditor.tsx` = toolbar + Write/Preview
toggle; `MarkdownBody.tsx` = react-markdown + remark-gfm preview) is a faithful port of lazybones'
shipped `markdown-editor.tsx` / `markdown.tsx` — same `wrap`/`prefixLines` mechanics, same toolbar,
XSS-safe (no `rehype-raw`). It reads correctly in the report block card (`ReportEditor.tsx`) AND on the
A4 preview sheet (`ReportView.tsx`, which imports `a4SheetStyle`/`a4DeskStyle` from `a4-sheet.ts`).
**`a4-sheet.ts` is still used** (the preview geometry — the one source of truth matching the Typst
margins) — kept. The TipTap deps were unused (grep: 0 imports of `@tiptap/*` / `tiptap-markdown` /
prosemirror; the only `useEditor` hit was the unrelated `codeeditor/useEditorInsert`). Deleted from
`ui/package.json`: `@tiptap/react`, `@tiptap/starter-kit`, `@tiptap/pm`, `tiptap-markdown`; ran
`pnpm install` to refresh the workspace `pnpm-lock.yaml` (−572 lines, all `@tiptap/*` + their
prosemirror transitive deps). No editor code changed.

## B — Demo-shortcut coupling: lift the shared cell builders to `lib/panel/`

`PanelPicker` imported `timeseriesCell`/`templateCell`/`DEFAULT_SOURCE`/`DEMO_SQL` from
`features/admin/setup/dataToInsight.ts` and `TEMPLATE_GALLERY` from `templateGallery.ts` — a
cross-feature import purely to reuse the demo cells. The decision (confirmed with the user): **lift the
shared pieces into `lib/panel/`** (the panel client barrel), the cleaner cut per FILE-LAYOUT
(promote the helper when a second caller appears).

- New `ui/src/lib/panel/demoCells.ts` — `DEFAULT_SOURCE`, `DEMO_SQL`, `DEMO_PLOT`, `timeseriesCell`,
  `templateCell` (the shared demo `Cell` builders).
- New `ui/src/lib/panel/demoGallery.ts` — the starter template gallery (moved verbatim from
  `templateGallery.ts`): `TemplateExample`, `TEMPLATE_GALLERY`, `DEFAULT_TEMPLATE`.
- `ui/src/lib/panel/index.ts` re-exports both.
- `features/admin/setup/dataToInsight.ts` rewritten to keep ONLY the wizard-only artifacts
  (`DEMO_RULE`, `DEMO_DSN`, `DEMO_ENDPOINT`) and re-export the moved symbols from `@/lib/panel`, so the
  wizard imports (`DatasourceWizard`, `TemplateWidgetWizard`, `DatasourceStep`, `SqlPreviewStep`) stay
  green without edits.
- `features/admin/setup/templateGallery.ts` + its test DELETED; the test moved to
  `lib/panel/demoGallery.test.ts` (imports from `@/lib/panel`).
- `PanelPicker.tsx` imports from `@/lib/panel` — the cross-feature import is gone.
- `TemplateWidgetWizard.tsx` updated to import the gallery from `@/lib/panel`.

Rule 10 held (these are generic federation-query cells; `view` stays opaque data). FILE-LAYOUT held
(one responsibility/file: `demoCells.ts` vs `demoGallery.ts`).

## C — Durable role-reseed fix (the design decision + a scope note)

**Root cause** (the frozen built-in role row): `ensure_builtin_authz_roles`→`ensure_one` writes a role
row only when ABSENT, so a workspace seeded before a new built-in cap was added keeps the stale
`member`/`workspace-admin` rows forever. `resolve_caps` read that stored record to expand a `role:<name>`
grant, so a new built-in cap (e.g. `mcp:report.save:call`) never reached an already-seeded workspace's
tokens. The viewer tier dodged it only because the login floor (`credentials.rs`) calls the LIVE
`viewer_role_caps()`; author/admin caps rode the stored record. The throwaway `reseed_roles.rs` was a
symptom fix for the demo.

**Decision (confirmed with the user — "best long term"): union live built-in caps in the resolver.**
The resolver now UNIONS the live built-in bundle on top of the stored record for a granted built-in
role — a new built-in cap takes effect the moment code ships, no re-seed, no version bump. Because the
resolver lives in the pure `lb-authz` crate (which must not depend on `lb-host`, where the live
`*_role_caps()` bundles live), the live bundles are injected via a `BuiltinRoleCaps` callback:

- `crates/authz/src/resolve.rs` — `resolve_caps_with`/`resolve_subject_caps_with` take the callback;
  the zero-arg `resolve_caps`/`resolve_subject_caps` bake in `NoBuiltinRoleCaps` (unchanged behaviour).
  `BuiltinRoleCaps` trait + `NoBuiltinRoleCaps` impl.
- `crates/authz/src/resolve_sourced.rs` — the sourced twins (`_sourced_with`) for parity (the
  access-console shows live caps; the resolver↔mint cross-check stays exact).
- `crates/authz/src/lib.rs` — re-exports the new symbols.
- `crates/host/src/authz/builtin_caps.rs` (new) — `LiveBuiltinRoleCaps` maps the three built-in names
  to their authoritative `*_role_caps()`.
- `crates/host/src/authz/resolve_live.rs` (new) — `resolve_caps_live`/`resolve_subject_caps_live`, the
  canonical host entry points baking in `LiveBuiltinRoleCaps`.
- Every host caller switched to the live variants: the login mint (`role/gateway/src/routes/login.rs`),
  apikey auth/get, reminder fire, dashboard access_check, the access console (`authz_resolve`).
- UNION not REPLACE — an installed extension's `grant_assign(Subject::Role(name), cap)` is still
  honoured; custom roles (no live bundle) untouched.

The throwaway `rust/node/examples/reseed_roles.rs` + the now-empty `examples/` dir DELETED. Scope note:
`docs/scope/auth-caps/builtin-role-freshness-scope.md` (root cause + the fix + the rejected versioning
alternative + the invariant going forward). Debug entry: `docs/debugging/auth/builtin-role-row-frozen-stale-on-new-caps.md`.

**Regression test:** `crates/authz/tests/builtin_role_freshness_test.rs` (4 tests) pins both halves —
a STALE stored `member` row (missing `mcp:report.save:call`) + `resolve_caps` (no builtins) → the cap
is MISSING (the pre-fix bug); the same store + `resolve_caps_with` (+ live bundle) → the cap IS
resolved (the fix); plus `NoBuiltinRoleCaps` = raw, custom roles unaffected, and the union keeps
direct role-subject grants. The existing `sourced_cap_set_equals_resolve_caps_no_drift` cross-check
still passes.

## D — Tests

New UI unit coverage (15 tests):
- `components/markdown-editor/MarkdownEditor.test.tsx` (6) — edit round-trip (typing → onChange), Bold
  wraps the selection, Heading/Bullet-list prefix the line, Preview mode renders the body + disables
  the toolbar, `editable={false}` renders only the preview.
- `components/markdown-editor/MarkdownBody.test.tsx` (6) — renders headings/paragraphs/emphasis, GFM
  tables, lists, code; escapes raw HTML (XSS-safe); the `data-testid` container.
- `features/reports/PanelPicker.test.tsx` (3) — starter widgets appear (gallery minus "ai") + onPick
  fires with a renderable cell; library hydrate (getPanel → specToCell → onPick).

No mocks of node behavior (§9): the UI tests mock only the transport seam in jsdom; the cell builders,
`specToCell`, and the gallery run for real. Rust tests use the real store.

## Green output

```
cd rust && cargo fmt && cargo build --workspace && make build-wasm  (clean)
cargo test -p lb-render --lib                     → 24 passed
cargo test -p lb-host --test report_test          → 7 passed
cargo test -p lb-role-gateway --test report_routes_test → 4 passed
cargo test -p lb-authz                            → all green (incl. the 4 new + the cross-check)

cd ../ui && npx tsc --noEmit                      → clean
npx vitest run src/features/reports src/features/panel src/components/markdown-editor
                                                  → 20 files, 106 tests passed
```

## E — Docs

- This session doc.
- `docs/debugging/auth/builtin-role-row-frozen-stale-on-new-caps.md` + a row in
  `docs/debugging/README.md`.
- `doc-site/content/public/reports/reports.md` promoted from the TODO stub to the real feature write-up.
- `docs/STATUS.md` — Reports row added to "Slices in flight".
- `docs/skills/reports/SKILL.md` — grounded in a live `report.save` + export run.
- `docs/scope/reports/report-builder-scope.md` "Post-scope demo pass" updated to mark TipTap removal,
  the coupling unwound, and the durable role-freshness fix landed.
- `docs/scope/auth-caps/builtin-role-freshness-scope.md` (new) — the scope note for Task C.

## Files touched

**New:** `rust/crates/authz/tests/builtin_role_freshness_test.rs`, `rust/crates/host/src/authz/builtin_caps.rs`,
`rust/crates/host/src/authz/resolve_live.rs`, `ui/src/lib/panel/demoCells.ts`, `ui/src/lib/panel/demoGallery.ts`,
`ui/src/lib/panel/demoGallery.test.ts`, `ui/src/components/markdown-editor/MarkdownEditor.test.tsx`,
`ui/src/components/markdown-editor/MarkdownBody.test.tsx`, `ui/src/features/reports/PanelPicker.test.tsx`,
`docs/scope/auth-caps/builtin-role-freshness-scope.md`, `docs/debugging/auth/builtin-role-row-frozen-stale-on-new-caps.md`,
`docs/sessions/reports/reports-finish-session.md`, `docs/skills/reports/SKILL.md`.

**Edited:** `rust/crates/authz/src/{resolve.rs,resolve_sourced.rs,lib.rs}`,
`rust/crates/host/src/{authz/mod.rs,authz/resolve.rs,lib.rs,dashboard/access_check.rs,reminder/fire.rs,apikey/get.rs,apikey/auth.rs}`,
`rust/role/gateway/src/routes/login.rs`, `ui/package.json`, `pnpm-lock.yaml`,
`ui/src/lib/panel/index.ts`, `ui/src/features/admin/setup/{dataToInsight.ts,TemplateWidgetWizard.tsx}`,
`ui/src/features/reports/PanelPicker.tsx`, `doc-site/content/public/reports/reports.md`,
`docs/STATUS.md`, `docs/debugging/README.md`, `docs/scope/reports/report-builder-scope.md`.

**Deleted:** `rust/node/examples/reseed_roles.rs` (+ `examples/`), `ui/src/features/admin/setup/templateGallery.ts`,
`ui/src/features/admin/setup/templateGallery.test.ts`.

No git commit (per the user's instruction — awaiting test & approve).
