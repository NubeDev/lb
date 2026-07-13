# Node roles

TODO — filled as the node-roles surface ships. Covers: roles by config (README §9), the thin
role-aware layers (§3.1), fleet presence / node connection, and (per
`docs/scope/node-roles/embed-node-scope.md`) **embedding lb as a Rust library** — the
`BootConfig` + `NodeBuilder` seam on the `node` package that the node binary, the Tauri desktop
shell, `test_gateway`, and third-party Rust programs (git-dep on `NubeDev/lb`) all boot through.

See `docs/scope/node-roles/` for the asks.

## Outbox delivery at boot (shipped 2026-07-11, `node-v0.2.0`)

The boot ritual now spawns the **outbox relay reactor**: a generic `RouterTarget` dispatches each
effect by its opaque `target` string to the registered adapter (`email` → `EmailTarget`, `push` →
`PushTarget`). Real providers are injected through the additive `BootConfig.outbox_providers`
seam (`Option<Arc<dyn EmailProvider>>` / `Option<Arc<dyn PushProvider>>`); when unset, logging
no-op providers ack the sends so boot never crashes and effects never strand. An effect whose
target has no route retries and dead-letters with a clear reason. Proof:
`rust/node/tests/relay_boot_test.rs` (drain-at-boot through the spawned reactor, both targets).
