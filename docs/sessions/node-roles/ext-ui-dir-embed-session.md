# Session — ext-UI serve dir on the embed seam (`BootConfig.ext_ui_dir`)

_2026-07-11._

## Goal

Let a downstream embedder (rubix-ai) relocate the gateway's extension-UI serve dir through the
supported embed seam, without env below the seam and without changing the standalone `node` binary.

## Decision

`store_path` and the ext-UI dir are the **two distinct on-disk footprints** of an installed extension:

1. wasm + manifest artifacts → rows in the SurrealDB store (`store_path`, already on the seam).
2. UI bundles → filesystem `Gateway.ext_ui_dir`, served at `/extensions/{ext}/ui/{file}`.

Footprint 2 was the only boot input still env-only (`LB_EXT_UI_DIR`, read in `Gateway::build`, below
the seam). Fix: give it the identical treatment as `store_path` — an additive `Option<String>` on
`BootConfig`, wired in `boot_full` via the pre-existing `Gateway::with_ext_ui_dir` builder.

Rejected: threading a whole `GatewayConfig` sub-struct — overkill for one field; the additive-field
pattern is the documented API-commitment mitigation and suffices.

## Changes

- `rust/node/src/config.rs`: `BootConfig.ext_ui_dir: Option<String>` + `Default` (`None`) + `from_env`
  (`None` on purpose — binary unchanged, gateway keeps reading `LB_EXT_UI_DIR`).
- `rust/node/src/builder.rs`: in `boot_full`, `Some(non-empty)` ⇒ `gw.with_ext_ui_dir(dir)`.
- Docs: `docs/scope/node-roles/ext-ui-dir-embed-scope.md`.

## Backward compatibility

`from_env()` sets `None`, so `boot_full(from_env())` (the binary) builds the gateway with no
`with_ext_ui_dir` call → the gateway's own `LB_EXT_UI_DIR`/`"extensions-ui"` default stands. The
standalone binary is untouched; `cargo build -p lb-node` green.

## Tag

`node-v0.1.12` cut from master `1801df9` + this slice.

## Proof

Live proof of the end-to-end behaviour (store + ext-UI dir under `.rubix-ai/`, publish, restart-survival)
is in the rubix-ai session doc `docs/sessions/node-roles/rubix-local-state-session.md` — that repo is the
embedder that exercises this field.
</content>
