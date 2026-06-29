# Operator CLI (`lb`)

Status: **TODO** — placeholder. Filled when the CLI ships; see `../../scope/cli/operator-cli-scope.md`.

The terminal twin of the React shell: a fourth client (besides browser/Tauri/mobile) of the gateway
surface, holding no authority of its own. Two modes — remote (over the gateway) and local (embeds the
host, offline) — both funneling through the one `lb_host::call_tool` chokepoint. Adds no new MCP verbs,
capabilities, or tables.
