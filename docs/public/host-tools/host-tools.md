# Host tools (`host.*`) — node introspection verbs

Shipped 2026-06-28: a built-in, read-only `host.*` MCP family for small node-local facts. These are
host-native tools, not extension registry components and not shell commands. They run through the same
MCP chokepoint as other host verbs: workspace-first, then `mcp:<verb>:call`, with opaque `Denied`.

## Verbs

| Verb | Capability | Shape |
| --- | --- | --- |
| `host.net.info {}` | `mcp:host.net.info:call` | `{hostname, interfaces:[{name, addresses:[{ip,family,scope}]}]}` |
| `host.net.reach {host, port, timeout_ms?}` | `mcp:host.net.reach:call` | `{host,port,reachable,latency_ms,timeout_ms,error}` |
| `host.time.now {}` | `mcp:host.time.now:call` | `{utc,local,zone,offset_seconds}` |
| `host.time.zones {}` | `mcp:host.time.zones:call` | `{zones,count}` |
| `host.fs.stat {path}` | `mcp:host.fs.stat:call` | `{path,os,exists,kind,size,mtime,readable,writable}` |
| `host.fs.list {path}` | `mcp:host.fs.list:call` | `{path,os,entries:[{name,kind,size}],truncated}` |

## Boundaries

`host.fs.*` is node-filesystem metadata only. It does not read workspace document assets, never reads file
contents, and never writes. `host.fs.list` is one directory level, name-sorted, capped at 1000 entries,
and marks `truncated` instead of recursing.

`host.net.reach` is TCP-only and uses `TcpStream::connect_timeout`. Default timeout is 2 seconds; callers
may lower it, but the host caps it at 5 seconds. It accepts one host and one port, not ranges.

The v1 surface is backend/agent-facing only. There is no dedicated UI panel in this slice; existing UI,
extension pages/widgets, and agents can reach these through the generic MCP bridge when granted.

## Platform

The DTO shape is identical across Windows and Unix. OS differences are isolated below the verb files:
network interface enumeration is in `host_tools/net/platform.rs`; path normalization is in
`host_tools/fs/path.rs`. Filesystem paths are returned with forward-slash separators plus an `os`
discriminator.

A routed `host.*` call reports facts for the node where the verb executes. Asking a remote node for
`host.time.now` returns that remote node's local time and zone.

## Tests

Backend coverage lives in `rust/crates/host/tests/host_tools_test.rs`: six per-verb deny tests, an
other/empty-workspace denial test, cross-platform DTO allow-list assertions, bounded reachability over a
real loopback `TcpListener`, port-range and port-zero refusal, and file/network leak allow-lists. The
focused green run, `cargo fmt --check`, and the workspace verification notes are recorded in
`../../sessions/host-tools/host-tools-session.md`.
