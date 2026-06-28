# Host-tools scope — built-in `host.*` introspection verbs (networking · timezone · files)

Status: scope (the ask). Promotes to `public/host-tools/host-tools.md` once the first slice ships.

The agent (and any MCP caller — the UI, another extension) keeps needing **small facts about the node
it runs on**: what time is it here and in what zone, what is this node's networking posture (its
addresses, whether it can reach a host), what does a path on the host filesystem look like. Today an
agent improvises these with `shell.run` (`date`, `ip addr`, `ls`) — which is OS-specific, ungated
beyond the coarse shell cap, unstructured (the model parses `ifconfig` text), and **broken across
platforms** (`ip` is Linux-only, `ipconfig` is Windows, `date` flags differ on BSD/macOS).

This scope adds a **first-class `host.*` MCP verb family**: a handful of tiny, read-shaped,
cross-platform host-introspection tools, each its own capability, returning **structured JSON** the
model never has to parse. They are the in-process, host-mediated answer to "tell me about this
machine" — the same chokepoint (`caps::check`, workspace-first) every other tool runs through, never a
shell-out. We start with three folders — **networking**, **timezone**, **files** — and a layout that
makes "add another host fact" a new file, not a new mechanism.

> Read with: `../../README.md` §6.5 (MCP / tool layer — the contract these are), §6.6 (caps project
> onto the dispatch chokepoint), §6.7 (secrets — what these verbs must *never* leak), §7 (workspace =
> tenant), `../mcp/mcp-scope.md` (the tool surface + `authorize_tool` gate), `../files/files-scope.md`
> (**workspace doc assets** — deliberately *not* what `host.files` is; see Non-goals), `../auth-caps/
> auth-caps-scope.md` (the cap grammar + deny path), `../extensions/reference-extensions-scope.md`
> (the `net:*` family — why `host.net` is its read-only cousin, not the same thing).

---

## Goals

- A **`host.*` MCP verb family** of small, read-shaped, **cross-platform** node-introspection tools,
  each returning structured JSON, each gated by its own `mcp:host.<verb>:call` capability, all running
  the standard workspace-first dispatch gate (`tool_call.rs` → `authorize_tool`).
- **Three folders to start**, one responsibility per file (FILE-LAYOUT — folder-of-verbs):
  - **`networking/`** — `host.net.info` (this node's hostname + interface addresses, loopback/private/
    public classification) and `host.net.reach {host, port}` (can this node open a TCP connection to a
    host:port, with a bounded timeout — a *reachability probe*, not a port scanner).
  - **`timezone/`** — `host.time.now` (current instant: UTC + the node's local time + the IANA zone
    name + UTC offset) and `host.time.zones` (list the known IANA zone ids, for a picker).
  - **`files/`** — `host.fs.stat {path}` (does this host path exist; is it a file/dir/symlink; size;
    mtime; readable/writable by the node) and `host.fs.list {path}` (one directory level: entries with
    name + kind + size). **Host-OS paths**, normalized so the *result shape* is identical on Windows
    and Unix even though the inputs differ.
- **One implementation, every OS.** No `if cfg!(windows)` branch in a *verb*; platform differences are
  isolated behind a per-folder `platform` seam (see Intent) so the verb file is OS-agnostic and the
  symmetric-node rule holds (the differences are environment, not a code fork of the feature).
- **Deny by default, leak nothing.** Each verb has its own cap and its own **deny test**; the
  networking + files verbs are the ones an operator can withhold entirely (a locked-down workspace
  grants `host.time.*` but not `host.fs.*`). No verb returns secrets, env vars, or file *contents*.

## Non-goals

- **Workspace file/doc assets.** `host.fs.*` is **node-filesystem introspection**, a totally different
  thing from `../files/files-scope.md` (workspace-scoped, capability+membership-gated *document
  assets* in SurrealDB). `host.fs.*` never reads SurrealDB and is **not** a document store. The two
  share the word "files" and nothing else — this Non-goal exists precisely so they are never conflated.
