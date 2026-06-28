# Host tools (`host.*`) — node introspection verbs

> **TODO (stub).** This page gets *filled* when the first `host-tools` slice ships. Until then the
> authoritative source is `../../scope/host-tools/host-tools-scope.md`.

A built-in, cross-platform `host.*` MCP verb family for reading small facts about the node a call runs
on — structured JSON, one capability per verb, no shell-out. First three folders:

- **networking** — `host.net.info`, `host.net.reach`
- **timezone** — `host.time.now`, `host.time.zones`
- **files** — `host.fs.stat`, `host.fs.list` (node-filesystem **metadata**, *not* workspace doc assets)

Read-only introspection: no writes, no file contents, no secrets. See the scope for the full contract.
