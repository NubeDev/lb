# External-agent scope — the capability wall & sandbox

Status: scope (the ask). Sub-scope #3 of `external-agent-scope.md`. **The topic's safety exit gate.**
Promotes to `public/external-agent/`.

Guarantee that a third-party agent loop **we do not control** can act on the world **only** through
Lazybones' capability-checked MCP surface — never the host filesystem, network, or shell. This
sub-scope owns the layered, **fail-closed** enforcement: built-ins disabled (and proven so before the
run starts), an OS sandbox that denies all egress/fs except the gateway socket and a scratch dir, and
the protocol-level decision to **decline** ACP's filesystem/terminal capabilities so everything routes
through MCP under the derived principal. If this is not green, nothing that drives a real external agent
ships.

## Goals

- **Built-ins off, fail-closed.** Each `AgentProfile` declares how its agent disables built-in tools
  (VT Code: deny-all-builtins policy; dirge: `--no-tools`). At launch, `sandbox.rs` **asserts** the
  agent will run with no built-in tools and **aborts the run** if it cannot establish that — an open
  tool surface is never allowed to start.
- **OS sandbox.** The subprocess runs with **no network egress except the gateway socket** and **no
  filesystem except a per-run scratch dir** (and the stdio pipe). A direct provider call, a raw HTTP
  request, or a write outside scratch is impossible at the kernel level, not merely unconfigured.
- **Decline ACP fs/terminal capabilities.** During `initialize` the client advertises **only** the MCP
  tool bridge; it does **not** offer ACP's filesystem or terminal client capabilities. "Everything is
  MCP" starts at the handshake.
- **MCP-only, derived-principal tool exposure.** The agent sees exactly the **derived principal's
  granted** MCP tools (`caller ∩ agent`, agent scope), optionally narrowed by the profile — never
  widened. Every call re-runs `caps::check`, workspace-first.
- **The wall test is the gate.** A real external agent, scripted to try a tool it isn't granted (and to
  try to touch fs/net directly), is **denied/contained** every way — proven, not asserted.

## Non-goals

