# HANDOVER — Reports feature: build done, one live-store reseed left

## TL;DR

The **reports feature is built, compiles, and every automated test is green** (Rust + gateway +
UI). The ONLY remaining problem is a **stale persisted role record** in your live dev store: your
admin/member token does not carry the new `mcp:report.save:call` / `mcp:report.export:call` /
`mcp:brand.save:call` caps, so the UI denies "create report". This is **not a code bug** — the caps
are correctly in the code. It is the well-known "built-in role rows only re-seed when absent + no
Rust hot-reload + node auto-respawns and holds the store lock" gotcha.

**Do the fix in ONE terminal (see "THE FIX" below). The trick you were missing: `make dev`
auto-respawns the node, so `pkill node` alone doesn't work — you must stop the whole `make dev`
process group first, THEN reseed, THEN restart.**

---

## What shipped (all 5 tracks complete)

Feature = report builder + branded PDF exporter. Spec: `docs/scope/reports/report-builder-scope.md`
(no open questions; every decision was pre-made — do not re-litigate). Built per `docs/HOW-TO-CODE.md`.

### Track A — `lb-render` crate (pure Typst PDF) — GREEN, 24 tests
- New crate `rust/crates/render/` (package `lb-render`, **edition 2024** — the sole 2024 island;
  the workspace default is 2021, set explicitly in its Cargo.toml).
- Ported verbatim from `/home/user/code/rust/lazybones/crates/lazybones-render/`: `convert.rs`,
  `world.rs`, `error.rs`, `model.rs`, `pdf.rs`, `spike.rs`. **Dropped `html.rs`** (no server HTML
  preview). `lib.rs` rewritten to remove html.
- Typst stack exact-pinned in root `rust/Cargo.toml` `[workspace.dependencies]`:
  `typst = "=0.15.0"`, `typst-pdf`, `typst-assets` (feature `fonts`), `typst-layout`,
  `comemo = "=0.5.1"`, `pulldown-cmark = "0.13"`. **The Phase-3a spike PASSED on this toolchain
  (rustc 1.96) — Typst 0.15 → %PDF- bytes with embedded fonts. Verified before anything else.**
- Public API: `lb_render::render_pdf(&Assembled) -> Result<Vec<u8>, RenderError>`, with
  `Assembled { title, pages, brand, logo, images, page_titles, options }` builder + `Brand/Colors/
  Fonts/ImageAsset/RenderOptions`.
- Verify: `cd rust && cargo test -p lb-render --lib` → **24 passed** (16 converter + 8 pdf incl. spike).

### Track B — host `report/` + `brand/` modules — GREEN, 7 tests
- New `rust/crates/host/src/report/` (mirrors `panel/`): `mod.rs model.rs store.rs authorize.rs
  visibility.rs error.rs get.rs list.rs save.rs delete.rs share.rs export.rs tool.rs`.
- New `rust/crates/host/src/brand/`: `mod.rs model.rs store.rs authorize.rs error.rs get.rs list.rs
  save.rs delete.rs seed.rs tool.rs`.
- `report:{id}` asset with ordered `blocks[]`; a panel block embeds `crate::dashboard::Cell`
  directly (a block IS a Cell) and reuses the SHIPPED `lb_host::hydrate_cells` (at get) +
  `validate_and_strip_refs` (at save). `report.usage` deferred (scope Decision 4) — delete is a
  plain owner-only tombstone.
