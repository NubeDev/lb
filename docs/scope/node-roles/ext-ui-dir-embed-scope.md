# Ext-UI serve dir on the embed seam — `BootConfig.ext_ui_dir`

Status: scope (the ask). Additive slice on the stable embed seam (`embed-node-scope.md`).

## The ask

An embedder (`NubeIO/rubix-ai`) wants to relocate where the gateway serves installed extension **UI
bundles** from — putting all installed-extension on-disk state under its own product dir — without
reading env below the boot seam and without touching the standalone `node` binary.

## What existed

- The gateway serves UI bundles from `Gateway.ext_ui_dir` (`role/gateway/src/state.rs`),
  `{ext_ui_dir}/{ext}/{file}` → `GET /extensions/{ext}/ui/{file}` (`routes/ext_ui.rs`).
- The dir is seeded in `Gateway::build` from `LB_EXT_UI_DIR` (default `extensions-ui` beside cwd) —
  **below** the embed seam, i.e. env-driven, the one boot input not yet struct-config.
- A `Gateway::with_ext_ui_dir(dir)` builder already existed (tests use it).

This is the exact shape `store_path` already has (a durable footprint the embedder must place), so it
gets the exact same treatment.

## The change (additive, backward-compatible)

- `BootConfig` gains `pub ext_ui_dir: Option<String>` (mirrors `store_path`'s doc + `#[non_exhaustive]`
  shape — additive field, `default()`-then-mutate construction unbroken).
- `boot_full`: `Some(non-empty)` ⇒ build the gateway with `.with_ext_ui_dir(dir)`; `None` ⇒ today's
  behaviour unchanged (the gateway keeps reading `LB_EXT_UI_DIR`/`"extensions-ui"` itself).
- `from_env()` leaves it `None` **on purpose** — the standalone binary's gateway still reads
  `LB_EXT_UI_DIR` directly, so the binary is byte-for-byte unchanged. Only an embedder filling the
  struct uses the field.

No library code below the seam reads env for this (the embed doctrine). Symmetric nodes: it is config,
not a role branch. One datastore untouched — this is the *filesystem* UI-bundle footprint, distinct
from the wasm/manifest artifacts that live as store rows (`store_path`).

## Tag

Cut as `node-v0.1.12` (master tip `1801df9` + this slice). rubix-ai bumps `lb-node` to it.

## Related

- `embed-node-scope.md` — the seam this extends.
- `../extensions/ext-out-of-tree-scope.md` — where installed extensions live out of tree.
</content>
