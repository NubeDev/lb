# Session: host.fs.list — search by name, file type, and show/hide hidden dirs

## Ask

Add to `host.fs.list` (`rust/crates/host/src/host_tools/fs`):
- search by name,
- filter by file type / extension (e.g. `.txt`, `.db`, `.sql`),
- a way to show/hide hidden dirs.

Must work on linux, darwin, and windows.

## What shipped

`host.fs.list` now accepts three optional, additive input fields (all default to the
prior behavior when omitted, so existing callers are unaffected):

- `name` — case-insensitive substring the entry name must contain.
- `extensions` — array of extensions, with or without a leading dot (`".txt"` or
  `"txt"`), matched case-insensitively against the entry's extension. Only real
  files match; dirs/symlinks never satisfy an extension filter. An entry whose
  name is all-extension (`.txt` with an empty stem) is treated as hidden, not as a
  `.txt` file.
- `include_hidden` — bool, default `false`. Hidden = leading-dot name, which is the
  uniform cross-OS convention (linux/darwin dotfiles; on Windows we deliberately use
  the same dot rule rather than the FILE_ATTRIBUTE_HIDDEN bit, so results are
  identical and predictable on all three platforms — symmetric-node spirit).

Filtering happens per-entry **before** the `HOST_FS_LIST_LIMIT` cap, so the limit
bounds returned (filtered) rows.

### Files

- `rust/crates/host/src/host_tools/fs/list.rs` — added `Filter`, `parse_filter`,
  `is_hidden`, `ext_matches`, `keep`; wired into the read_dir loop.
- `rust/crates/host/src/system/catalog.rs` — catalog description mentions the filters.

## Tests (green)

`rust/crates/host/tests/host_tools_test.rs`:
- existing `fs_list_without_its_cap_is_denied` (capability-deny) still passes.
- new `fs_list_filters_by_extension_hides_hidden_and_matches_name` over a real temp
  tree exercises: default hides dot entries; `include_hidden` surfaces hidden file +
  dir; `extensions: [".txt","db","sql"]` case-insensitive + dot-agnostic; `name`
  case-insensitive substring.

```
running 2 tests
test fs_list_filters_by_extension_hides_hidden_and_matches_name ... ok
test fs_list_without_its_cap_is_denied ... ok
test result: ok. 2 passed; 0 failed
```

Real infra only (`Node::boot()`, real caps, real temp dir) — no mocks (rule 9).

## Notes / rejected alternatives

- Rejected the Windows hidden-attribute bit in favor of the dot rule for
  cross-platform determinism and to avoid a `#[cfg(windows)]` branch in a core crate.
- Kept filtering in `list.rs` rather than a new verb — it's the same fact, narrowed.
