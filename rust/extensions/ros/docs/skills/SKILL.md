---
name: ros
description: >-
  STUB — written for real by the implementing session, grounded in a live run. Manage a fleet of ROS
  (Rubix) REST appliances over the node gateway: create/list ROS connections, browse the
  network → device → point tree, toggle polling at any tree level, read polled values from `series`,
  and write setpoints (`point.write`). Use when a task says "add a ROS box", "poll ROS points",
  "disable polling for a network/device", "write a ROS setpoint", or "drive the ros extension over
  the API". Covers the `ros|network|device|point.*` MCP verbs, `point.write`, the `ros.start|stop|status`
  poller controls, and the `series.latest|read` read path for live values.
---

# Driving the ROS extension (STUB)

> Scope-time stub. The implementing session owns turning this into a runnable how-to, grounded in a
> live run against a real spawned gateway + a fake ROS box (per `../ros-scope.md` testing plan). The
> commands below are the *intended* surface — confirm and correct them on ship.

The `ros` extension is a native Tier-2 sidecar. Everything is reachable over the node gateway two
ways (like every extension): the universal MCP bridge `POST /mcp/call {tool, args}` and, where they
exist, dedicated REST routes. Workspace + principal come from the bearer token, never the body.

Intended verbs (see the scope's "MCP surface"):

- `ros.create {name, base_url, token}` → `{ros_uuid}` (token stashed via `lb-secrets`, never returned)
- `ros.list` / `ros.get {ros_uuid}` — the fleet; `ros.ping {ros_uuid}` — box health
- `network.list {ros_uuid}` / `device.list {network_uuid}` / `point.list {device_uuid}` — the tree
- `*.create|update|delete` per level; poll toggle = `*.update {enable:false}` at the chosen level
- `ros.start|stop|status {ros_uuid}` — arm/disarm/inspect the poll task (runnable trait)
- `point.write {point_uuid, slot, value}` — must-deliver setpoint (outbox-staged)
- live values: `series.latest {series:"ros.{ws}.{ros}.{net}.{dev}.{point}"}` and
  `series.read {series, range}` (from the S8 ingest/series surface — see `skills/ingest-series`)

To fill this in on ship: run each verb against the real gateway + fake box, paste the actual request
/ response, and document the capability each requires and the deny behavior.
