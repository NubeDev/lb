---
name: external-agent
description: >-
  Set up and drive a third-party ACP coding agent (Open Interpreter by default; VT Code / Codex as
  alternates) from a Lazybones node. Use when a task says "set up / configure the agent", "use Open
  Interpreter", "make the workspace use an external agent", "which runtime does the agent use", "pick
  the agent runtime", "route the agent's model", or "turn on the external-agent feature". Covers the
  compile-time `external-agent` cargo feature (OFF by default), the runtime registry
  (`open-interpreter-default` / `vtcode-default` / `codex-default`), the read verb `agent.runtimes`,
  the NEW per-workspace `agent.config.get`/`set` (persist the default runtime + model endpoint), and
  running a real agent via `agent.invoke { runtime }` / the channel `/agent` palette. The agent's only
  tools are our caps-checked MCP surface; its model routes through our endpoint (Z.AI GLM-4.6 today).
  It holds NO extra authority — every tool it calls is re-checked under the derived principal.
---

# Setting up and driving the external agent (Open Interpreter)

The **default runtime is the in-house loop** (`"default"`). An **external** agent — Open Interpreter
by default — is an *opt-in* runtime behind a **compile-time feature**. This skill is the operating
manual: turn it on, confirm the node offers it, persist it as the workspace default, and run it.

> **The three gates that must all be true for a real Open Interpreter run** (a common source of "why
> is it still the in-house agent?"):
> 1. the node is **built with `--features external-agent`** (OFF by default);
> 2. the `interpreter` binary is on `PATH` (v0.0.17 verified) and a provider key is in the node env;
> 3. the call **selects** the runtime (`runtime: "open-interpreter-default"`, or the workspace default
>    once you set it) — registering a runtime does not make it the default.

Everything below is grounded in a live tree: `interpreter --version` → `interpreter 0.0.17`; the role
crate's `swap_test` (9 green) proves the registry facts; the `agent_config_test` (6 green) proves the
new persistence verbs.

---

## 1. Turn the feature on (compile-time)

A plain `cargo build` produces a node with **no external agent** — the registry holds only the
in-house `"default"`, and the registration hook is a no-op. Build the node with the feature to compile
and register the ACP driver + Open Interpreter/VT Code/Codex profiles:

```bash
cd rust
cargo build -p node --features external-agent
```

At boot the node prints what it registered (from `node/src/external_agent.rs::install`):

```
external-agent: runtimes installed = ["codex-default", "default", "open-interpreter-default", "vtcode-default"]
```

- **Feature OFF** (default): `agent.runtimes` returns exactly `{ default:"default", runtimes:["default"] }`.
- **Feature ON**: the three external ids appear alongside `default`. The **default id is still
  `"default"`** — the in-house loop — until a caller/workspace selects an external one.

Provide the model key by env var **name** (never the value in any record): the default endpoint is
Z.AI GLM-4.6 over the coding endpoint, key env `ZAI_API_KEY`:

```bash
export ZAI_API_KEY=sk-…        # the driver passes the NAME to the agent; the value lives here
```

---

## 2. See what the node offers — `agent.runtimes`

The read surface behind every runtime picker. Gated by `mcp:agent.runtimes:call` (member-level).

```bash
lb call agent.runtimes '{}' -o json
# { "default": "default", "runtimes": ["codex-default", "default", "open-interpreter-default", "vtcode-default"] }
```

REST mirror: reached over `POST /mcp/call` (`{ "tool":"agent.runtimes", "args":{} }`). Read-only,
list-only, registry-derived — it reads no store record, so it cannot leak cross-workspace data (the
workspace still gates the *call*).

If you only see `["default"]`, the node was built without the feature — go back to step 1.

---

## 3. Persist the workspace's choice — `agent.config.get` / `agent.config.set`

New in the agent-config slice. One record per workspace (`workspace_agent_config:[ws]`) holding the
**default runtime** + a **names-only model endpoint**. This is what "set up the agent" writes.

- `agent.config.get` — **member** (`mcp:agent.config.get:call`). Read the current selection.
- `agent.config.set` — **admin** (`mcp:agent.config.set:call`). MERGE patch; a `default_runtime` the
  node does not offer is rejected (`BadInput`), never silently accepted.

```bash
# Read (unset → null)
lb call agent.config.get '{}' -o json
# { "config": null }

# Set the workspace default to Open Interpreter over the Z.AI coding endpoint (admin)
lb call agent.config.set '{
  "patch": {
    "default_runtime": "open-interpreter-default",
    "model_endpoint": {
      "provider": "zaicoding",
      "model": "glm-4.6",
      "api_key_env": "ZAI_API_KEY",
      "base_url": "https://api.z.ai/api/coding/paas/v4"
    }
  }
}' -o json
# { "ok": true }

lb call agent.config.get '{}' -o json
# { "config": { "default_runtime": "open-interpreter-default", "model_endpoint": { "provider":"zaicoding", … } } }
```

REST mirror: `GET /agent/config` → `{ config }`, `PUT /agent/config` (body = the patch) → `204`.

**Names only.** `api_key_env` is an env-var *name*; the key value never enters the record (a test
asserts no secret round-trips). Set the value in the node env (step 1), name it here.

**Gotchas:**
- A non-admin `agent.config.set` → `DENIED  mcp:agent.config.set:call` (opaque, non-zero exit).
- A patch naming an unavailable runtime (feature off, or a typo) → `400 BadInput` listing the offered
  ids. Confirm with `agent.runtimes` first.
- Setting the record does **not** yet change what `agent.invoke` picks when it omits `runtime` — see
  the follow-up note in §5. Today you still pass `runtime` explicitly (or use the picker).

### From the UI

**Settings → Agent** drives exactly these verbs: a runtime dropdown backed by `agent.runtimes` + the
names-only endpoint fields. Editable for an admin holding `agent.config.set`; **read-only** for a
member. A stored-but-now-unavailable runtime is flagged as registry drift rather than erroring.

---

## 4. Run the agent — `agent.invoke { runtime }`

Invoking is gated by `mcp:agent.invoke:call` (the SAME gate for every runtime — choosing a runtime is
an *argument*, not a new grant). Select the external runtime by id:

```bash
lb call agent.invoke '{
  "runtime": "open-interpreter-default",
  "goal": "write hello.py that prints hi, run it, report the output"
}' -o json
```

- **Absent `runtime`** → the in-house `"default"` loop.
- **A known id** → that runtime (`open-interpreter-default` spawns the real `interpreter` subprocess).
- **An unknown id** → error (never a silent downgrade to a different engine).

### In a channel (the `/agent` palette command)

In a channel, `/agent` is a first-class palette command whose `runtime` arg is a **dropdown** backed
by `agent.runtimes`. Pick `open-interpreter-default`, type the goal, submit → the live `AgentCard`
streams the run to the answer. (The command appears only for a member holding `mcp:agent.invoke:call`.)

### What actually happens under the hood

The `AcpRuntime` spawns the profile's binary in an isolated scratch dir. For Open Interpreter (a Rust
fork of Codex) the codex-family wrapper builds:

