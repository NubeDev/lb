# Agent-personas scope — the extension-builder persona (persona-coding)

Status: **SHIPPED** (`builtin.extension-builder` seeded with the Ask `policy_preset` + in-house
`runtimes` restriction; `clamp_to_preset` floor + `check_runtime` applied at run assembly; 10 tests
incl. the "never on its own" suspend-e2e green). Sub-scope #4 of `agent-personas-scope.md` — the last
umbrella gate bullet. Session:
[`sessions/agent-personas/persona-coding-session.md`](../../sessions/agent-personas/persona-coding-session.md).
Promoted to [`public/agent-personas/agent-personas.md`](../../public/agent-personas/agent-personas.md).

> **Two findings (recorded, not coded around):** (1) the `policy_preset` floor is a **clamp** applied
> after `evaluate`, NOT a merged rule — an appended Ask can't beat a blanket Allow under the evaluator's
> Deny>Allow>Ask precedence. (2) a TOML sub-table binding bug (`runtimes` authored after
> `[persona.policy_preset]` bound to it) — fixed by ordering; both caught by tests.

The coding persona — `builtin.extension-builder` — with the posture stated by the ask verbatim:
**"100% coding, but never on its own."** The agent writes UI / WASM / native (Tier-2) process
**extensions** against the devkit — scaffolded, built, inspected, published through the
platform's own gated verbs — and is **supervised at every consequential step** via the shipped
per-tool-call Ask policy. It is *not* a free-range coding agent over the repository: the
platform is extended through extensions (the "everything else is an extension" thesis), so the
coding persona builds extensions, in a devkit workdir, full stop.

## Goals

