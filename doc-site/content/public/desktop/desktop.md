# Desktop

The Lazybones desktop app is the Tauri v2 shell (`ui/src-tauri`, crate `lazybones-shell`) — the
node running in-process with a window attached (the `workstation` persona).

Two build modes ship from one source (config, not a code branch — symmetric nodes, §3.1):

- **Thin shell** (`--features desktop`) — the window + a small IPC command layer.
- **Full standalone** (`--features desktop,full`) — the shell mounts the SSE/HTTP gateway
  in-process on a loopback port and runs the boot seeders, so the packaged binary is a 100%
  standalone node: login, MCP, SSE, agents, flows, rules, insights — no external node. Ships for
  Linux (links `webkit2gtk-4.1`) and Windows (OS-provided WebView2 → genuinely standalone).
  Scope: `docs/scope/desktop/desktop-standalone-backend-scope.md`.

Datasources in the full standalone build require the **federation sidecar**, which is bundled into
the `full` package and auto-installed at boot (with a `net:*` grant for the local sqlite convention,
`127.0.0.1:0`), so the shipped `demo-buildings.db` registers **and** queries with zero setup — no
`make dev`. The desktop default is **sqlite-only**: a postgres source still registers, but its
endpoint is refused pre-connect until an admin widens the grant (deferred). Set
`LB_DESKTOP_NO_DEMO_SOURCE=1` to skip pre-registering the demo. Scope:
`docs/scope/desktop/desktop-federation-bundle-scope.md`.

Packaging, build container, and platform targets: `docs/scope/desktop/` +
`desktop/docs/<os>/`.