- **Reading or writing file *contents*.** `host.fs.*` is **metadata only** (exists / kind / size /
  mtime / a single directory level). No `read`, no `write`, no `cat`. Reading host file *bytes* into an
  agent is a much larger trust decision (it can exfiltrate `/etc/shadow`, `.env`, ssh keys) and is
  deferred behind its own future scope + a stricter cap, not smuggled into a "stat" verb.
- **A general networking client.** `host.net.reach` is a **boolean reachability probe** (connect,
  immediately close, report can/can't + latency). It is **not** an HTTP client, not a socket the agent
  owns, not a port scanner (no ranges/sweeps). An extension that needs to *own* an external socket uses
  the `net:*` capability family (`../extensions/reference-extensions-scope.md`), which is a different
  (write-shaped, long-lived) grant — `host.net` is its read-only, point-probe cousin.
- **Cron / scheduling / "wait until".** `host.time.*` reports time; it does not schedule. Timers and
  durable wakeups are the jobs/outbox concern (README §6.9/§6.10).
- **A second tool mechanism.** These are **host-native verbs** on the existing `tool_call.rs` bridge
  (like `agent.*`, `bus.*`, `inbox.*`), **not** runtime-registry components and **not** a new plugin
  surface. One contract (§6.5), one gate (§6.6).
- **Mutating the host.** Nothing here changes node state (no `setenv`, no `mkdir`, no time-set). The
  whole family is **read-only introspection** — which is what lets it be broadly granted without an
  approval gate.

## Intent / approach

**A `host.` verb prefix on the existing bridge — the cheapest correct shape.** `tool_call.rs` already
dispatches host-native verb families by prefix (`agent.`, `bus.`, `inbox.`, `dashboard.`, …). We add
exactly one more arm — `host.` → `call_host_tool` — mirroring `call_agent_tool` precisely:
`call_host_tool` matches the qualified verb (`host.net.info`, `host.time.now`, …), runs
`authorize_tool(principal, ws, verb)` (opaque `Denied`), and delegates to the per-folder handler. No
new dispatch path, no new auth path, no registry. This is the symmetric dual of how `agent.*` was
added (`crates/host/src/agent/tool.rs`) — read that file as the template; this scope produces its
sibling `crates/host/src/host_tools/`.

**Folder-of-verbs, one file per verb (FILE-LAYOUT §2).** The crate layout *is* the API:

```
crates/host/src/host_tools/
  mod.rs                 ← barrel + `call_host_tool` dispatch (verb match + the gate), ≤80 lines
  net/
    mod.rs               ← re-exports + the `host.net.*` sub-match, ≤30 lines
    info.rs              ← host.net.info   — hostname + classified interface addresses
    reach.rs             ← host.net.reach  — bounded TCP connect probe
    platform.rs          ← the ONE place OS differences live (enumerate interfaces) — see below
  time/
    mod.rs               ← re-exports + the `host.time.*` sub-match
    now.rs               ← host.time.now   — UTC + local + IANA zone + offset
    zones.rs             ← host.time.zones — the IANA zone-id list
  fs/
    mod.rs               ← re-exports + the `host.fs.*` sub-match
    stat.rs              ← host.fs.stat    — metadata for one path
    list.rs              ← host.fs.list    — one directory level
    path.rs              ← path normalization (Windows ⇄ Unix) so result shape is identical
```

Each verb file is the *whole* responsibility for that verb: parse args → call the platform/std API →
shape the JSON DTO → return. None exceeds ~120 lines.

**Cross-platform is a seam, not a branch (the load-bearing decision).** The symmetric-node rule (no
`if cloud {…}`, by extension no per-OS fork of a *feature*) applies here as: the **verb** is
OS-agnostic; only the **lowest-level fact-gathering** differs by OS, and that difference is isolated in
exactly one named file per folder:
- **Networking:** interface enumeration is the only OS-specific part. We take a cross-platform crate
  (recommend `local-ip-address` / `if-addrs` — both pure-Rust, Windows+Unix; key-stack row added) so
  even `platform.rs` has no raw `cfg`. `host.net.reach` is `std::net::TcpStream::connect_timeout` —
  already cross-platform, no seam needed.
- **Timezone:** `time`/`chrono` + `iana-time-zone` (pure-Rust, reads the OS zone on Win/macOS/Linux)
  give UTC, the local offset, and the IANA name uniformly. No `cfg`.
- **Files:** `std::fs::metadata`/`symlink_metadata`/`read_dir` are already cross-platform; the only
  divergence is **path syntax** (`C:\…` vs `/…`, separators, case), normalized in `fs/path.rs` so the
  *returned* shape (always forward-slash-normalized + an explicit `os` discriminator) is identical.
  `readable`/`writable` is reported as an attempted-access boolean (portable) rather than raw Unix mode
  bits, so Windows isn't a special case in the result.
The rule we hold: **a reviewer grepping the verb files finds zero `cfg!(windows)`**; every OS `cfg`
lives in a `platform.rs`/`path.rs` named seam, behind a cross-platform crate where one exists.

**Why not just keep using `shell.run`?** Rejected. `shell.run` is (a) OS-specific at the call site (the
model must know `ip` vs `ipconfig`), (b) unstructured (the model parses human-formatted text, fragile
across locales/versions), (c) gated only by the coarse shell cap — you cannot grant "may ask the time"
without granting "may run arbitrary commands", and (d) a much larger attack surface. A typed `host.*`
family is *more* secure (finer caps, no shell), *more* reliable (structured, version-stable), and
*portable* — exactly the three things `shell.run` fails at here.

**Why a `host.` prefix and not folding into existing families?** Time/net/fs are **node-environment
facts**, orthogonal to `agent.*` (run control), `bus.*` (motion), `store.*` (state). A new top-level
prefix keeps the cap names self-describing (`mcp:host.fs.stat:call` reads exactly as what it grants)
and leaves room for the obvious next siblings (`host.os.info`, `host.proc.self`) as new files under the
same mechanism.

## How it fits the core

- **Tenancy / isolation:** these verbs report **node** facts, not workspace data — but the **gate is
  still workspace-first**. The call carries the caller's workspace; `authorize_tool(principal, ws, …)`
  checks the workspace wall *then* the cap exactly like every other verb, so a token with no valid
  workspace is denied before any host fact is read. There is no cross-workspace leak risk in the
  *result* (the node's clock/IP are the same regardless of caller), but the **deny path is identical**:
  ws-B with the cap and ws-A without it both go through the same chokepoint, and the isolation test
  asserts a token scoped to no/other workspace can't call. (The subtlety — node facts aren't
  per-workspace — is called out in Risks so it isn't mistaken for "isolation N/A".)
- **Capabilities:** **one cap per verb**, deny by default:
  `mcp:host.net.info:call`, `mcp:host.net.reach:call`, `mcp:host.time.now:call`,
  `mcp:host.time.zones:call`, `mcp:host.fs.stat:call`, `mcp:host.fs.list:call`. Granularity is the
  point: an operator can grant the harmless `host.time.*` broadly while withholding `host.fs.*` and
  `host.net.reach` (the egress/disclosure-sensitive ones). The deny is the opaque `Denied` from
  `authorize_tool` — **a deny test per verb** (HOW-TO-CODE §3 step 4a). No verb ever bypasses
  `caps::check`.
- **Placement:** *either*, by config — and the **result is intentionally node-local**: `host.time.now`
  on an edge node reports *that edge node's* zone; `host.net.info` reports *that node's* interfaces.
  This is correct, not a bug: the verb answers "about the node I ran on." No `if cloud {…}`; the same
  binary serves it everywhere. (When a remote caller routes a `host.*` call to another node, it gets
  *that* node's facts — the natural meaning; routing is the existing MCP routing seam, unchanged.)
- **MCP surface** (§6.1 — judged, not defaulted; this family is deliberately **read-only**):
  - **Get / list (all of it):** every verb is a **read**. `host.net.info`, `host.time.now`,
    `host.fs.stat` are single-`get`-shaped; `host.time.zones` and `host.fs.list` are `list`-shaped
    (bounded — `host.fs.list` is **one directory level**, no recursion; it states its cap on entry
    count and truncates with a `truncated: true` flag rather than walking a tree). `host.net.reach` is
    a parametric read (a probe with a `{host, port}` input returning `{reachable, latency_ms}`).
  - **CRUD: N/A — and that's a stated decision, not an omission.** There are **no write verbs**: the
    family does not mutate the host (Non-goals). A read-only roster with no `create/update/delete` is
    exactly the §6.1 "read-only roster has no write verbs and should say so" case.
  - **Live feed: N/A.** These are point-in-time snapshots; nothing here is a stream. (A future
    `host.net.watch` for interface up/down *could* be a `watch` over the bus, but it is out of scope —
    no caller needs motion today.)
  - **Batch: N/A.** Each call is a single bounded fact; there is no bulk surface and nothing that runs
    long (every verb is sub-second; `host.net.reach` is **bounded by an explicit connect timeout** so
    it can never block a run — see Risks). No job is needed.
- **Data (SurrealDB):** **none.** This family touches **no tables, no buckets** — it reads the OS, not
  the store. (This is the cleanest way to keep it distinct from `host.files`'s namesake, the
  *workspace* file assets, which are all SurrealDB.) The one-datastore rule is upheld trivially: no new
  persistence is introduced.
- **Bus (Zenoh):** **none** in v1 (no live feed). State-vs-motion is N/A: there is neither durable
  state nor motion — just synchronous reads of the local OS.
- **Sync / authority:** N/A — node-local facts, nothing synced, nothing authoritative beyond "the node
  you asked." Offline behavior is trivially correct (the node can always read its own clock/fs even
  fully offline; `host.net.reach` to an unreachable host simply returns `reachable: false`).
- **Secrets:** **none stored or returned — and a hard line to hold.** The verbs are explicitly designed
  to *not* leak: `host.net.info` returns interface addresses but **never** routing tables, ARP, or
  hostnames of *other* machines beyond the probe target; `host.fs.*` returns **metadata only, never
  contents** and never the values of env vars; nothing returns provider keys or tokens (§6.7). The
  "leak nothing" property is a per-verb test, not a hope.

## Example flow

An agent is drafting a deploy note and needs to know the node's timezone and whether the artifact host
is reachable.

1. The model calls `host.time.now {}`. `tool_call.rs` sees the `host.` prefix → `call_host_tool` →
   `authorize_tool(principal, ws, "host.time.now")` passes (the workspace grants `mcp:host.time.now:call`)
   → `time::now` reads the OS clock + `iana-time-zone` and returns
   `{ "utc": "2026-06-28T04:12:09Z", "local": "2026-06-28T14:12:09+10:00", "zone": "Australia/Sydney", "offset_seconds": 36000 }`.
   The model gets structured fields — no `date`-output parsing, identical shape whether the node is on
   Linux, macOS, or Windows.
2. The model calls `host.net.reach {host: "registry.internal", port: 443}`. The gate passes; `net::reach`
   does a `TcpStream::connect_timeout(.., 2s)`, succeeds, closes immediately, returns
   `{ "reachable": true, "latency_ms": 7 }`. It is a *probe*, not an owned socket — nothing is held.
3. The model then tries `host.fs.list {path: "/srv/artifacts"}`. This workspace's policy **does not**
   grant `mcp:host.fs.list:call`, so `authorize_tool` returns the opaque `Denied`; the loop feeds the
   model a "denied by policy" tool result (the existing tool-error path) and the model proceeds without
   the listing. The deny is identical to any other capability deny — same chokepoint, same opaque error.
4. On a **Windows** node the same `host.fs.stat {path: "C:\\srv\\artifacts\\build.zip"}` would, if
   granted, return the *same-shaped* DTO — `{ "exists": true, "kind": "file", "size": 10485760,
   "mtime": "...", "readable": true, "writable": false, "path": "C:/srv/artifacts/build.zip", "os": "windows" }`
   — the only difference being the `os` discriminator and the normalized path; the model's handling is
   unchanged across platforms. That cross-OS uniformity is the whole point.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`) — the gate, not extras. **No mocks**: tested
against the **real** node + real caps store (`mem://`) and the **real OS** of the test runner (the verbs
read the actual clock / loopback interface / a real temp dir seeded on disk — there is nothing to fake;
a `*.fake.ts` host would defeat the entire point of "real host facts"). The OS-specific seam is
exercised on whatever platform CI runs; the **shape** assertions are platform-independent by design.

- **Capability-deny (mandatory, §2.1) — one per verb (HOW-TO-CODE §3 step 4a):** each of the six verbs
  is **denied** without its `mcp:host.<verb>:call` cap and returns the opaque `Denied` — six deny
  tests, the core security gate.
- **Workspace-isolation (mandatory, §2.2):** a token scoped to no workspace / a different workspace
  than its grant is denied at the workspace-first gate **before** any host fact is read — proving the
  chokepoint is workspace-first even though the *result* is node-global (the Risks subtlety, made a test).
- **Cross-platform shape (the headline correctness test):** for each verb, assert the **result DTO
  shape is identical** regardless of OS — same JSON keys, same types, the `os`/path normalization
  applied. Run on Linux in CI; the assertions contain no OS-conditional branches (if they had to, the
  normalization seam failed). Seed a **real temp directory** on disk for `host.fs.*` (a real file + a
  real subdir), call the real verb, assert the real metadata.
- **Bounded probe (`host.net.reach`):** a connect to a guaranteed-closed local port returns
  `reachable: false` **within the timeout** (no hang); a connect to a real listening socket seeded in
  the test (bind an ephemeral `TcpListener`) returns `reachable: true` with a latency. Proves the probe
  is bounded and uses the real network stack, not a mock.
- **Leak-nothing (the secrets guard, §6.7):** assert `host.fs.stat`/`host.fs.list` results contain **no
  file contents** and `host.net.info` contains **no** unexpected fields (a positive allow-list assertion
  on the DTO keys) — a regression test that a future field-add can't silently start leaking bytes.
- **Offline (trivial but asserted):** `host.time.now` and `host.fs.stat` succeed with the network down;
  `host.net.reach` to an unroutable host returns `reachable:false` (not an error) within the timeout.
- **Unit:** `fs/path.rs` normalization (Windows + Unix inputs → identical normalized output + correct
  `os`); the interface classifier (loopback/private/public buckets); the IANA-zone read returns a
  non-empty zone + a consistent offset.
- **Frontend (if surfaced):** if the UI exposes any of these (e.g. a "node info" panel via `http.ts`),
  a Vitest against a **real spawned gateway** (`pnpm test:gateway`) — not a fake — asserting the panel
  renders the real node's time/zone. If v1 is backend-only (agent-facing), state that explicitly and
  defer the UI verb.

## Risks & hard problems

- **Cross-platform is the whole risk — and it hides in the verb if you let it.** The failure mode is a
  `cfg!(windows)` creeping into `info.rs`/`stat.rs` "just this once," at which point the feature has
  forked per-OS and the symmetric-node rule is violated in spirit. Containment is structural: **every**
  OS `cfg` lives in `platform.rs`/`path.rs`, behind a cross-platform crate where one exists; the test
  asserting identical result shape across OS is the canary.
- **`host.net.reach` must be strictly bounded.** An unbounded `connect` to a black-holed host hangs the
  agent run. It **must** use `connect_timeout` with a small, capped, server-side-enforced timeout
  (caller may lower it, never raise it past the cap) and must not accept port *ranges* (no scan). If
  this is hand-waved, one `reach` call wedges a run — the bounded-probe test exists for this.
- **`host.fs.*` is a disclosure surface even without contents.** Directory *listings* and path
  *existence* leak structure (usernames in `/home/`, deploy layout). Mitigations: it is **metadata
  only** (Non-goal: no contents), it is its **own withholdable cap** (an operator can deny it), and
  `host.fs.list` is **one level, bounded, non-recursive**. Reading contents is a separate future scope
  with a stricter gate — do **not** let "stat" grow a `read` arm.
- **"Isolation N/A" is the wrong conclusion.** Because node facts aren't per-workspace, it's tempting to
  skip the workspace gate. That's the hole: the **gate** is still workspace-first (an invalid-workspace
  token must be denied), even though the *result* is node-global. The isolation test makes this explicit
  so a later refactor doesn't quietly drop the `ws` check as "unnecessary."