- `report.export` = bounded-synchronous: `report_export(store, principal, ws, id,
  snapshots: Vec<(String /*cell.i*/, Vec<u8> /*png*/)>, now) -> Result<Vec<u8>, ReportError>`;
  assembles blocks + brand + logo + client snapshots → `lb_render::render_pdf`. Server NEVER fetches
  widget data for export (snapshots come from the client under the viewer's caps).
- Wiring edited (SHARED host files): `crates/host/Cargo.toml` (+`lb-render`), `lib.rs`
  (`mod report; mod brand;` + pub-use blocks; `Block` re-exported as `ReportBlock`, `Visibility` as
  `ReportVisibility`), `tool_call.rs` (`"report."`/`"brand."` in HOST_NATIVE_PREFIXES + dispatch
  arms), `system/catalog.rs` (6 report + 4 brand entries), **`authz/builtin_roles.rs`** (see caps
  below).
- Tests: `crates/host/tests/report_test.rs` — CRUD round-trip, capability-deny per verb (save/
  export/brand.save), workspace isolation, panel_ref hydration + dangling-ref reject, brand seed
  idempotent, MAX_BLOCKS, markdown-only export → %PDF bytes.
- Verify: `cd rust && cargo test -p lb-host --test report_test` → **7 passed**.

### Track C — gateway routes + IPC — GREEN, 4 tests
- New `rust/role/gateway/src/routes/report.rs`, `brand.rs`, `assets_bin.rs`.
- Routes (registered in `server.rs router()`):
  `GET/POST /reports`, `GET/DELETE /reports/{id}`, `POST /reports/{id}/share`,
  `POST /reports/{id}/export.pdf` (raw `application/pdf` bytes + `authenticate`, with
  `DefaultBodyLimit::max(32 MiB)` — the 2 MB default would reject snapshot POSTs),
  `GET/POST /brands`, `GET/DELETE /brands/{id}`,
  `POST /assets` (+32 MiB limit) & `GET /assets/{id}` (the greenfield binary-asset route).
- `rust/role/gateway/Cargo.toml` got `base64.workspace = true`.
- `ui/src/lib/ipc/http.ts`: added `report_*` / `brand_*` / `assets_put_asset` cases + a `postBytes`
  helper (export returns a Blob, not JSON).
- Tests: `rust/role/gateway/tests/report_routes_test.rs` — 3-block round-trip, export → 200
  `application/pdf` `%PDF-`, deny (save/export/brand.save), cross-ws isolation.
- Verify: `cd rust && cargo test -p lb-role-gateway --test report_routes_test` → **4 passed**.
- NOTE: no per-workspace boot-seed seam exists for brands; `seed_default_brand` is available but not
  boot-wired. Brands are created on demand; export falls back to a neutral default when `brand_id`
  is empty. A `// NOTE (brand seed)` is left in `server.rs`.

### Track D — UI feature — GREEN, tsc clean, 3 tests
- New `ui/src/features/reports/`: `ReportsView` (router entry, exported from `@/features/reports`),
  `ReportsPage` (roster), `ReportEditor` (block list, add markdown/panel/image, move-up/down
  reorder — `@dnd-kit` is not a dep, so keyboard buttons; `page_break`/caption/panel-picker), `ReportView`
  (A4 print-fidelity live preview), `ExportButton` (snapshot → POST → download), `PanelPicker`,
  `blocks.ts`, `ReportsPage.test.tsx`.
- New shared: `ui/src/components/markdown-editor/` (TipTap + tiptap-markdown, true-A4 210×297mm
  sheet, 20mm margins), `ui/src/components/brand-picker/` (+ `EMBEDDABLE_FONTS` = Libertinus Serif /
  DejaVu Sans Mono / New Computer Modern — lesson 4), `ui/src/lib/snapshot/` (ECharts getDataURL
  fast path + html-to-image fallback; shipped charts are Recharts/SVG so html-to-image is primary),
  `ui/src/lib/report/`, `ui/src/lib/brand/`.
- Extracted `ui/src/features/panel/PanelEmbed.tsx` from `PanelPage.tsx` (the
  `DashboardCacheProvider` → `WidgetHost` composition — the provider is REQUIRED or widgets/
  useDatasourceList break). `PanelPage.tsx` refactored to use it (its gateway test contract kept).
- Deps added to `ui/package.json`: `@tiptap/react @tiptap/starter-kit @tiptap/pm tiptap-markdown
  html-to-image` (pnpm-lock.yaml updated).
- NOTE: image-block upload currently uses the existing `readBrandImage` File→data-URI helper inline;
  switch to Track C's `assets_put_asset` (`asset_id` ref) for large images — noted inline.
- Verify: `cd ui && npx tsc --noEmit` (clean) and `npx vitest run src/features/reports` → **3 passed**.

### Track E — sidebar/nav access — DONE (the 7 edits)
- `ui/src/features/shell/NavRail.tsx` (`| "reports"` in `CoreSurface` + Workspace group),
  `surfaceDefs.ts` (`FileText` icon), `routing/surface.ts` (`reports: "/reports"` in CORE_PATHS),
  `routing/allowed.ts` (`CAP.reportList` push), `lib/session/admin-caps.ts` (`reportList/reportSave/
  reportExport/brandSave` CAP constants), `routing/createAppRouter.tsx` (`ReportsView` import + core
  route).
- Server reach: **no edit needed**. `reach:reports:view` is derived generically by the opaque
  `collect_surfaces` fold in `rust/crates/host/src/nav/reach.rs` (surface keys are opaque data —
  rule 10), and fallback navs already grant `reach:*:view`.

### The exact caps added (in `rust/crates/host/src/authz/builtin_roles.rs`)
- `VIEWER_CAPS` (near the panel viewer block): `mcp:report.get:call`, `mcp:report.list:call`,
  `mcp:brand.get:call`, `mcp:brand.list:call`.
- `AUTHOR_CAPS` (near the panel author block): `mcp:report.save:call`, `mcp:report.delete:call`,
  `mcp:report.share:call`, `mcp:report.export:call`, `mcp:brand.save:call`, `mcp:brand.delete:call`.
  (`report.export` is a concrete cap — NOT covered by any `mcp:*.*:call` wildcard.)
- Role composition (already correct): `viewer` = VIEWER_CAPS; `member` = viewer ∪ author;
  `workspace-admin` = member ∪ admin. So admin DOES include the report author caps **in code**.

---

## THE BUG YOU'RE HITTING (create report → denied, even as admin)

**Root cause: a stale persisted role record.** `resolve_caps` (`rust/crates/authz/src/resolve.rs:73`)
resolves a member/admin's caps by reading the **stored `role` record** (`role_caps` →
`read(store, ws, "role", name)`), NOT by recomputing from the current `workspace_admin_role_caps()`
function. The built-in role rows are seeded by `ensure_builtin_authz_roles` →
`ensure_one`, which **writes a role row only when it is ABSENT**
(`rust/crates/host/src/authz/builtin_roles.rs:128`). Your `acme` dev store was seeded
(`LB_STORE_PATH=.lazybones/data/dev-store`) BEFORE the `report.*` caps existed, so its
`member` / `workspace-admin` rows are frozen at the old cap set and are never overwritten.

