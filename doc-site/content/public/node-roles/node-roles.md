# Node roles

TODO — filled as the node-roles surface ships. Covers: roles by config (README §9), the thin
role-aware layers (§3.1), fleet presence / node connection, and (per
`docs/scope/node-roles/embed-node-scope.md`) **embedding lb as a Rust library** — the
`BootConfig` + `NodeBuilder` seam on the `node` package that the node binary, the Tauri desktop
shell, `test_gateway`, and third-party Rust programs (git-dep on `NubeDev/lb`) all boot through.

See `docs/scope/node-roles/` for the asks.