- Talking ACP / spawning / encoding (#2) — this sub-scope wraps #2's `spawn.rs` with the sandbox and the
  fail-closed assertion; it doesn't drive the protocol.
- Defining the gateway endpoint or token (#4) — the sandbox *allows* exactly the gateway socket; #4 says
  what listens there and how the token is scoped.
- The derived-principal **derivation** itself — that's the auth-caps/agent-scope intersection this reuses.

## Intent / approach

**Defence in depth, each layer fail-closed.** Three independent layers, any one of which alone would
mostly hold, and which together leave no path:

1. **Protocol layer** — advertise only the MCP bridge; decline ACP fs/terminal. The agent can't even
   *ask* for a filesystem the protocol way.
2. **Config layer** — built-ins disabled via the profile's documented switch, **verified at launch**.
   If the agent can't be shown to start tool-less, the run aborts (fail-closed, not best-effort).
3. **Kernel layer** — an OS sandbox (Linux: namespaces/seccomp/landlock-style egress+fs restriction;
   the exact mechanism is an open question) so that even a misbehaving or compromised agent binary
   physically cannot reach the network (except the gateway socket) or the fs (except scratch).

**Why three layers and not one.** We are running an **untrusted, third-party loop**. The config layer
(built-ins off) depends on the agent honoring its own flags; the protocol layer depends on the agent
not having out-of-band tools; only the kernel layer is independent of the agent's cooperation. Treat the
sandbox as **load-bearing**, not hygiene — it is the layer that holds when the other two are
mis-profiled or the agent misbehaves.

**The wall is the MCP chokepoint, not the loop.** This is the same principle as the in-house runtime:
isolation and capability checks live at `lb_mcp` → `caps::check`, which the #2 bridge routes every tool
call through. The sandbox + built-ins-off exist precisely to guarantee there is **no second path** that
skips that chokepoint.

**Rejected: trusting the agent's own permission engine.** dirge ships a Policy Decision Point; VT Code
ships tool policies. They're good, but they are the *agent's* policy, not ours — relying on them would
put the wall inside a process we don't control. We disable them down to "no tools" and re-impose the
*only* policy that matters (ours) at the bridge + sandbox.

## How it fits the core

- **Tenancy / isolation:** the bridge exposes only `ws`-scoped, derived-principal tools; the scratch dir
  and any cache are per-run and per-`ws`. A workspace-B run can't see workspace-A tools/docs/skills —
  proven across **store + MCP** (mandatory isolation test).
- **Capabilities:** the deny path is the headline. A tool neither the caller nor the agent holds is
  denied at `caps::check` and returned as an ACP tool error; built-ins-off means there is no ungated tool
  to begin with. (Mandatory capability-deny test.)
- **Placement:** `either`. The sandbox mechanism may differ by OS, but the *contract* (no egress but the
  gateway, no fs but scratch, no built-ins) is identical — config/role, not an `if cloud {…}`.
- **MCP surface:** consumes only; adds no verb.
- **Data / Bus:** none — the wall is enforcement around the subprocess, not a record.
- **Secrets:** the sandbox is *why* withholding the provider key is safe (#4) — even a key-curious agent
  can't exfiltrate via egress it doesn't have.
- **No fake backend (rule 9):** the wall test runs the **real** agent binary; the only fake is the
  provider HTTP (scripted, behind the gateway), used to *drive* the agent into attempting the denied
  actions deterministically.

## Example flow

1. #2's `spawn.rs` calls `sandbox.rs` before launch. `sandbox.rs` constructs the OS sandbox (egress →
   gateway socket only; fs → scratch only) and verifies the profile's built-ins-off switch is applied;
   if it can't verify, it **aborts the run** (the job ends `failed: unsafe-profile`, no subprocess).
2. The agent launches tool-less inside the sandbox; the client `initialize` advertises only the MCP
   bridge (no fs/terminal).
3. The agent (scripted) tries to call an ungranted MCP tool → `caps::check` **denies** → ACP tool error.
4. The agent (scripted) tries to open a file outside scratch / a socket other than the gateway → the
   **kernel** denies it; the attempt is logged, the run is unaffected (or terminated per policy).
5. Every successful action is a granted MCP call under the derived principal — there is no other way out.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`) — **this is the topic's safety gate**:

- **Capability-deny (§2.1):** real agent, scripted to call an ungranted tool → `caps::check` deny, effect
  never occurs; a tool the *caller* lacks is denied even if the agent's profile lists it (intersection
  holds, no widening).
- **Built-ins-off fail-closed:** a profile whose built-ins-off switch is missing/ineffective → the run
  **aborts at launch**; no subprocess with an open tool surface ever starts.
- **Sandbox containment:** a real agent scripted to (a) open a non-scratch file and (b) connect to a
  non-gateway host is **denied by the OS sandbox**; assert the syscalls fail and nothing leaked.
- **Decline-caps handshake:** assert the `initialize` advertises only the MCP bridge (no fs/terminal),
  and a fs/terminal request from the agent is unsupported.
- **Workspace-isolation (§2.2):** a ws-B run cannot list/call ws-A tools across **store + MCP**; scratch
  + cache are ws/run-scoped.

## Risks & hard problems

- **The sandbox is the whole ballgame, and it's OS-specific.** Linux namespaces/seccomp/landlock differ
  from macOS sandbox-exec; getting the egress allowlist (gateway socket only) and fs (scratch only)
  exactly right is fiddly and easy to loosen "to make it work." Treat any loosening as a finding. Pick
  the mechanism explicitly (open question) and test containment, don't assume it.
- **Built-ins-off verification is per-agent and can rot.** The switch differs by agent and could change
  across agent versions; pin the agent version (profile, #2) and re-verify on upgrade. Fail-closed means
  an unverifiable upgrade stops runs rather than silently opening the surface.
- **Out-of-band tools.** An agent with a built-in MCP-client of its own could try to reach a *different*
  MCP server; the egress sandbox (gateway socket only) blocks it — another reason the kernel layer is
  non-negotiable.
- **Performance / startup cost** of constructing a sandbox per run; acceptable for a hub subprocess, note
  it for edge.

## Open questions

- **Sandbox mechanism:** Linux (namespaces + seccomp + landlock?) and the macOS story — one abstraction
  with per-OS backends, behind one trait in one file. Which crate(s), if any?
- **Containment-failure policy:** log-and-continue vs terminate-the-run when the kernel blocks an attempt
  (i.e., the agent *tried* something it shouldn't). Proposal: terminate + audit, since a compliant agent
  never tries.
- **CI for sandbox tests:** containment tests need real OS primitives; which CI runners, and the
  fallback when a runner can't provide them (don't let the gate silently skip). (Shared with #2.)
- **Egress to the gateway:** unix socket vs localhost TCP — the narrower the allowlist, the better.

## Related

- `external-agent-scope.md` (umbrella), `acp-driver-scope.md` (#2, `spawn.rs` calls `sandbox.rs`),
  `model-routing-scope.md` (#4, the one allowed egress).
- `scope/auth-caps/auth-caps-scope.md` (the derived `caller ∩ agent` principal), `scope/agent/agent-scope.md`
  (the intersection + deny path), `scope/mcp/mcp-scope.md` (the `caps::check` chokepoint). README `§6.5`, `§7`.
