# `ros` — ROS (Rubix) driver extension

A native (Tier-2) extension that manages a fleet of ROS REST appliances as capability-gated
resources, polls their points into the platform's time-series, and writes setpoints back.

This is **100% an extension** — all of its docs live here, nothing in the repo-root `docs/` tree
(same convention as `control-engine`).

- **Authoritative scope:** [`docs/ros-scope.md`](docs/ros-scope.md).
- **Skill (drive over the API):** [`docs/skills/SKILL.md`](docs/skills/SKILL.md) (stub — filled on
  ship, grounded in a live run).
- **Manifest:** [`extension.toml`](extension.toml) (the CRUD + `point.write` + poller verbs).

## Layout (to build)

```
rust/extensions/ros/
  extension.toml          # the manifest (present, stub tool descriptors)
  docs/ros-scope.md       # the authoritative scope
  Cargo.toml              # the ros-sidecar bin (to add to the workspace members)
  src/
    main.rs               # sidecar entry (stdio-supervised MCP server)
    ros_api.rs            # the RosApi trait — the ONE external-fake seam (testing-scope §0)
    ros_client/           # vendored rust-ros (ported to async reqwest; sqlx dropped)
    poller/               # the reusable engine: poller.rs (loop), source.rs (trait),
                          #   sink.rs (ingest.write), gating.rs (enable AND up the tree)
    handlers/             # one file per MCP verb (ros_list.rs, point_write.rs, …)
  ui/                     # the federated shadcn/Tailwind-v4 page (fleet-monitor pattern)
```

The `rust-ros` client currently at `/home/user/code/rust/rust-ros` is vendored into
`src/ros_client/` — **ported to async `reqwest`** and with its `sqlx`/Postgres dependency dropped
(the ROS box owns its own DB; we speak REST only). See the scope's open questions.
