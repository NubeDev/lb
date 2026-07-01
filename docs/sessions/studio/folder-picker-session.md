# Extension Studio — folder picker session

**Date:** 2026-07-01 · **Area:** `ui/src/features/studio` + `rust/crates/host/src/devkit` · **Type:** feature (UX + one small backend verb)

## The ask

In the Studio wizard's **step 1 → "Open existing"**, the user had to *type* a folder path — error-prone
and opaque (you couldn't tell which folder was a valid extension, and the path had to resolve under a
root the UI never surfaced). Replace it with a real filesystem picker, using the existing
`host_tools/fs` verbs (`rust/crates/host/src/host_tools/fs`).

## The constraint that shaped the design

`devkit.inspect` / `devkit.build` / gateway publish all run the path through `resolve_under_root`,
anchored at `default_devkit_root()` = **`rust/extensions`** (or `$LB_DEVKIT_ROOT`). Paths outside the
root — or containing `..` — are rejected. So a picker **must** browse within that root, and the UI had
no way to discover its absolute location. That gap is exactly why typing was fragile.

## What shipped

**Backend — one tiny verb** (`devkit.root`):
- `rust/crates/host/src/devkit/root.rs` — returns `{ path, os }`, the canonicalized absolute devkit
  root (creates it if missing, matching what the other verbs resolve against). Gated
  `mcp:devkit.root:call` via the existing `authorize_devkit`.
- Wired into `devkit/mod.rs`, the `call_devkit_tool` router (`devkit/tool.rs`), `lib.rs` re-exports,
  and the host tool catalog (`system/catalog.rs`). `lb-host` compiles green; the catalog is sorted at
  serve time (`host_catalog()`), so insertion order is irrelevant to `system_map_test`.

**UI:**
- `ui/src/lib/host/fs.api.ts` — a browser client for `host.fs.list` (one dir level, over `POST
  /mcp/call`). One responsibility: the host-fs read surface.
- `ui/src/lib/devkit/devkit.api.ts` — added `devkitRoot()`.
- `ui/src/features/studio/DirectoryPicker.tsx` — the browser: anchors at `devkit.root`, lists with
  `host.fs.list`, breadcrumb + `Up` (clamped to root — you can't and needn't go above it), folders
  navigate on click, files are muted, `extension.toml` is highlighted. The **current folder is the
  candidate**: it's selectable (sets `w.path`) only when it holds an `extension.toml`; otherwise the
  selection is cleared and a hint guides the user deeper — so the wizard's "Open folder" button is
  never enabled on a folder that would fail `inspect`. A manual path `Input` is the graceful fallback
  when the fs capability isn't granted.
- `CreateStep.tsx` — the "Open existing" branch now renders `<DirectoryPicker>` instead of a raw path
  input.

## New capabilities the feature needs

The Studio user now also needs `mcp:devkit.root:call` and `mcp:host.fs.list:call` granted (browsing is
denied-opaque without them; the picker falls back to a manual path field on error).

## Verification

- **Rust:** `cargo build -p lb-host` — clean. Catalog sort test reasoning confirmed (sorted at serve
  time). `host.fs.list` / `devkit.root` both dispatch + authorize.
- **UI:** `npx tsc --noEmit` clean in the changed areas; `npm run lint` — **0 errors** (the new files
  are fully standard-compliant; the 91 warnings are pre-existing legacy views).
- **Visual QA:** rendered the picker against a throwaway stub gateway (deleted after — not a committed
  fake, per rule 9) and screenshotted **light + dark**, both states: at the root (folders navigable,
  `Up` disabled, "no extension.toml here" hint, empty selection) and inside an extension folder
  (`extension.toml` highlighted, accent "is an extension — open it below" banner, populated
  selection). Both themes read on-brand.

## Docs

- Fixed a real inaccuracy in `docs/skills/extensions/SKILL.md`: scaffolds land under
  `$LB_DEVKIT_ROOT` (default `rust/extensions`), **not** `~/.lazybones` (which is `$LB_DIR`, only the
  publisher key). Added the `devkit.root` cap and a gotcha on the under-root path rule.

## Follow-ups

- The picker detects extension-ness only for the *current* folder (one `host.fs.list` level). Badging
  child folders as extensions would cost one `stat` per child — deferred; the current guidance is
  enough.
- A render-level test of the picker (navigate → select) against the real gateway would complement the
  existing `StudioView.gateway.test.tsx` happy-path.