```
interpreter exec --json --skip-git-repo-check -C <scratch> \
  -c model_providers.zaicoding.name=zaicoding \
  -c model_providers.zaicoding.base_url=https://api.z.ai/api/coding/paas/v4 \
  -c model_providers.zaicoding.env_key=ZAI_API_KEY \
  -c model_providers.zaicoding.wire_api=chat \
  -c model_provider=zaicoding \
  -m glm-4.6 "<goal>"
```

`wire_api=chat` is mandatory (Z.AI/our gateway speak Chat Completions, not the OpenAI Responses API).
The agent's tools are **only** our MCP surface, re-checked under the derived `caller ∩ agent`
principal — being allowed to invoke never implies the tools/skills it may reach.

---

## 5. Swapping the agent (Open Interpreter → VT Code / Codex)

The swap is **data, not code**: pick a different `runtime` id. `open-interpreter-default` and
`codex-default` even share the identical wrapper — they differ *only* by binary (`interpreter` vs
`codex`), the cleanest possible proof of the seam. VT Code is `vtcode-default` over its own wrapper.

```bash
lb call agent.invoke '{ "runtime": "vtcode-default", "goal": "…" }' -o json
# or persist it: lb call agent.config.set '{"patch":{"default_runtime":"vtcode-default"}}'
```

**Named follow-up (not yet wired):** having `agent.invoke` read `agent.config.get` and use the stored
`default_runtime` when `runtime` is omitted. Until that lands, the persisted default is
displayed/authoritative for the UI but the invoke call must still name the runtime (or the picker
does). Falling back to the registry default if the stored id is unavailable is part of the same slice.

---

## 6. Verify it end to end

```bash
# 1. Registry offers the external ids (feature on)
lb call agent.runtimes '{}' -o json | grep open-interpreter-default

# 2. Persisted selection round-trips
lb call agent.config.set '{"patch":{"default_runtime":"open-interpreter-default"}}'
lb call agent.config.get '{}' -o json

# 3. Real run (needs ZAI_API_KEY + interpreter on PATH)
lb call agent.invoke '{"runtime":"open-interpreter-default","goal":"print the current date"}' -o json
```

The role crate also carries a real-subprocess smoke test — `#[ignore]` by default (it needs a real
binary + a non-throttled key). Drive it deliberately:

```bash
cd rust
EXTAGENT_SMOKE=1 ZAI_API_KEY=sk-… \
  cargo test -p lb-role-external-agent --test smoke_test -- --ignored --nocapture
# override the agent with EXTAGENT_PROFILE=vtcode-default (or codex-default)
```

---

## Surface summary

| Verb | Cap | Level | Route | Purpose |
|---|---|---|---|---|
| `agent.runtimes` | `mcp:agent.runtimes:call` | member | `POST /mcp/call` | List the node's offered runtimes + default |
| `agent.config.get` | `mcp:agent.config.get:call` | member | `GET /agent/config` | Read the workspace's persisted selection |
| `agent.config.set` | `mcp:agent.config.set:call` | **admin** | `PUT /agent/config` | Persist default runtime + names-only endpoint |
| `agent.invoke` | `mcp:agent.invoke:call` | member | `POST /mcp/call` | Run the agent on a `{runtime, goal}` |

**Never**: put a raw API key in `agent.config.set` (it takes the env-var *name*); expect an external
runtime without the `--features external-agent` build; assume registering a runtime makes it the
default (it doesn't — select it, or set the workspace default).

## Related

- Public: [`../../public/external-agent/external-agent.md`](../../public/external-agent/external-agent.md)
  ("Agent config"), [`../../public/prefs/prefs.md`](../../public/prefs/prefs.md) (the sibling Settings
  Preferences tab).
- Scope: [`../../scope/external-agent/external-agent-scope.md`](../../scope/external-agent/external-agent-scope.md)
  (umbrella + the five sub-scopes), [`../../scope/external-agent/agent-config-scope.md`](../../scope/external-agent/agent-config-scope.md).
- Skill: [`../lb-cli/SKILL.md`](../lb-cli/SKILL.md) (the `lb call` transport used throughout).