**Proof (captured live):** a fresh login as `user:ada` in `acme` yields a token with
`report caps: ['mcp:brand.get:call','mcp:brand.list:call','mcp:report.get:call','mcp:report.list:call']`
— i.e. only the VIEWER caps (those come from the live `viewer_role_caps()` login floor in
`role/gateway/src/session/credentials.rs`). `report.save/export/brand.save` (the AUTHOR caps, which
ride the stored `member`/`workspace-admin` role row) are MISSING.

**Why a plain restart didn't fix it:** the seed is idempotent — the stale rows survive the restart.
**Why `pkill node` didn't fix it:** `make dev` runs `trap 'kill 0' EXIT INT TERM; ( ...node... ) & ( ...ui... ) & wait`, so killing just the `node` process makes the wrapper respawn it (you saw a new
pid). The respawned node re-acquires the SurrealKV store lock, so the reseed one-shot can't open the
store.

---

## THE FIX (copy/paste — one terminal, in order)

A throwaway maintenance one-shot already exists: `rust/node/examples/reseed_roles.rs`. It deletes the
`viewer`/`member`/`workspace-admin` role rows in the given workspace(s) and immediately re-seeds them
from the CURRENT code. You must run it while the node is STOPPED (it holds the store lock).

```bash
cd /home/user/code/rust/lb

# 1. Stop the WHOLE dev stack (not just `node` — the make-dev wrapper respawns it).
#    Ctrl-C in the terminal running `make dev` is cleanest. From another terminal, use:
make kill 2>/dev/null || pkill -f "kill 0" ; pkill -f "target/debug/node" ; pkill -f "vite" ; sleep 3
pgrep -af "target/debug/node" | grep -v grep && echo "STILL RUNNING — stop make dev in its own terminal" || echo "node stopped"

# 2. Reseed the built-in role rows in acme (add any other workspaces you use as extra args).
cd rust
LB_STORE_PATH=/home/user/code/rust/lb/.lazybones/data/dev-store \
  cargo run -p node --example reseed_roles -- acme
# expect: "reseeded built-in roles in workspace acme"

# 3. Restart the dev stack.
cd /home/user/code/rust/lb
make dev
```

