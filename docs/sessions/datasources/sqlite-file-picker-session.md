# Session — reuse the extension file picker for the sqlite datasource path

**Ask.** When a user adds a datasource of kind `sqlite`, let them *browse the node*
for the `.db` file instead of typing the path. Reuse the file-select UI that the
Extensions/Studio page already uses (the "open existing" folder picker), so there is
one node-side path browser, not two.

## What was there

- `ui/src/features/studio/DirectoryPicker.tsx` — the Studio "open existing" browser.
  It walked the host filesystem one level at a time via `host.fs.list`, anchored at
  the devkit root (`devkit.root`), and treated a folder holding `extension.toml` as
  selectable. All the browse/breadcrumb/manual-fallback mechanics were baked into
  this one file, hardwired to *folder* selection + *extension* semantics.
- `ui/src/lib/host/fs.api.ts` — the `host.fs.list` browser client (unchanged).
- `ui/src/features/datasources/AddDatasourceForm.tsx` — the add-datasource form. The
  sqlite DSN was a plain (password-typed) text input; the path had to be typed.

## What shipped

1. **Extracted the reusable engine** → `ui/src/lib/host/HostPathPicker.tsx`. It owns
   all the node-side browse mechanics (resolve anchor once, list a level, breadcrumb,
   Up, manual-path fallback) and is parameterized by:
   - `resolveRoot()` — the anchor to confine the browse to;
   - `mode` — `"dir"` (the current folder is the candidate) or `"file"` (a file entry
     is the candidate);
   - `selectable(list|entry)` — the caller's "what counts as pickable" predicate;
   - `renderBanner` — optional guidance banner (dir mode).

2. **`DirectoryPicker` is now a thin wrapper** over `HostPathPicker` in `"dir"` mode —
   same devkit-root anchor, same `extension.toml` predicate, same guidance banner. No
   behavior change (the Studio real-gateway test still passes green).

3. **`AddDatasourceForm` sqlite path picker.** For kind `sqlite` the form now offers a
   **Browse node** toggle that renders `HostPathPicker` in `"file"` mode, marking
   `.db`/`.sqlite`/`.sqlite3` files selectable. Clicking a file fills the DSN with its
   absolute node path. A **Type path** toggle restores the manual field. The sqlite DSN
   input is plain text now (a file path is not a secret); non-sqlite DSNs stay
   password-typed.

4. **New `host.fs.home` verb** (`crates/host/src/host_tools/fs/home.rs`) — returns the
   node's home dir (normalized like the other `host.fs.*` facts), catalogued and gated
   on `mcp:host.fs.home:call` (added to the dev-login granted caps). The sqlite picker
   anchors here, NOT at the devkit root.

### First attempt anchored at the devkit root — wrong (fixed same session)

The first cut reused `devkit.root` as the anchor. Live-tested: the picker then browsed
the **`extensions/` tree**, which is the wrong place for a sqlite DB — a node-local DB
lives anywhere (`/var/lib/lb/…`), not under the devkit tree. Fixes:
- anchor at the node **home dir** via the new `host.fs.home` verb (a clean start where
  user data lives) instead of the devkit root or the noisy `/`;
- make the anchor a **starting point, not a wall**: added `confineToRoot` to
  `HostPathPicker` (default `true` = the extension picker's hard wall; `false` for the
  datasource picker so **Up** walks above home to reach `/var/lib/lb/…`). The breadcrumb
  falls back to the absolute path when the browse goes above the anchor.

### Endpoint prefill `127.0.0.1:0` for sqlite is CORRECT — leave it

For sqlite the DSN *is* the file path; `source/sqlite.rs::connect` never uses the
endpoint (no network). The endpoint field only drives the `net:tls:host:port:connect`
grant preview, and `127.0.0.1:0` is the default-pre-approved local convention
(`FED_ENDPOINTS`). See `docs/sessions/datasources/sqlite-datasource-demo-session.md`.

### Rules held

- No core branch on an extension id; the picker is generic UI over the generic
  `host.fs.list`/`devkit.root` verbs (§10).
- One responsibility per file: engine (`HostPathPicker`), extension wrapper
  (`DirectoryPicker`), form (`AddDatasourceForm`) each stay small (§8).

## Tests (real gateway, no fakes — §9)

- **New:** `ui/src/features/datasources/AddDatasourceForm.gateway.test.tsx` — signs in
  with `devkit.write_file` + `host.fs.list` + `host.fs.home` caps, seeds a REAL
  `buildings.db` and a non-db `notes.txt` (the seed returns their canonical abs paths),
  reads `host.fs.home`, then drives the form: switch to sqlite → Browse node → walk the
  segments below home down to the seeded folder → asserts `buildings.db` is a selectable
  button that fills the DSN with its absolute node path, and `notes.txt` is shown but
  NOT selectable. Green.
- **Host:** `cargo test -p lb-host --test host_tools_test --test catalog_mcp_test` green
  (the new `host.fs.home` verb is catalogued + cap-gated alongside the other host facts).
- **Regression:** `StudioView.gateway.test.tsx` (drives the refactored `DirectoryPicker`
  through the real `host.fs.list`) and `DatasourcesAdmin.gateway.test.tsx` both still
  pass. `StudioShell.test.tsx` (unit) passes. `tsc --noEmit` clean.

### Command output

```
✓ src/features/datasources/AddDatasourceForm.gateway.test.tsx (1 test) 192ms
✓ src/features/datasources/DatasourcesAdmin.gateway.test.tsx (6 tests)
✓ src/features/studio/StudioView.gateway.test.tsx (1 test)
✓ src/features/studio/StudioShell.test.tsx (4 tests)
```

Pre-existing (not from this change): `AddDatasourceForm.tsx` still trips the
`no-restricted-syntax` lint on its raw `<select>` for the kind dropdown — present on
clean master, out of scope here.