- **The persona (seeded with #3's `personas.toml`, specified here):**
  - **identity:** an extension developer for this platform; scaffolds from devkit templates,
    follows FILE-LAYOUT (≤400-line files, one responsibility), builds real (`devkit.build` +
    `build.sh`), tests against the live node per the e2e runbooks, and **always proposes —
    never assumes — publish/install**.
  - **granted_tools:** `devkit.*` (templates/scaffold/inspect/build/root), `host.fs.list`
    (workdir picker read), `ext.list`, `ext.publish`, `ext.disable`, `ext.uninstall`,
    `native.install`, `native.call`, `native.reset`, plus the verify loop: `tools.catalog`
    (is my tool registered + reachable?), `bus.watch` (my extension's subjects),
    `telemetry.read`, `system.tools`. All devkit/ext verbs are **admin-tier** — this persona
    is only useful to an admin-capped caller, by design (a member caller gets the honest deny).
  - **grounding_skills (pinned):** `core.extension-authoring` (#2 — the devkit manual + WIT
    boundary + FILE-LAYOUT rules folded in), `core.extensions` (operator view),
    `core.e2e-backend`. Catalog: `core.mcp`, `core.lb-cli`, `core.testing-*`.
- **"Never on its own" = the shipped Ask policy, preconfigured.** Agent-run Part 2 already
  ships per-tool-call **Allow/Deny/Ask** with durable suspension (`agent.policy.set`,
  `agent.decide`). This persona ships a **policy preset** applied on activation:
  - **Ask (human gate):** `ext.publish`, `ext.uninstall`, `ext.disable`, `native.install`,
    `native.reset` — anything that changes what runs on the node.
  - **Allow:** `devkit.templates/scaffold/inspect/build`, `host.fs.list`, the read verbs —
    the edit-build-inspect inner loop stays fluid (supervision that prompts on every build is
    supervision that gets turned off).
  - The preset is **data on the persona record** (#1 gains an optional `policy_preset` field —
    the one persona-model addition this sub-scope requires), applied via the existing
    `agent.policy.set` machinery on persona activation; an admin can tighten it, and
    **loosening below the seed preset requires an explicit per-workspace admin write** (the
    preset is a floor, not a suggestion).
- **The workdir is the boundary.** All scaffold/build happens under `devkit.root`'s workdir —
  per-workspace, node-configured. For the **external** runtime this persona's runs are where
  the capability-wall sandbox (external-agent #3) earns its keep: scratch = the devkit workdir,
  egress = gateway only. **Dependency stated plainly:** until external-agent #3 ships, this
  persona is **in-house-runtime-only by seed** (a `runtimes: ["default"]` restriction on the
  persona record — #1's apply refuses the pairing with a named error). The in-house loop
  reaches code only through the devkit verbs (no raw fs/shell tools exist in the catalog), so
  the MCP wall alone genuinely bounds it — that asymmetry is why the restriction is honest
  rather than theater.
- **Built ≠ done: the persona tests what it builds.** The identity + `core.e2e-backend` pin
  bind the platform rule "test the change in the same session": scaffold → build →
  `ext.publish` (Ask → human approves) → drive the new tool via its own MCP call → assert via
  `tools.catalog`/`telemetry.read`. The hello/hello-v2 extensions are the reference targets.

## Non-goals

- General repo-wide coding (working ON the platform itself) — that is a human-driven Claude
  Code session, not a workspace persona; the persona's surface deliberately contains no
  git/fs/shell verbs and no path outside the devkit workdir.
- The OS sandbox itself — external-agent #3 owns it; this persona *consumes* it (and is
  restricted to the in-house runtime until then).
- Registry federation / marketplace publishing (`registry.install/publish` are scoped-only —
  excluded from the persona until they ship; noted for the seed bump).
- CI-grade build infrastructure — `devkit.build` is the node's local build door; keep it.

## Intent / approach

**Extensions are the sanctioned way code enters the platform, so the coding persona is the
devkit made agentic — nothing more.** Every capability it needs already exists as an
admin-gated MCP verb; the persona is curation (the devkit surface), grounding (the authoring
manual), and a supervision preset (Ask on mutation of the running node). The "never on its own"
requirement maps onto machinery that already ships (Part-2 policy + durable suspension +
`agent.decide`), so supervision is **durable and auditable** — an approval survives a
disconnect, and the transcript shows who approved which publish.

**Rejected: a repo-access coding persona** (hand the agent the source tree + shell). It is
exactly the confused whole-codebase posture this topic exists to kill, it bypasses every
mediated seam (rule 5), and its blast radius is the node itself. If the platform ever wants an
agent working on core code, that's a separate scope with the external-agent sandbox as a hard
prerequisite — named here so it isn't smuggled in.

**Rejected: Ask-on-everything.** Gating the inner loop (scaffold/build/inspect) teaches the
user to click through; the gate belongs only on state-changing verbs where a wrong approval has
consequences. The preset draws that line as data, adjustable per workspace.

## How it fits the core

- **Tenancy / isolation:** the devkit workdir + any published artifact are per-workspace;
  `ext.publish` lands in the workspace's registry namespace; ws-B cannot see ws-A's workdir
  (`host.fs.list` is rooted) or artifacts — mandatory isolation test.
- **Capabilities:** no new caps; the persona advertises admin-tier verbs and the wall does the
  rest (member caller → deny, the mandatory test). The **policy preset** is not a cap: it
  layers the shipped Ask gate *on top of* granted caps — narrowing interaction, never widening.
- **Placement:** either, but honest about the runtime restriction (above) — the pairing rule
  is data on the record, not an `if` in core.
- **MCP surface:** consumes only. One additive optional field on the #1 record
  (`policy_preset`, plus `runtimes` restriction) — flagged back into #1's model as its single
  cross-sub-scope change.
- **Data:** none beyond #1's record fields; approvals/suspensions ride the shipped run/job
  records.
- **Bus / secrets:** none new; `bus.watch` is the persona's read of its own extension's
  subjects. **WIT impact:** none (the persona *targets* the WIT boundary; it doesn't change it).
- **No mocks (rule 9):** the tests scaffold and build a **real** extension via the real devkit
  verbs against a live node (the hello template), publish it for real, and call its tool for
  real. Nothing simulated.
- **File layout:** the only code is #1's two record fields + the preset application in
  `personas/apply.rs`; the rest is `personas.toml` + skills content.
- **Skill doc:** `core.extension-authoring` (#2) is this persona's manual; the persona row
  lands in `skills/agent/SKILL.md`'s table with the Ask-preset note.

## Example flow

1. Admin activates `builtin.extension-builder`; the Ask preset applies
   (publish/install/uninstall gated).
2. "Build me a widget extension that renders our energy series as a heatmap." The agent
   (grounded in `core.extension-authoring`) runs `devkit.templates` → `devkit.scaffold`
   (federation-widget template, workdir) → iterates → `devkit.build` — all Allow, fluid.
3. It proposes `ext.publish` → the run **suspends durably**; the admin sees the approval card
   (`agent.decide`), reviews `devkit.inspect` output, approves.
4. The agent verifies: `tools.catalog` shows the new tool; it drives one real render call;
   `telemetry.read` confirms; it reports with the evidence, then proposes the e2e runbook
   checks from `core.e2e-backend`.
5. A member tries the same persona: every devkit verb denies (admin-tier) — the persona
   narrowed nothing into existence.

## Testing plan

- **Capability-deny (§2.1):** member caller under this persona → `devkit.scaffold` and
  `ext.publish` denied at the wall; nothing on disk, nothing published (THE persona-never-
  widens headline on the highest-stakes surface).
- **Ask-gate (the "never on its own" proof):** a scripted run proposing `ext.publish` suspends;
  no artifact is published before `agent.decide` approval; a **deny** decision feeds back and
  the run continues without the effect; the suspension **survives a node restart** (durable —
  re-assert the Part-2 test under this preset).
- **Preset-floor:** loosening `ext.publish` below Ask without the explicit admin write is
  rejected; with it, honored (and auditable).
- **Runtime restriction:** activating this persona with an external runtime selected → named
  error at run start, no subprocess (until external-agent #3 flips the seed).
- **Workspace-isolation (§2.2):** ws-B can't list ws-A's workdir or see its published
  artifact/tool in catalog.
- **End-to-end (rule 9):** the full example flow against a live node — scaffold → build →
  approve → publish → call the new tool — green in the session doc.

## Risks & hard problems

- **Approval fatigue is the failure mode of "never on its own."** If publish-approve becomes a
  reflex click, the gate is theater. The approval card must carry *reviewable substance*
  (`devkit.inspect` summary: declared caps, tools, size) — an approve button next to real
  information, not next to a spinner. Treat a bare "agent wants ext.publish — OK?" card as a
  finding.
- **The devkit verbs' own hardening becomes load-bearing.** `devkit.scaffold/build` were built
  for Studio (a trusted admin UI); an agent will fuzz them with hostile-shaped input (path
  traversal in names, template injection). The session must adversarially test the verbs
  themselves, not just the persona.
- **`native.*` (Tier-2) is the sharpest tool** — a native sidecar is host code. The Ask gate +
  admin tier hold today; when external-agent #3 lands, revisit whether `native.install`
  proposed by an *external* runtime should be Deny-by-seed rather than Ask.
- **Scope creep toward "fix the platform itself."** Users will ask it to edit core. The
  identity says no and names the boundary (extensions only); the tool surface enforces it
  (nothing can touch the repo). Keep both aligned.

## Open questions

1. **Publish-approval reviewer:** any admin, or the invoking admin only? Proposal: any
   workspace admin (the approval is workspace-owned, matching `workflow.resolve_approval`).
2. **Does `devkit.build` output (warnings/errors) stream as run events** or return as one blob?
   Proposal: blob first (the verb's current shape), streaming when `agent-close-out` C lands.
3. **UI-extension preview:** the widget dev loop wants a render preview before publish — reuse
   the Studio preview seam, or is "publish to a scratch workspace" the honest v1? Proposal:
   scratch-workspace publish v1; preview seam is Studio's scope.

## Related

- `agent-personas-scope.md` (umbrella), `persona-model-scope.md` (#1 — gains `policy_preset` +
  `runtimes`), `persona-grounding-scope.md` (#2 — `core.extension-authoring`),
  `persona-catalog-scope.md` (#3 — the seed file this row lives in).
- `scope/extensions/extensions-scope.md` (the surface), `scope/agent-run/agent-run-scope.md`
  Part 2 (Allow/Deny/Ask + durable suspension — the machinery), `scope/external-agent/
  capability-wall-scope.md` (the sandbox this persona waits on for external runtimes),
  `scope/coding-workflow/` (the S6 composition this persona feeds).
- `docs/FILE-LAYOUT.md` (folded into the authoring skill), `docs/testing/e2e-backend.md`.
- README `§3` (rules 4, 5, 10), `§6.7` (extension runtime), `§6.16`.
