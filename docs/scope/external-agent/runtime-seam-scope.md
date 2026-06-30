# External-agent scope — the `AgentRuntime` seam & the compile-time feature

Status: scope (the ask). Sub-scope #1 of `external-agent-scope.md` (the foundation). Promotes to
`public/external-agent/`.

Define the **host-owned `AgentRuntime` seam** that lets a run be served by either the in-house loop
(default) or an external ACP agent (optional), and the **`external-agent` cargo feature** that makes
the external path *compile-time* optional. This sub-scope ships **no agent driving** — it ships the
trait, the registry/selection, the feature wiring, and the proof that the node builds and tests
**both with the feature off and on**. Everything else in the topic plugs in here.

## Goals

- A trait `AgentRuntime` in `lb-host` (beside `ModelAccess`): *run a bounded loop toward a goal, using
  only the tools at a given MCP endpoint, emitting `RunEvent`s; return when done or ceiling-hit.* The
  signature is caller-agnostic — `agent.invoke`, a job, or the UI call it identically.
- A **runtime registry** on the node: the **in-house loop is always registered** (the default); the
  `AcpRuntime` (sub-scope #2) registers itself **only when the `external-agent` feature is on**.
  Selection is by `AgentProfile` id (config); absent/unknown id → the default.
- The **`external-agent` cargo feature, OFF by default**, in a dedicated role crate
  `lb-role-external-agent`. With the feature off: the ACP-SDK dependency tree is **not compiled**, no
  external-agent code exists, the node runs on the in-house runtime, and `cargo build`/`cargo test`
  are green. With it on: `AcpRuntime` is available to the registry.
- **No new caller capability and no new caller code path.** Callers already hold
  `mcp:agent.invoke:call`; choosing a runtime is an argument resolved against the node's configured
  registry, not a new grant.

## Non-goals

- The `AcpRuntime` implementation itself (spawn/bridge/encode) — #2.
- The sandbox / built-ins-off enforcement — #3. (#1 only defines *where* a runtime is selected; #3
  guarantees the selected external runtime is safe.)
- Model routing (#4) and the run job / resume / supervision (#5).
- A per-workspace runtime **policy** or an `agent.profile.*` admin CRUD — selection is node config in
  this slice; policy is an open question carried by the umbrella.

## Intent / approach

**Copy the `ModelAccess`/`Provider` move exactly, one level up.** `lb-host` owns the trait; roles
depend on host, never the reverse. The in-house loop (`scope/agent/`) gets a blanket impl
unconditionally. `lb-role-external-agent` supplies `impl AgentRuntime for AcpRuntime` **gated by the
feature**, and the node binary enables the feature (and pulls the role crate) only when built for it —
the same way the binary already composes optional roles.

**The feature gate, not a runtime flag — deliberately.** The ACP SDK + transitive deps are weight a
minimal/Pi profile must not pay, and a node that will never drive an external agent should not carry
the attack surface. A cargo feature gives a *clean* build with the code absent — and it stays
consistent with rule 1: the difference is build/role config, **never** a `cfg!(cloud)` branch inside
core logic. The trait lives in host so callers compile **identically** either way; only the registry's
*contents* differ. Rejected: a pure runtime flag (simpler, but forces every node to carry the SDK +
surface even when disabled — fails the "minimal profile pays for nothing it doesn't use" posture).

**Registry, not a match.** Runtime selection is a lookup in a small registry keyed by profile id, not a
`match runtime_kind { … }` — so adding the external runtime is *registering an entry*, and a node
without the feature simply has one fewer entry. No call site enumerates runtime kinds.

## How it fits the core

- **Tenancy / isolation:** the trait carries `ws`; selection and dispatch are workspace-agnostic plumbing
  — isolation is enforced downstream at the MCP chokepoint (#3) and the job record (#5). N/A to add here.
- **Capabilities:** none new. `agent.invoke` stays gated by `mcp:agent.invoke:call`; the runtime
  argument is validated against the node's registry (unknown → default), never a grant.
- **Placement:** `either`. Whether a node offers the external runtime is the **feature + config**, not a
  branch. A feature-off node and a feature-on-but-unconfigured node both behave as default-only.
- **MCP surface:** none new in #1. (The `agent.runtimes` read verb that lists the registry is #5.)
- **Data / Bus / Secrets:** N/A — #1 is a trait, a registry, and build wiring; it touches no store,
  bus, or secret.
- **SDK/WIT impact:** **adds one host-owned internal seam** (`AgentRuntime`). Like `ModelAccess` it is a
  stable internal contract callers depend on; it is **not** the WASM guest ABI. Flagged loudly.

## Example flow

1. Node A is built **without** `external-agent`. Its registry holds only `default` (in-house loop).
   `cargo tree` shows no `agent-client-protocol*` crates. `agent.invoke { … }` runs the in-house loop.
2. Node B is built **with** `--features external-agent` and configured with profile `vtcode-default`.
   Its registry holds `default` + `vtcode-default`.
3. `agent.invoke { goal, runtime: "vtcode-default" }` → registry resolves `AcpRuntime` (#2 does the
   driving). `agent.invoke { goal }` with no runtime → `default`. `agent.invoke { runtime: "bogus" }`
   → falls back to `default` (or errors per the resolution rule — decided here).

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`):

- **Compile-time optionality (this sub-scope's headline gate):** a CI matrix builds + tests the
  workspace with the feature **OFF** and **ON**; both green. An assertion (`cargo tree` / a `compile_fail`
  doc-test) proves the ACP-SDK crates are **absent** from the OFF build.
- **Default-unaffected:** with the feature ON but no profile selected, `agent.invoke` behaves exactly as
  feature-OFF (the in-house loop) — the external path adds nothing to the default path.
- **Capability-deny (§2.1):** `agent.invoke` denied without `mcp:agent.invoke:call`, identically for a
  default-runtime and an external-runtime invoke (the gate is the same).
- **Unit:** registry resolution (known id → entry, unknown → default/err, feature-off → only default);
  the trait object is dispatched without enumerating kinds at the call site.
- **Workspace-isolation (§2.2):** N/A at #1 (no data/MCP surface) — enforced in #3/#5; stated so the
  reviewer doesn't expect it here.

## Risks & hard problems

- **Feature leakage.** A stray `use lb_role_external_agent::…` in a non-gated crate would drag the SDK
  into the OFF build. The `cargo tree` assertion is the guard; keep all external-agent types behind the
  role crate + feature, never re-exported from host.
- **Trait churn.** `AgentRuntime` is a forever-ish internal contract; getting the signature wrong forces
  changes across both impls. Shape it from the in-house loop's real needs first (events out, ceiling in,
  ws + derived principal + MCP endpoint in), so the external impl conforms to it, not vice-versa.

## Open questions

- **Unknown-profile resolution:** silent fallback to `default`, or a hard error? Default proposal: error
  on an explicitly-named unknown runtime, fall back only when none was named.
- **Where the registry is built:** node boot from config, or a role-registration hook? Prefer the
  existing role-composition path the binary already uses for optional roles.
- **Selection granularity** (carried to umbrella): node-config-only now; per-workspace policy / per-invoke
  allowlist later.

## Related

- `external-agent-scope.md` (umbrella), `acp-driver-scope.md` (#2, the impl that registers here).
- `scope/ai-gateway/ai-gateway-scope.md` — the `ModelAccess`/`Provider` pattern this copies.
- `scope/agent/agent-scope.md` — the default runtime. `scope/node-roles/node-roles-scope.md` — the
  feature/role posture. README `§6.16`, `§6.5`.