### Verify the fix (from any terminal, node running)

```bash
BASE=http://127.0.0.1:8080
TOK=$(curl -s -X POST $BASE/login -H 'content-type: application/json' \
  -d '{"user":"user:ada","workspace":"acme"}' \
  | python3 -c 'import sys,json;print(json.load(sys.stdin)["token"])')
echo "$TOK" | cut -d. -f2 | python3 -c 'import sys,base64,json;s=sys.stdin.read().strip();s+="="*(-len(s)%4);d=json.loads(base64.urlsafe_b64decode(s));print("report/brand caps:",[c for c in d["caps"] if "report" in c or "brand" in c])'
# EXPECT to now ALSO include: mcp:report.save:call, mcp:report.delete:call, mcp:report.share:call,
#   mcp:report.export:call, mcp:brand.save:call, mcp:brand.delete:call
```

Then in the browser: **log out and log back in** (the old token in localStorage is still stale —
a re-login mints a fresh token from the refreshed role rows), open **Reports → New report**, and
saving/exporting should now work.

> If ada is a plain member not an admin and you want admin: the same reseed fixes `workspace-admin`.
> Whatever role `user:ada` holds in `acme`, its row is now refreshed.

---

## Fallback fix (if the one-shot is awkward): nuke the dev store

Wipes ALL dev data (dashboards, panels, users, any reports) but is guaranteed correct — boot
re-seeds everything fresh:

```bash
cd /home/user/code/rust/lb
# stop make dev first (see step 1 above), then:
rm -rf .lazybones/data/dev-store
make dev   # re-seeds roles + LB_SEED_USER=user:ada from current code
```

---

## Cleanup / finish-the-work checklist (after the fix verifies green)

The user's original constraints: **NO git commits, NO session/debugging/doc-site writes until they
test & approve.** So do NOT do those yet. When approved:

1. **Delete the throwaway** `rust/node/examples/reseed_roles.rs` (and its `examples/` dir if empty).
2. **Consider a durable fix for the idempotent-seed footgun** (optional, discuss first): built-in
   role rows silently going stale when a new built-in cap is added is a repeat trap. Options:
   (a) version the built-in role records and re-seed when the code version is newer; (b) union the
   live `workspace_admin_role_caps()` into `resolve_caps` for the built-in role names. Either is a
   real design change — write a scope note, don't just patch.
3. **Full integration re-run** (all were green before the reseed detour):
   ```bash
   cd rust && cargo fmt && cargo build --workspace && \
     cargo test -p lb-render --lib && \
     cargo test -p lb-host --test report_test && \
     cargo test -p lb-role-gateway --test report_routes_test
   cd ../ui && npx tsc --noEmit && npx vitest run src/features/reports src/features/panel
   # full suites (note: some pre-existing reds are unrelated — see memory):
   #   cargo test --workspace  (run `make build-wasm` first, per memory: test-be-no-wasm-dep)
   #   pnpm test ; pnpm test:gateway  (gateway suite has broad pre-existing reds — validate touched files)
   ```
