# Tabularis harvest — what else is worth taking, and the verdict on each

Status: reference (an advisory companion to [`query-builder-10x-scope.md`](query-builder-10x-scope.md)).
Not a scope; a decision list. Each item: what it is, where it lives in Tabularis, how it maps onto *our*
architecture, and a **take-now / scope-later / skip** verdict with the reason.

> **The governing lens.** Tabularis is a single-user **desktop** SQL client (Tauri, local drivers, files
> for state). Lazybones is a **multi-tenant platform** (workspace wall, capabilities, one datastore, MCP
> contract, extensions). So the rule for every steal is: *take the interaction design and the pure-TS
> utilities; re-implement anything that touches data through our gated seams; skip anything that assumes a
> single local user or a second datastore.* Tabularis is Apache-2.0 — ported code carries a per-file
> attribution header (see the editor slice §Licensing).

## Already covered by the three slices

| Tabularis feature | Slice |
|---|---|
| Drag-and-connect visual JOIN builder (React Flow) | [visual-canvas-builder](visual-canvas-builder-scope.md) |
| Schema-aware SQL autocomplete | [sql-editor-10x](sql-editor-10x-scope.md) |
| Dialect-aware multi-statement splitter (`sqlSplitter`) | [sql-editor-10x](sql-editor-10x-scope.md) — **pure copy** |
| SQL formatter (`sql-formatter`) | [sql-editor-10x](sql-editor-10x-scope.md) |
| A standalone SQL workspace surface | [query-workbench-view](query-workbench-view-scope.md) |

## Take-now — small, pure, high-leverage (fold into the slices or a quick follow)

- **`sqlSplitter/` (pure TS, zero coupling).** Already in slice 2. Reusable far beyond the builder — any SQL
  surface. *The single best steal.* `/tmp/tabularis/src/utils/sqlSplitter/`.
- **Clipboard tabular import with type inference.** Paste TSV/CSV → infer column types → results grid (and,
  for us, optionally an `ingest`/`store` write). The *type-inference parser* (`/tmp/tabularis/src/utils/clipboardParser.ts`)
  is pure and liftable; the write path must go through our gated ingest/store verbs, not Tabularis's direct
  driver insert. **Verdict: scope-later, small** — a nice ingest-adjacent utility; pairs with the data
  console.
