# docker/

Container build and compose assets for the platform.

One image, like one binary: the `node` runs everywhere and selects edge vs cloud
by **config and role**, never a separate image per role (`../README.md` §3, rule 1).
So expect a single `Dockerfile` producing one `node` image, plus compose files that
start it in different roles for local dev.

- **What goes here:** the `node` `Dockerfile` (multi-stage Rust build), a
  `compose.yml` for a local multi-node setup (edge + cloud roles, talking over
  Zenoh), and any `.dockerignore` / entrypoint scripts.
- **Persona dirs (e2e fixtures).** The local end-to-end setup is named by
  **deployment persona** (`../README.md` §5), not by role — one dir per persona,
  each a thin config wrapper over the *same* image:
  - `hub/` — the cloud-hub role (router, authority, gateway, registry).
  - `appliance/` — an **edge** node, headless (Raspberry-Pi class), full stack.
  - `workstation/` — an **edge** node with the desktop UI; same stack as `appliance`.
  - `mobile/` — the Flutter mobile **client** (not a node): a thin UI to the hub gateway.
  The canonical fixture is `hub` + `appliance` + `workstation` (two edge nodes, one
  hub), with `mobile` optional. These replace the placeholder `cloud / edge-1 / edge-2`.
- **One datastore.** SurrealDB is embedded in the node — no separate database
  service in compose (`../README.md` §3, rule 2). Persist its data with a volume.
- **Config, not branches.** Roles are chosen via env/config passed to the
  container, matching how the binary selects roles.
- **Extensions** are WASM/native artifacts mounted or fetched at runtime, not baked
  per-image.

Before adding files, read [`../docs/FILE-LAYOUT.md`](../docs/FILE-LAYOUT.md): one
responsibility per file — prefer `compose.<role>.yml` and a `docker/entrypoint/`
folder of small scripts over one catch-all file.

Status: not yet scaffolded — architecture scope only. When set up, document the
`docker build` / `compose up` commands here.