4. **Then** (only on approval) write per HOW-TO-CODE.md: `docs/sessions/reports/…-session.md`,
   any `docs/debugging/…` entry for THIS role-reseed issue (root cause + the reseed fix + a
   regression note), promote to `doc-site/content/public/reports/reports.md`, update
   `docs/STATUS.md`, and `docs/skills/reports/SKILL.md` grounded in a live run.

---

## Key facts / gotchas for whoever picks this up

- **rustc is 1.96; Typst 0.15 compiles fine here** (spike proven). Don't touch the version pins.
- **No Rust hot-reload.** Any host/gateway change needs `make kill && make dev` (memory:
  flows-dev-node-no-hot-reload). The node binary in `make dev` is `rust/target/debug/node`.
- **`make dev` auto-respawns `node`** via `trap 'kill 0'...wait` — kill the wrapper/process group,
  not just node, or it comes back and re-locks the store.
- **Built-in role rows are frozen at first seed** (`ensure_one` writes only when absent). Adding a
  cap to `VIEWER_CAPS`/`AUTHOR_CAPS`/`ADMIN_ONLY_CAPS` does NOT reach an already-seeded workspace
  until its role rows are deleted+reseeded (this whole bug). VIEWER caps DID work because the login
  floor `credentials.rs` calls the live `viewer_role_caps()` — but AUTHOR/ADMIN caps come only from
  the stored role record.
- **Browser token is cached in localStorage** — after reseeding, users must re-login to mint a fresh
  token.
- Persistent dev store path: `/home/user/code/rust/lb/.lazybones/data/dev-store`; gateway on
  `127.0.0.1:8080`; workspace `acme`; seed user `user:ada`.
- Pre-existing unrelated test reds (do NOT chase — from memory): panel_test 'unknown view STALE',
  agent_routed_test, SystemView.gateway, sqlSource.gateway, and broad `pnpm test:gateway` reds
  (WorkflowView 404 on _seed/approval, ProofPanel, RolesAdmin). Validate via touched files.

## New/changed files (for review)

New dirs/files: `rust/crates/render/`, `rust/crates/host/src/report/`,
`rust/crates/host/src/brand/`, `rust/crates/host/tests/report_test.rs`,
`rust/role/gateway/src/routes/{report,brand,assets_bin}.rs`,
`rust/role/gateway/tests/report_routes_test.rs`, `rust/node/examples/reseed_roles.rs` (THROWAWAY),
`ui/src/features/reports/`, `ui/src/features/panel/PanelEmbed.tsx`,
`ui/src/components/{markdown-editor,brand-picker}/`, `ui/src/lib/{report,brand,snapshot}/`.

Edited: `rust/Cargo.toml` (+ Cargo.lock), `rust/crates/host/Cargo.toml`,
`rust/crates/host/src/{lib.rs,tool_call.rs,system/catalog.rs,authz/builtin_roles.rs}`,
`rust/role/gateway/{Cargo.toml,src/routes/mod.rs,src/server.rs}`,
`ui/src/features/panel/PanelPage.tsx`, `ui/src/features/shell/{NavRail.tsx,surfaceDefs.ts}`,
`ui/src/features/routing/{surface.ts,allowed.ts,createAppRouter.tsx}`,
`ui/src/lib/{ipc/http.ts,session/admin-caps.ts}`, `ui/package.json` (+ pnpm-lock.yaml).

(Some `git status` entries like `header-breadcrumbs`, `TopMenuNav`, `theme-options`,
`shell-chrome-layout*` are from OTHER concurrent sessions, not this reports work.)