- **Naming collision with workspace `files`.** `host.fs.*` vs `../files/` (workspace doc assets) will
  confuse anyone skimming. Mitigation: the verb prefix is `host.fs.*` (not `host.files.*`) and the
  folder is `host_tools/fs/`, and this scope's Non-goals state the distinction first. Hold that line.
- **Crate dependency surface.** Pulling `if-addrs` / `iana-time-zone` adds dependencies that must build
  under the project's **zig-linked, no-system-cc** toolchain (`rust/.cargo/config.toml`). Verify each
  candidate crate is pure-Rust / cross-compiles cleanly before adopting; an interface-enumeration crate
  that needs a C toolchain is disqualified. (Key-stack row + a build check in the implementing session.)
- **Scope creep into a node agent.** `host.*` will tempt "while we're here" additions (run a command,
  read a file, set the time). Resist: this family is **read-only introspection**. Anything that mutates
  or reads contents is a different scope with a different (write/contents) cap and an approval story.

## Open questions

- **Crate choices.** `if-addrs` vs `local-ip-address` for interface enumeration; `iana-time-zone` +
  `time` vs `chrono-tz` for the zone read. Decide in the session by which builds cleanly under the
  zig/no-cc toolchain and is pure-Rust (the Risks build check). Record the chosen rows in `key-stack.md`.
