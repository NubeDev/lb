# rust/

The Rust workspace — the platform core and the `node` binary.

One Cargo workspace. Core crates compile into every node; role-only crates and the
`node` binary wire them together and select edge/cloud roles by config.

- **What goes here:** `host`, `bus`, `store`, `runtime`, `mcp`, `auth`, `caps`,
  `tags`, `inbox`, `jobs`, `secrets`, `sync`, `ext-loader`, the SDK crate, the
  role-only crates (`gateway`, `registry-host`, `bootstrap-ui`), and the `node`
  binary. See `../README.md` §9 for the crate map.
- **Extensions** ship as separate WASM/native artifacts, never dynamically-linked
  Rust.

Before writing code, read [`../docs/FILE-LAYOUT.md`](../docs/FILE-LAYOUT.md): one
responsibility per file, ≤400 lines hard, one verb per file.

Status: not yet scaffolded — architecture scope only.
