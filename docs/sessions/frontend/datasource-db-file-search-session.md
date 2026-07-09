# Session: Add-datasource sqlite picker — search common .db files + hide hidden dirs

## Ask

When adding a new datasource (`ui/src/features/datasources`), give the user the option to
search for common `.db` files and auto-hide hidden dirs, with the ability to turn each off.

## What shipped

The sqlite DB-file browse in [AddDatasourceForm](../../../ui/src/features/datasources/AddDatasourceForm.tsx)
now shows two checkboxes above the `HostPathPicker`, both defaulting to the convenient state
and each independently toggle-able:

- **Only show `.db` files** (default ON) — narrows the picker view to sqlite extensions
  (`db`/`sqlite`/`sqlite3`), the same set `isSqliteFile` deems selectable. Now a single
  `SQLITE_EXTS` constant is the source of truth for both selectable and view-narrowing.
- **Show hidden dirs** (default OFF) — dot-prefixed files AND dirs are hidden unless enabled.

Turning both off restores the full, unfiltered browse.

### Layering (why the split)

- **Hidden** is applied **server-side** by `host.fs.list` (the `include_hidden` param added in the
  prior host-tools session) so it hides *directories* too — a client filter can't, because dirs
  must still be navigable when narrowing files.
- **Extension narrowing** is applied **client-side** in `HostPathPicker`: non-matching *files* are
  dropped from the view, but directories always remain (navigation) and the current selection stays
  visible even if it wouldn't match. Pushing `extensions` to the server would have hidden every
  folder (the host's extension filter is files-only), breaking descent — so it stays client-side.

### Files

- `ui/src/lib/host/fs.api.ts` — `listHostDir(path, filter?)` gains an optional `HostFsListFilter`
  (`name` / `extensions` / `includeHidden`) forwarded to the host verb (`include_hidden` sent only
  when enabled, so omission = host default).
- `ui/src/lib/host/HostPathPicker.tsx` — new optional props `hideHidden` (default true, server-side)
  and `narrowExtensions` (client-side view filter). Empty-folder copy distinguishes truly-empty from
  "narrow hid every file here". Both existing call-sites (studio DirectoryPicker, this form) keep
  working on defaults; only the datasource picker sets them.
- `ui/src/features/datasources/AddDatasourceForm.tsx` — the two checkboxes (shadcn `Checkbox`/`Label`),
  state, wiring; `SQLITE_EXTS` constant.

## Tests (green, real gateway — CLAUDE §9)

`ui/src/features/datasources/AddDatasourceForm.gateway.test.tsx` (`pnpm test:gateway`):
- existing "browses … selects a real .db" test updated to first toggle the db-only narrow OFF (its
  assertion needs the non-db sibling visible).
- new "defaults to only .db files + hidden entries off, and both toggles reveal the rest": seeds a
  real `.db`, a `.txt`, and a hidden `.secret.db` via `devkit.write_file`, then asserts the default
  view hides the `.txt` (narrowed) and `.secret.db` (hidden); "Show hidden" surfaces `.secret.db`;
  turning off db-only brings the `.txt` back.

```
✓ AddDatasourceForm.gateway.test.tsx (2 tests)
Test Files  1 passed (1)   Tests  2 passed (2)
```

`pnpm tsc --noEmit` clean. Lint clean except the pre-existing raw `<select>` for datasource kind
(present before this session — not touched here).

## Notes / rejected alternatives

- Rejected sending `extensions` server-side (would drop navigable dirs — host extension filter is
  files-only). Client-side view filter keeps folders + current selection visible.
- Kept both filters as plain checkboxes over adding a query box — the ask was specifically the two
  named conveniences; a free-text `name` search is already supported by the API layer if needed later.