- **`host.net.reach` timeout cap.** What is the server-enforced maximum (proposed: 2s default, 5s hard
  cap)? And do we allow UDP at all, or TCP-only in v1 (proposed: TCP-only — UDP "reachability" is
  ill-defined)?
- **`host.fs.list` entry cap + ordering.** Max entries before `truncated: true` (proposed: 1000), and
  do we sort (proposed: name-ascending for determinism) or return OS order?
- **`readable`/`writable` semantics.** Report as attempted-access booleans (portable, proposed) or omit
  on Windows? Proposal: always the boolean pair, computed via a real access check, so the DTO is uniform.
- **Is v1 agent-facing only, or also a UI "node info" panel?** Proposed: backend/agent verbs first; the
  UI panel is a fast follow once `http.ts` needs it (state it as a non-goal of *this* slice if deferred).
- **Routing semantics for a remote `host.*` call.** Confirm a call routed to node B returns node B's
  facts (the natural meaning, via the existing MCP routing seam) and that this is documented so a
  caller isn't surprised that `host.time.now` isn't "the hub's" time.

## Related

- README `§6.5` (MCP / tool layer — these *are* MCP verbs), `§6.6` (caps → the dispatch chokepoint
  these run through), `§6.7` (secrets — the leak-nothing line), `§7` (workspace = tenant — the gate).
- `../mcp/mcp-scope.md` — the `authorize_tool` gate + tool surface these slot into.
- `../files/files-scope.md` — the **workspace doc assets** `host.fs.*` is deliberately *not*; the
  Non-goal that keeps the namesake from being conflated.
- `../extensions/reference-extensions-scope.md` — the **`net:*`** capability family for *owned*
  external sockets; `host.net.reach` is its read-only, point-probe cousin (different grant shape).
- `../auth-caps/auth-caps-scope.md` — the cap grammar + the opaque `Denied` deny path each verb reuses.
- `../jobs/jobs-scope.md` — where time-based *scheduling* lives (a Non-goal here: `host.time` reports,
  it does not schedule).
- `crates/host/src/agent/tool.rs` — the host-native `agent.*` dispatcher this family mirrors
  one-to-one (read it as the implementation template for `host_tools/`).