- **Fuzzy filter (`fuse.js` wrapper).** `/tmp/tabularis/src/utils/fuzzy.ts` — for the datasource/table
  picker and a future quick-navigator. Trivial, dependency is small. **Verdict: take-now if the picker's
  substring filter feels weak; otherwise skip** (don't add a dep speculatively).

## Scope-later — genuinely valuable, each its own scope, maps cleanly onto our seams

- **SQL Notebook (SQL + Markdown cells, cross-cell CTE variables).** Tabularis's notebook chains cells by
  rewriting `{{cell_N}}` into injected CTEs (`/tmp/tabularis/src/utils/notebookVariables.ts`) — a clever,
  engine-agnostic way to pipeline queries. This is a **strong fit as a new Data Studio view kind**
  ("Notebook", alongside the Query workbench) and a natural companion to the insights work (a notebook is
  how an analyst *derives* an insight). Our version: cells run through `store.query`/`federation.query`;
  cell results feed the next cell as a CTE (or a temp frame); persistence is a SurrealDB record (like
  `ui_layout`), member-owned. **Verdict: scope-later — the highest-value "next thing" after the builder.**
  Cross-links: `docs/scope/insights/`, the Data Studio pages-as-panes registration.
- **Visual EXPLAIN (query-plan graph).** Tabularis normalizes each driver's EXPLAIN into a common
  `ExplainPlan` tree and renders it as a React-Flow node graph, heat-colored by cost/time
  (`/tmp/tabularis/src/utils/explainPlan.ts`, `types/explain.ts`). We'd need a host/sidecar `*.explain` verb
  (surreal `EXPLAIN`, federation/DataFusion `EXPLAIN`), then reuse the exact React-Flow renderer pattern
  (we already have `@xyflow/react`). **Verdict: scope-later — excellent for the data console / query
  workbench "Explain" tab; needs one new gated verb per engine.** A real feature, not a v1.
- **Rich results DataGrid (tanstack-table + tanstack-virtual).** Virtualized, sortable, inline-editable,
  typed cell renderers (JSON/BLOB/geometry/date), CSV/JSON/SQL-INSERT export
  (`/tmp/tabularis/src/components/ui/DataGrid.tsx` + `utils/{dataGrid,clipboard}.ts`). Our query workbench
  (slice 3) uses the shipped table render path for v1; this is the upgrade. Inline *edit* must go through
  gated write verbs (we're SELECT-only today — editing is a separate authz story). **Verdict: scope-later —
  read-only virtualized grid + export first; inline edit is its own capability + scope.**
- **Remappable keybindings.** `/tmp/tabularis/src/config/shortcuts.json` + `useKeybindings`, persisted.
  Maps onto our `prefs` (member-owned). **Verdict: scope-later, low priority — a platform-wide nicety, not
  builder-specific.**
- **Detached/secondary windows (results, JSON viewer) with persisted geometry.** Tauri multi-window. We have
  the Dockview multi-pane workbench that already covers most of this need in-window. **Verdict: skip for the
  browser; maybe-later for the Tauri desktop build only** — Dockview panes are the platform-symmetric answer.

## Study, don't copy — we already have a better-fit equivalent

- **MCP server + human-in-the-loop approval gate + JSONL audit + heartbeat.** Tabularis's best backend idea:
  a headless MCP subprocess whose every non-SELECT is (a) fail-closed classified, (b) queued to the GUI for
  human approval via a filesystem queue, (c) audited to `ai_activity.jsonl`
  (`/tmp/tabularis/src-tauri/src/{mcp,ai_approval,ai_activity,heartbeat}.rs`). **But we already have the
  better-architected version of every piece**: MCP *is* our universal contract (rule 7); the capability
  system + `granted = requested ∩ admin_approved` is our authz; the **inbox/outbox + approval reactor**
  (`scope/inbox-outbox/`, `scope/rules/rules-approvals-scope.md`) is our human-in-the-loop; audit is
  `scope/audit/` + `scope/observability/`. Tabularis's file-queue is a single-machine hack our bus/store
  supersede. **Verdict: skip the mechanism; the *idea* (an AI querying prod must pass a human gate + be
  audited) is already ours — cite it as validation, not a task.** The one transferable detail: their
  **fail-closed query classifier** (SELECT vs write) mirrors our host parse-allowlist — good cross-check
  that our `store.query`/federation SELECT-validation is the right boundary.
- **Multi-driver plugin protocol (JSON-RPC, any language) + capability-flags-drive-UI.** Tabularis's
  `DatabaseDriver` trait + JSON-RPC subprocess plugins (13 official: DuckDB, ClickHouse, Redis, Mongo,
  Firestore, …) + a 30-flag `DriverCapabilities` the UI reads to show/hide affordances
  (`/tmp/tabularis/src-tauri/src/{drivers,plugins}/`). We have the **federation `Source` trait** (one
  external engine per impl file) + the **extension runtime** (WASM + Tier-2 native, signed registry). The
  *architectural idea* — capability flags declared by a driver/source that the UI reflects — is a genuinely
  good pattern we partially have (`datasource.kind`, the dialect seam) and could formalize (a
  `datasource.capabilities` descriptor: supports-joins, supports-explain, quote-char, dialect). **Verdict:
  skip the plugin protocol (we have extensions + federation); scope-later a small `datasource.capabilities`
  descriptor** if/when the builder needs per-source affordance gating beyond `kind` (e.g. which sources
  support EXPLAIN). Rule 2 forbids adopting their "13 drivers" as datastores — external engines are
  federation *sources*, never second stores.
- **AI text-to-SQL / explain / notebook-cell-naming (multi-provider).** Tabularis wires OpenAI/Anthropic/
  Ollama/OpenRouter behind its approval gate (`/tmp/tabularis/src-tauri/src/ai.rs`). We have the **AI core +
  AI-gateway sidecar** and personas (`builtin.data-analyst` already has the data-verb menu). A "generate SQL
  from a prompt" button in the editor is the shipped-follow-up the `SqlEditor` port already named — it
  should call our `sql.generate`-style MCP verb through the AI gateway, **not** a new provider abstraction.
  **Verdict: skip their AI layer; scope-later the one editor button onto our AI core.**
- **Themes (12 Monaco/app presets).** We have the theme-customizer + look-packs
  (`theme-appearance-scope.md`). **Verdict: skip** — our theming is richer and token-based; if we adopt any
  editor theme JSON it's only if slice-2's Monaco open question flips (it shouldn't).
- **Off-main-thread dagre layout worker.** `/tmp/tabularis/src/workers/layoutWorker.ts` — useful *if* the
  EXPLAIN graph or a big-schema ER diagram needs auto-layout without jank. **Verdict: scope-later, bundled
  with the Visual EXPLAIN item** (only then is auto-layout needed).

## Skip — desktop/single-user assumptions that fight our model

- **Tauri multi-window, keychain secrets, SSH/K8s tunnels, connection pooling, health-check pings, task
  manager (kill plugin processes).** All single-machine desktop concerns. Our datasource connectivity is the
  federation sidecar + the platform's secret mediation + node model. **Verdict: skip** (the K8s-tunnel and
  SSH-askpass are self-contained and clever, but they belong to a desktop DB client, not a tenant platform).
- **`saved_queries.rs` / `query_history.rs` as-is.** The *features* are wanted (the user owns saved queries;
  history is a follow-up), but implement them as **workspace-scoped SurrealDB records + MCP verbs**
  (`querydef.*`), never Tabularis's local-file store. **Verdict: skip the implementation; the user's
  `querydef.*` work is the right shape.**
- **Plugin UI-slot injection (React components from plugins into named slots).** Powerful, but our
  UI-federation model (`ext.list` discovery, federated bundles) is the platform-native equivalent. **Verdict:
  skip** — don't fork a second extension-UI mechanism.

## Suggested sequencing after the three slices

1. **SQL Notebook view** (Data Studio pane) — highest analyst value; composes with insights.
2. **Rich read-only results grid + export** — the natural upgrade to slice-3's results area.
3. **Visual EXPLAIN tab** (needs a `*.explain` verb per engine) — deep debugging value.
4. **AI "generate SQL" button** onto the AI core — the already-named editor follow-up.
5. Smaller: clipboard-import type-inference, remappable keybindings, `datasource.capabilities` descriptor.

## Related

- [`query-builder-10x-scope.md`](query-builder-10x-scope.md) and its three slices — what's being built now.
- `docs/scope/insights/` — the notebook + explain items compose with the insights plane.
- `docs/scope/inbox-outbox/` · `docs/scope/rules/rules-approvals-scope.md` · `docs/scope/audit/` — our
  human-in-the-loop + audit (the better-fit equivalent of Tabularis's MCP approval gate).
- `docs/scope/datasources/` · `rust/extensions/federation/` — our federation `Source` trait (the equivalent
  of Tabularis's driver layer; external engines are sources, not stores — rule 2).
- `docs/scope/extensions/` — our extension runtime + UI federation (the equivalent of Tabularis's plugin
  protocol + UI slots).
- Tabularis (read-only reference): `/tmp/tabularis` — `src/utils/{sqlSplitter,notebookVariables,explainPlan,clipboardParser,fuzzy}.ts`, `src/components/{notebook,explain,ui/DataGrid.tsx}`, `src-tauri/src/{mcp,ai_approval,ai_activity,drivers,plugins}`.
</content>
